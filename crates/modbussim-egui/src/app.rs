use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use modbussim_core::data_source::{DataSource, DataSourceConfig, DataSourceState};
use modbussim_core::log_collector::LogCollector;
use modbussim_core::log_entry::LogEntry;
use modbussim_core::register::{decode_value, DataType, Endian, RegisterDef, RegisterType};
use modbussim_core::slave::{ConnectionState, SlaveConnection, SlaveDevice};
use modbussim_core::transport::Transport;
use modbussim_ui_shared::format::{format_u16, U16Format};
use modbussim_ui_shared::icons;
use modbussim_ui_shared::log_panel::{self, LogPanelAction, LogPanelState};
use modbussim_ui_shared::theme::{self, Flavor};
use modbussim_ui_shared::ui as uikit;
use modbussim_ui_shared::value_panel::{self, F64Order};
use modbussim_ui_shared::project::{
    deserialize_slave, serialize_slave, SlaveConnectionSave, SlaveDeviceSave, SlaveProject, TcpSpec,
};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

/// Holds a connection together with its log collector. Authoritative state
/// lives in `connection`; the UI side renders off `ConnSnapshot` below.
pub struct SlaveConnectionEntry {
    pub id: String,
    #[allow(dead_code)]
    pub label: String,
    pub connection: Arc<RwLock<SlaveConnection>>,
    #[allow(dead_code)] // Read by log panel in S3.
    pub log_collector: Arc<LogCollector>,
}

pub type SharedConnections = Arc<RwLock<Vec<SlaveConnectionEntry>>>;

/// One active data-source binding: periodically writes next_value() into a
/// specific register address of a specific device.
pub struct ActiveSource {
    pub id: u64,
    pub conn_id: String,
    pub slave_id: u8,
    pub reg_type: RegisterType, // FC03 / FC04 only (we don't drive coils here)
    pub addr: u16,
    pub enabled: bool,
    pub state: DataSourceState,
    pub last_output: Option<Instant>,
}

pub type SharedSources = Arc<tokio::sync::Mutex<Vec<ActiveSource>>>;

/// Minimal "kind" dropdown for the quick-add form; each kind maps to a
/// DataSource variant with sensible defaults.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DsKind {
    Counter,
    Sine,
    Sawtooth,
    Triangle,
    Random,
    Fixed,
    CsvPlayback,
}

impl DsKind {
    pub fn label(&self) -> &'static str {
        match self {
            DsKind::Counter => "计数器 (+1)",
            DsKind::Sine => "正弦波",
            DsKind::Sawtooth => "锯齿波",
            DsKind::Triangle => "三角波",
            DsKind::Random => "随机 U16",
            DsKind::Fixed => "固定值",
            DsKind::CsvPlayback => "CSV 序列",
        }
    }

    pub fn default_source(&self) -> DataSource {
        match self {
            DsKind::Counter => DataSource::Counter {
                start: 0,
                step: 1,
                wrap: true,
            },
            DsKind::Sine => DataSource::Sine {
                amplitude: 10000.0,
                frequency: 0.5,
                offset: 32768.0,
                phase: 0.0,
            },
            DsKind::Sawtooth => DataSource::Sawtooth {
                min: 0,
                max: 1000,
                period_ms: 5000,
            },
            DsKind::Triangle => DataSource::Triangle {
                min: 0,
                max: 1000,
                period_ms: 5000,
            },
            DsKind::Random => DataSource::Random {
                min: 0,
                max: 65535,
            },
            DsKind::Fixed => DataSource::Fixed { value: 42 },
            DsKind::CsvPlayback => DataSource::CsvPlayback {
                values: vec![0, 100, 200, 300, 400],
                loop_playback: true,
            },
        }
    }
}

/// What the user's search-box input semantically means.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SearchIntent {
    /// Empty input — no filtering, no jump.
    None,
    /// Valid u16 address — scroll-to-row and highlight.
    Jump(u16),
    /// Non-numeric / out-of-range — substring filter on address text (rendered as
    /// decimal); matches register rows whose "1234"-style address contains the
    /// lowercased needle. Name/comment filtering will be added when register_defs
    /// get cached in RegViewCache.
    Filter(String),
}

fn parse_search_intent(raw: &str) -> SearchIntent {
    let t = raw.trim();
    if t.is_empty() {
        return SearchIntent::None;
    }
    if let Some(rest) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        if let Ok(n) = u16::from_str_radix(rest, 16) {
            return SearchIntent::Jump(n);
        }
    }
    if let Ok(n) = t.parse::<u16>() {
        return SearchIntent::Jump(n);
    }
    SearchIntent::Filter(t.to_lowercase())
}

fn source_short_desc(s: &DataSource) -> String {
    match s {
        DataSource::Fixed { value } => format!("Fixed={}", value),
        DataSource::Random { min, max } => format!("Rand[{}..{}]", min, max),
        DataSource::Sine { amplitude, frequency, offset, .. } => {
            format!("Sine A={} f={}Hz off={}", amplitude, frequency, offset)
        }
        DataSource::Sawtooth { period_ms, .. } => format!("Sawtooth T={}ms", period_ms),
        DataSource::Triangle { period_ms, .. } => format!("Triangle T={}ms", period_ms),
        DataSource::Counter { step, wrap, .. } => format!("Counter +{} wrap={}", step, wrap),
        DataSource::CsvPlayback { values, .. } => format!("CSV ({} pts)", values.len()),
    }
}

/// Snapshot of register counts for a device, used when rendering the tree.
#[derive(Clone, Default)]
pub struct RegCounts {
    pub coils: usize,
    pub discrete_inputs: usize,
    pub holding_registers: usize,
    pub input_registers: usize,
}

impl RegCounts {
    pub fn from_device(dev: &SlaveDevice) -> Self {
        Self {
            coils: dev.register_map.coils.len(),
            discrete_inputs: dev.register_map.discrete_inputs.len(),
            holding_registers: dev.register_map.holding_registers.len(),
            input_registers: dev.register_map.input_registers.len(),
        }
    }

    pub fn count_for(&self, rt: RegisterType) -> usize {
        match rt {
            RegisterType::Coil => self.coils,
            RegisterType::DiscreteInput => self.discrete_inputs,
            RegisterType::HoldingRegister => self.holding_registers,
            RegisterType::InputRegister => self.input_registers,
        }
    }
}

#[derive(Clone)]
pub struct DeviceSnapshot {
    pub slave_id: u8,
    pub name: String,
    pub counts: RegCounts,
    pub expanded: bool,
}

/// Messages from async tasks to the UI thread.
pub enum UiEvent {
    ConnectionCreated {
        id: String,
        label: String,
        devices: Vec<DeviceSnapshot>,
    },
    ConnectionStarted(String),
    ConnectionStopped(String),
    ConnectionRemoved(String),
    DeviceCountsUpdated {
        conn_id: String,
        slave_id: u8,
        counts: RegCounts,
    },
    DeviceAdded {
        conn_id: String,
        device: DeviceSnapshot,
    },
    DeviceRemoved {
        conn_id: String,
        slave_id: u8,
    },
    Info(String),
    Error(String),
}

/// Immutable view of a connection for UI rendering.
#[derive(Clone)]
struct ConnSnapshot {
    id: String,
    label: String,
    state: ConnectionState,
    expanded: bool,
    devices: Vec<DeviceSnapshot>,
}

/// What the user currently has selected in the tree; drives the main panel.
#[derive(Clone, PartialEq, Eq)]
enum Selection {
    None,
    Connection(String),
    Device {
        conn_id: String,
        slave_id: u8,
    },
    RegisterGroup {
        conn_id: String,
        slave_id: u8,
        reg_type: RegisterType,
    },
}

/// Per-frame cache of the currently displayed register group. We keep an
/// Arc-cloned snapshot of the source HashMap (cheap vs. materializing a
/// Vec<Option<u16>> across 20K rows every frame) — UI renders only the
/// visible window via TableBuilder, so HashMap::get stays O(1) per row.
pub struct RegViewCache {
    pub conn_id: String,
    pub slave_id: u8,
    pub reg_type: RegisterType,
    pub row_count: usize,
    /// Snapshot for FC03 / FC04.
    pub u16_map: Option<Arc<std::collections::HashMap<u16, u16>>>,
    /// Snapshot for FC01 / FC02.
    pub bool_map: Option<Arc<std::collections::HashMap<u16, bool>>>,
    /// addr → (name, comment) from the device's register_defs for this reg_type.
    /// Populated during refresh_reg_view so the table body can display names /
    /// comments without re-locking `connections` per frame.
    pub defs: Arc<std::collections::HashMap<u16, (String, String)>>,
}

pub struct SlaveApp {
    rt: Arc<Runtime>,
    connections: SharedConnections,
    data_sources: SharedSources,
    next_source_id: Arc<AtomicU64>,
    events_tx: crossbeam_channel::Sender<UiEvent>,
    events_rx: crossbeam_channel::Receiver<UiEvent>,

    // UI state
    selection: Selection,
    new_host: String,
    new_port: String,
    last_error: Option<String>,

    // Event-driven snapshot (never read from Arc<RwLock<...>> on the UI thread).
    conn_snapshot: Vec<ConnSnapshot>,
    next_conn_seq: Arc<AtomicU64>,

    // Register table view
    reg_view: Option<RegViewCache>,
    reg_row_limit: usize,
    reg_view_last_refresh: Option<Instant>,
    reg_view_refresh_interval_ms: u64,

    // Batch add modal
    batch_modal: Option<BatchModalState>,
    add_device_modal: Option<AddDeviceModalState>,
    status_msg: Option<String>,

    // Inline edit buffer: keyed by (reg_type, addr); implicitly bound to the
    // currently-selected (conn_id, slave_id) — cleared whenever selection moves.
    pending_edits: HashMap<(RegisterType, u16), i32>,

    // Row selection for ValuePanel multi-register analysis
    selected_addrs: BTreeSet<u16>,
    // Anchor row for shift-click range selection. Set on plain click or
    // ctrl-click; shift-click always ranges [anchor .. addr]. Prevents anchor
    // drift when shift-clicking multiple times in a row.
    click_anchor: Option<u16>,

    // RegisterGroup search: per-(conn_id, slave_id, reg_type) text, so moving
    // between groups preserves what the user typed. Filters / jumps based on
    // the input shape (see parse_search_intent).
    search_buf: HashMap<(String, u8, RegisterType), String>,
    /// Row currently being highlighted from an address jump. Fades over 2s.
    highlight: Option<(String, u8, RegisterType, u16, Instant)>,
    /// Tells the RegisterGroup renderer to `request_focus()` the search
    /// TextEdit on the next frame (Cmd+F / Ctrl+F sets this).
    want_focus_search: bool,

    // Data source "add" form
    ds_add_addr: u32,
    ds_add_kind: DsKind,
    ds_add_interval_ms: u64,

    pub flavor: Flavor,

    // Register table display mode
    reg_display_mode: ValueDisplayMode,

    // Log panel
    log_state: LogPanelState,
    log_cache: Vec<LogEntry>,
    log_cache_conn_id: Option<String>,
    log_last_refresh: Option<Instant>,
}

pub struct BatchModalState {
    pub conn_id: String,
    pub slave_id: u8,
    pub start_addr: u32,
    pub end_addr: u32,
    pub reg_type: RegisterType,
    pub data_type: DataType,
    pub endian: Endian,
    pub name_prefix: String,
    pub busy: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DeviceInitMode {
    Empty,
    Default,
    Random,
}

pub struct AddDeviceModalState {
    pub conn_id: String,
    pub slave_id: u8,
    pub name: String,
    pub init_mode: DeviceInitMode,
    pub max_address: u32,
    pub busy: bool,
}

impl SlaveApp {
    pub fn new(rt: Arc<Runtime>, flavor: Flavor) -> Self {
        let (events_tx, events_rx) = crossbeam_channel::unbounded();
        let connections: SharedConnections = Arc::new(RwLock::new(Vec::new()));
        let data_sources: SharedSources = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        // Background runner: every 50 ms, iterate active sources and write
        // their next_value into the target device's RegisterMap when due.
        {
            let connections = connections.clone();
            let sources = data_sources.clone();
            rt.spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    let mut srcs = sources.lock().await;
                    if srcs.is_empty() { continue }
                    let now = Instant::now();
                    // Collect writes first so we don't hold the sources lock while touching connections.
                    let mut updates: Vec<(String, u8, RegisterType, u16, u16)> = Vec::new();
                    for s in srcs.iter_mut() {
                        if !s.enabled { continue }
                        let interval = std::time::Duration::from_millis(
                            s.state.config.update_interval_ms.max(10),
                        );
                        let due = match s.last_output {
                            Some(t) => now.duration_since(t) >= interval,
                            None => true,
                        };
                        if due {
                            let v = s.state.next_value();
                            s.last_output = Some(now);
                            updates.push((s.conn_id.clone(), s.slave_id, s.reg_type, s.addr, v));
                        }
                    }
                    drop(srcs);
                    if updates.is_empty() { continue }
                    let conns = connections.read().await;
                    for (conn_id, slave_id, rtype, addr, v) in updates {
                        let Some(entry) = conns.iter().find(|e| e.id == conn_id) else { continue };
                        let conn = entry.connection.read().await;
                        let mut devs = conn.devices.write().await;
                        if let Some(dev) = devs.get_mut(&slave_id) {
                            match rtype {
                                RegisterType::HoldingRegister => {
                                    dev.register_map.holding_registers.insert(addr, v);
                                }
                                RegisterType::InputRegister => {
                                    dev.register_map.input_registers.insert(addr, v);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            });
        }

        // Background jitter runner: every 100 ms, iterate connections → devices,
        // apply `jitter::apply_tick` to any device whose jitter is enabled and
        // whose interval_ms has elapsed since its last tick.
        {
            let connections = connections.clone();
            rt.spawn(async move {
                use rand::SeedableRng;
                let mut rng = rand::rngs::StdRng::from_entropy();
                let mut last_tick: std::collections::HashMap<(String, u8), Instant> =
                    std::collections::HashMap::new();
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    let now = Instant::now();
                    let conns = connections.read().await;
                    for entry in conns.iter() {
                        let conn_id = entry.id.clone();
                        let conn = entry.connection.read().await;
                        let mut devs = conn.devices.write().await;
                        for (slave_id, dev) in devs.iter_mut() {
                            if !dev.jitter.enabled {
                                last_tick.remove(&(conn_id.clone(), *slave_id));
                                continue;
                            }
                            let interval = std::time::Duration::from_millis(
                                dev.jitter.interval_ms.clamp(100, 5000),
                            );
                            let key = (conn_id.clone(), *slave_id);
                            let due = match last_tick.get(&key) {
                                Some(t) => now.duration_since(*t) >= interval,
                                None => true,
                            };
                            if !due {
                                continue;
                            }
                            modbussim_core::jitter::apply_tick(
                                &mut dev.register_map,
                                &dev.jitter,
                                &mut rng,
                            );
                            last_tick.insert(key, now);
                        }
                    }
                }
            });
        }

        Self {
            rt,
            connections,
            data_sources,
            next_source_id: Arc::new(AtomicU64::new(1)),
            events_tx,
            events_rx,
            selection: Selection::None,
            new_host: "0.0.0.0".to_string(),
            new_port: "5502".to_string(),
            last_error: None,
            conn_snapshot: Vec::new(),
            next_conn_seq: Arc::new(AtomicU64::new(1)),
            reg_view: None,
            reg_row_limit: 20001,
            reg_view_last_refresh: None,
            reg_view_refresh_interval_ms: 200,
            batch_modal: None,
            add_device_modal: None,
            status_msg: None,
            pending_edits: HashMap::new(),
            selected_addrs: BTreeSet::new(),
            click_anchor: None,
            search_buf: HashMap::new(),
            highlight: None,
            want_focus_search: false,
            ds_add_addr: 0,
            ds_add_kind: DsKind::Counter,
            ds_add_interval_ms: 1000,
            flavor,
            reg_display_mode: ValueDisplayMode::U16,
            log_state: LogPanelState::new(),
            log_cache: Vec::new(),
            log_cache_conn_id: None,
            log_last_refresh: None,
        }
    }

    /// Pull the most recent N log entries for the selected connection into cache.
    /// Throttled to ~500ms. Uses sync try_read to stay off the async path.
    fn refresh_log_cache(&mut self) {
        let Some(id) = selection_conn_id(&self.selection) else {
            self.log_cache.clear();
            self.log_cache_conn_id = None;
            return;
        };

        // Throttle when already tracking the same connection.
        if self.log_cache_conn_id.as_deref() == Some(id) {
            if let Some(t) = self.log_last_refresh {
                if t.elapsed().as_millis() < 500 {
                    return;
                }
            }
        } else {
            self.log_cache.clear();
        }

        let Ok(entries) = self.connections.try_read() else { return };
        let Some(entry) = entries.iter().find(|e| e.id == id) else { return };
        let Some(mut all) = entry.log_collector.try_get_all() else { return };
        let start = all.len().saturating_sub(500);
        self.log_cache = all.drain(start..).collect();
        self.log_cache_conn_id = Some(id.to_string());
        self.log_last_refresh = Some(Instant::now());
    }

    fn clear_logs_for_selection(&self) {
        let Some(id) = selection_conn_id(&self.selection) else { return };
        let id = id.to_string();
        let connections = self.connections.clone();
        self.rt.spawn(async move {
            let entries = connections.read().await;
            if let Some(entry) = entries.iter().find(|e| e.id == id) {
                entry.log_collector.clear().await;
            }
        });
    }

    fn export_logs_for_selection(&mut self, ctx: egui::Context) {
        let Some(id) = selection_conn_id(&self.selection) else { return };
        let id = id.to_string();
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        self.rt.spawn(async move {
            let entries = connections.read().await;
            let Some(entry) = entries.iter().find(|e| e.id == id) else {
                let _ = tx.send(UiEvent::Error(format!("连接 {id} 未找到")));
                return;
            };
            let csv = entry.log_collector.export_csv().await;
            drop(entries);
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let default_name = format!("modbus_log_{}_{}.csv", id, ts);
            let Some(path) = rfd::AsyncFileDialog::new()
                .set_file_name(&default_name)
                .add_filter("CSV", &["csv"])
                .save_file()
                .await
            else {
                return;
            };
            match tokio::fs::write(path.path(), csv).await {
                Ok(()) => {
                    let _ = tx.send(UiEvent::Info(format!("日志已导出：{}", path.path().display())));
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("导出失败: {e}")));
                }
            }
        });
        ctx.request_repaint();
    }

    /// Spawn an async task that writes a single address into the device's
    /// RegisterMap. For Coil/DiscreteInput value 0/1 selects false/true.
    fn commit_write(
        &self,
        conn_id: String,
        slave_id: u8,
        reg_type: RegisterType,
        addr: u16,
        value: u16,
        ctx: egui::Context,
    ) {
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == conn_id)
                .map(|e| e.connection.clone());
            let Some(conn_arc) = conn_arc else {
                let _ = tx.send(UiEvent::Error(format!("连接 {conn_id} 未找到")));
                return;
            };
            let new_counts = {
                let conn = conn_arc.read().await;
                let mut devs = conn.devices.write().await;
                let Some(dev) = devs.get_mut(&slave_id) else {
                    let _ = tx.send(UiEvent::Error(format!("从站 {slave_id} 未找到")));
                    return;
                };
                match reg_type {
                    RegisterType::Coil => {
                        dev.register_map.write_coil(addr, value != 0);
                    }
                    RegisterType::DiscreteInput => {
                        dev.register_map.discrete_inputs.insert(addr, value != 0);
                    }
                    RegisterType::HoldingRegister => {
                        dev.register_map.write_holding_register(addr, value);
                    }
                    RegisterType::InputRegister => {
                        dev.register_map.input_registers.insert(addr, value);
                    }
                }
                RegCounts::from_device(dev)
            };
            let _ = tx.send(UiEvent::DeviceCountsUpdated {
                conn_id,
                slave_id,
                counts: new_counts,
            });
        });
        ctx.request_repaint();
    }

    fn add_data_source(
        &self,
        conn_id: String,
        slave_id: u8,
        reg_type: RegisterType,
        addr: u16,
        kind: DsKind,
        interval_ms: u64,
    ) {
        let id = self.next_source_id.fetch_add(1, Ordering::Relaxed);
        let cfg = DataSourceConfig {
            source: kind.default_source(),
            update_interval_ms: interval_ms.max(10),
        };
        let state = DataSourceState::new(cfg);
        let sources = self.data_sources.clone();
        self.rt.spawn(async move {
            sources.lock().await.push(ActiveSource {
                id,
                conn_id,
                slave_id,
                reg_type,
                addr,
                enabled: true,
                state,
                last_output: None,
            });
        });
    }

    fn toggle_data_source(&self, id: u64) {
        let sources = self.data_sources.clone();
        self.rt.spawn(async move {
            let mut g = sources.lock().await;
            if let Some(s) = g.iter_mut().find(|s| s.id == id) {
                s.enabled = !s.enabled;
                s.last_output = None; // immediate output after re-enable
            }
        });
    }

    fn remove_data_source(&self, id: u64) {
        let sources = self.data_sources.clone();
        self.rt.spawn(async move {
            sources.lock().await.retain(|s| s.id != id);
        });
    }

    fn set_device_jitter(
        &self,
        conn_id: String,
        slave_id: u8,
        cfg: modbussim_core::jitter::JitterConfig,
    ) {
        let connections = self.connections.clone();
        self.rt.spawn(async move {
            let conns = connections.read().await;
            let Some(entry) = conns.iter().find(|e| e.id == conn_id) else { return };
            let conn = entry.connection.read().await;
            let mut devs = conn.devices.write().await;
            if let Some(dev) = devs.get_mut(&slave_id) {
                dev.jitter = cfg;
            }
        });
    }

    fn open_add_device_modal(&mut self, conn_id: String) {
        let next_slave_id = self
            .conn_snapshot
            .iter()
            .find(|s| s.id == conn_id)
            .map(|s| {
                let mut ids: Vec<u8> = s.devices.iter().map(|d| d.slave_id).collect();
                ids.sort();
                (1u8..=247)
                    .find(|id| !ids.contains(id))
                    .unwrap_or(1)
            })
            .unwrap_or(1);

        self.add_device_modal = Some(AddDeviceModalState {
            conn_id,
            slave_id: next_slave_id,
            name: format!("从站 {}", next_slave_id),
            init_mode: DeviceInitMode::Default,
            max_address: 20000,
            busy: false,
        });
    }

    fn submit_add_device(&mut self, ctx: egui::Context) {
        let Some(state) = self.add_device_modal.as_mut() else { return };
        if state.busy { return; }
        if state.max_address > u16::MAX as u32 {
            self.last_error = Some("max_address 超过 65535".to_string());
            return;
        }
        state.busy = true;

        let conn_id = state.conn_id.clone();
        let slave_id = state.slave_id;
        let name = state.name.clone();
        let init_mode = state.init_mode;
        let max_addr = state.max_address as u16;

        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == conn_id)
                .map(|e| e.connection.clone());
            let Some(conn_arc) = conn_arc else {
                let _ = tx.send(UiEvent::Error(format!("连接 {conn_id} 未找到")));
                return;
            };
            let device = match init_mode {
                DeviceInitMode::Empty => SlaveDevice::new(slave_id, name.clone()),
                DeviceInitMode::Default => {
                    SlaveDevice::with_default_registers(slave_id, name.clone(), max_addr)
                }
                DeviceInitMode::Random => {
                    SlaveDevice::with_random_registers(slave_id, name.clone(), max_addr)
                }
            };
            let snap = DeviceSnapshot {
                slave_id,
                name: name.clone(),
                counts: RegCounts::from_device(&device),
                expanded: true,
            };
            let conn = conn_arc.read().await;
            match conn.add_device(device).await {
                Ok(()) => {
                    let _ = tx.send(UiEvent::DeviceAdded { conn_id, device: snap });
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("新增从站失败: {e}")));
                }
            }
        });
        self.add_device_modal = None;
        ctx.request_repaint();
    }

    fn remove_device(&self, conn_id: String, slave_id: u8, ctx: egui::Context) {
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == conn_id)
                .map(|e| e.connection.clone());
            let Some(conn_arc) = conn_arc else {
                let _ = tx.send(UiEvent::Error(format!("连接 {conn_id} 未找到")));
                return;
            };
            let conn = conn_arc.read().await;
            match conn.remove_device(slave_id).await {
                Ok(_) => {
                    let _ = tx.send(UiEvent::DeviceRemoved { conn_id, slave_id });
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("删除从站失败: {e}")));
                }
            }
        });
        ctx.request_repaint();
    }

    fn open_batch_modal(&mut self, conn_id: String, slave_id: u8, reg_type: RegisterType) {
        self.batch_modal = Some(BatchModalState {
            conn_id,
            slave_id,
            start_addr: 0,
            end_addr: 99,
            reg_type,
            data_type: match reg_type {
                RegisterType::Coil | RegisterType::DiscreteInput => DataType::Bool,
                _ => DataType::UInt16,
            },
            endian: Endian::Big,
            name_prefix: String::new(),
            busy: false,
        });
    }

    fn submit_batch_add(&mut self, ctx: egui::Context) {
        let Some(state) = self.batch_modal.as_mut() else { return };
        if state.busy { return; }
        if state.end_addr < state.start_addr {
            self.last_error = Some("结束地址必须 ≥ 起始地址".to_string());
            return;
        }
        let count = state.end_addr - state.start_addr + 1;
        if count > 50_000 {
            self.last_error = Some(format!("范围过大（最多 50000，当前 {count}）"));
            return;
        }
        if state.end_addr > u16::MAX as u32 {
            self.last_error = Some("地址超过 65535".to_string());
            return;
        }
        state.busy = true;

        let conn_id = state.conn_id.clone();
        let slave_id = state.slave_id;
        let reg_type = state.reg_type;
        let data_type = state.data_type;
        let endian = state.endian;
        let name_prefix = state.name_prefix.clone();
        let start = state.start_addr as u16;
        let end = state.end_addr as u16;

        let connections = self.connections.clone();
        let tx = self.events_tx.clone();

        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == conn_id)
                .map(|e| e.connection.clone());
            let Some(conn_arc) = conn_arc else {
                let _ = tx.send(UiEvent::Error(format!("连接 {conn_id} 未找到")));
                return;
            };

            let stride = data_type.register_count().max(1);
            let mut added = 0usize;

            let new_counts = {
                let conn = conn_arc.read().await;
                let mut devs = conn.devices.write().await;
                let Some(dev) = devs.get_mut(&slave_id) else {
                    let _ = tx.send(UiEvent::Error(format!("从站 {slave_id} 未找到")));
                    return;
                };
                let mut addr = start as u32;
                while addr <= end as u32 {
                    let def = RegisterDef {
                        address: addr as u16,
                        register_type: reg_type,
                        data_type,
                        endian,
                        name: if name_prefix.is_empty() {
                            String::new()
                        } else {
                            format!("{}_{}", name_prefix, addr)
                        },
                        comment: String::new(),
                    };
                    dev.register_map.ensure_from_def(&def);
                    dev.register_defs.push(def);
                    added += 1;
                    addr += stride as u32;
                }
                RegCounts::from_device(dev)
            };

            let _ = tx.send(UiEvent::DeviceCountsUpdated {
                conn_id: conn_id.clone(),
                slave_id,
                counts: new_counts,
            });
            let _ = tx.send(UiEvent::Info(format!(
                "批量添加完成：{}，共 {added} 个条目",
                conn_id
            )));
        });

        ctx.request_repaint();
    }

    /// Try to refresh the register view cache from the authoritative state.
    /// Uses sync `try_read` on the async RwLocks — skip this frame if contended.
    /// Clones the target HashMap into an Arc (cheap: shares buckets via Arc,
    /// no per-cell copy in the UI render path).
    fn refresh_reg_view(&mut self) {
        let Selection::RegisterGroup { conn_id, slave_id, reg_type } = self.selection.clone() else {
            self.reg_view = None;
            self.reg_view_last_refresh = None;
            return;
        };

        // Throttle: if the current cache matches and last refresh was recent, skip clone.
        let cache_matches = self
            .reg_view
            .as_ref()
            .map(|v| v.conn_id == conn_id && v.slave_id == slave_id && v.reg_type == reg_type)
            .unwrap_or(false);
        if cache_matches {
            if let Some(t) = self.reg_view_last_refresh {
                if t.elapsed().as_millis() < self.reg_view_refresh_interval_ms as u128 {
                    return;
                }
            }
        }

        let Ok(entries) = self.connections.try_read() else { return };
        let Some(entry) = entries.iter().find(|e| e.id == conn_id) else {
            self.reg_view = None;
            return;
        };
        let Ok(conn) = entry.connection.try_read() else { return };
        let Ok(devs) = conn.devices.try_read() else { return };
        let Some(dev) = devs.get(&slave_id) else {
            self.reg_view = None;
            return;
        };

        let map = &dev.register_map;
        let (u16_map, bool_map, row_count) = match reg_type {
            RegisterType::HoldingRegister => {
                let m = map.holding_registers.clone();
                let rc = m.keys().copied().max().map(|k| k as usize + 1).unwrap_or(0);
                (Some(Arc::new(m)), None, rc)
            }
            RegisterType::InputRegister => {
                let m = map.input_registers.clone();
                let rc = m.keys().copied().max().map(|k| k as usize + 1).unwrap_or(0);
                (Some(Arc::new(m)), None, rc)
            }
            RegisterType::Coil => {
                let m = map.coils.clone();
                let rc = m.keys().copied().max().map(|k| k as usize + 1).unwrap_or(0);
                (None, Some(Arc::new(m)), rc)
            }
            RegisterType::DiscreteInput => {
                let m = map.discrete_inputs.clone();
                let rc = m.keys().copied().max().map(|k| k as usize + 1).unwrap_or(0);
                (None, Some(Arc::new(m)), rc)
            }
        };

        let row_count = row_count.min(self.reg_row_limit.max(1));

        // Build a (name, comment) lookup from register_defs for this reg_type.
        let mut defs_map: std::collections::HashMap<u16, (String, String)> =
            std::collections::HashMap::new();
        for d in &dev.register_defs {
            if d.register_type == reg_type
                && (!d.name.is_empty() || !d.comment.is_empty())
            {
                defs_map.insert(d.address, (d.name.clone(), d.comment.clone()));
            }
        }

        self.reg_view = Some(RegViewCache {
            conn_id,
            slave_id,
            reg_type,
            row_count,
            u16_map,
            bool_map,
            defs: Arc::new(defs_map),
        });
        self.reg_view_last_refresh = Some(Instant::now());
    }

    fn allocate_connection(&self) -> (String, String) {
        let n = self.next_conn_seq.fetch_add(1, Ordering::Relaxed);
        let id = format!("slave_{}", n);
        let label = format!("TCP {}:{}", self.new_host.trim(), self.new_port.trim());
        (id, label)
    }

    fn spawn_create_tcp(&self, id: String, label: String, host: String, port: u16) {
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        self.rt.spawn(async move {
            let log_collector = Arc::new(LogCollector::new());
            let connection = SlaveConnection::new(Transport::Tcp { host, port })
                .with_log_collector(log_collector.clone());
            let device = SlaveDevice::with_default_registers(1, "从站 1", 20000);
            let device_snap = DeviceSnapshot {
                slave_id: device.slave_id,
                name: device.name.clone(),
                counts: RegCounts::from_device(&device),
                expanded: true,
            };
            if let Err(e) = connection.add_device(device).await {
                let _ = tx.send(UiEvent::Error(format!("添加默认设备失败: {e}")));
                return;
            }
            connections.write().await.push(SlaveConnectionEntry {
                id: id.clone(),
                label: label.clone(),
                connection: Arc::new(RwLock::new(connection)),
                log_collector,
            });
            let _ = tx.send(UiEvent::ConnectionCreated {
                id,
                label,
                devices: vec![device_snap],
            });
        });
    }

    fn create_tcp_connection(&mut self, ctx: egui::Context) {
        let host = self.new_host.trim().to_string();
        let port: u16 = match self.new_port.trim().parse() {
            Ok(p) => p,
            Err(_) => {
                self.last_error = Some(format!("无效端口: {}", self.new_port));
                return;
            }
        };
        let (id, label) = self.allocate_connection();
        self.spawn_create_tcp(id, label, host, port);
        ctx.request_repaint();
    }

    /// Attach a counter data source to the first auto-created slave (slave_id=1)
    /// at the given holding register address. Useful for smoke-testing the
    /// runner without going through the modal UI.
    pub fn auto_add_counter(&mut self, addr: u16) {
        // Wait 500 ms asynchronously for auto_start_tcp to register slave_1.
        let sources = self.data_sources.clone();
        let seq = self.next_source_id.clone();
        self.rt.spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let id = seq.fetch_add(1, Ordering::Relaxed);
            let cfg = DataSourceConfig {
                source: DsKind::Counter.default_source(),
                update_interval_ms: 200,
            };
            sources.lock().await.push(ActiveSource {
                id,
                conn_id: "slave_1".to_string(),
                slave_id: 1,
                reg_type: RegisterType::HoldingRegister,
                addr,
                enabled: true,
                state: DataSourceState::new(cfg),
                last_output: None,
            });
        });
    }

    /// Create+start immediately (invoked by `--auto-tcp` CLI arg).
    pub fn auto_start_tcp(&mut self, host: String, port: u16) {
        let (id, label) = {
            let n = self.next_conn_seq.fetch_add(1, Ordering::Relaxed);
            let id = format!("slave_{}", n);
            let label = format!("TCP {}:{}", host, port);
            (id, label)
        };
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        let id_task = id.clone();
        let label_task = label.clone();

        self.rt.spawn(async move {
            let log_collector = Arc::new(LogCollector::new());
            let connection = SlaveConnection::new(Transport::Tcp { host, port })
                .with_log_collector(log_collector.clone());
            let device = SlaveDevice::with_default_registers(1, "从站 1", 20000);
            let device_snap = DeviceSnapshot {
                slave_id: device.slave_id,
                name: device.name.clone(),
                counts: RegCounts::from_device(&device),
                expanded: true,
            };
            if let Err(e) = connection.add_device(device).await {
                let _ = tx.send(UiEvent::Error(format!("添加默认设备失败: {e}")));
                return;
            }
            let conn_arc = Arc::new(RwLock::new(connection));
            connections.write().await.push(SlaveConnectionEntry {
                id: id_task.clone(),
                label: label_task.clone(),
                connection: conn_arc.clone(),
                log_collector,
            });
            let _ = tx.send(UiEvent::ConnectionCreated {
                id: id_task.clone(),
                label: label_task,
                devices: vec![device_snap],
            });
            let start_result = {
                let mut guard = conn_arc.write().await;
                guard.start().await
            };
            match start_result {
                Ok(()) => {
                    let _ = tx.send(UiEvent::ConnectionStarted(id_task));
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("自动启动失败: {e}")));
                }
            }
        });
    }

    fn start_connection(&self, id: &str, ctx: egui::Context) {
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        let id = id.to_string();
        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.connection.clone());
            let Some(conn_arc) = conn_arc else {
                let _ = tx.send(UiEvent::Error(format!("连接 {id} 未找到")));
                return;
            };
            let result = {
                let mut guard = conn_arc.write().await;
                guard.start().await
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(UiEvent::ConnectionStarted(id));
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("启动失败: {e}")));
                }
            }
        });
        ctx.request_repaint();
    }

    fn stop_connection(&self, id: &str, ctx: egui::Context) {
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        let id = id.to_string();
        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.connection.clone());
            let Some(conn_arc) = conn_arc else {
                let _ = tx.send(UiEvent::Error(format!("连接 {id} 未找到")));
                return;
            };
            let result = {
                let mut guard = conn_arc.write().await;
                guard.stop().await
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(UiEvent::ConnectionStopped(id));
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("停止失败: {e}")));
                }
            }
        });
        ctx.request_repaint();
    }

    fn remove_connection(&mut self, id: &str, ctx: egui::Context) {
        let selection_refers = matches!(&self.selection,
            Selection::Connection(s) | Selection::Device { conn_id: s, .. } | Selection::RegisterGroup { conn_id: s, .. } if s == id);
        if selection_refers {
            self.selection = Selection::None;
        }
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        let id = id.to_string();
        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.connection.clone());
            if let Some(conn_arc) = conn_arc {
                let is_running = conn_arc.read().await.state() == ConnectionState::Running;
                if is_running {
                    let mut guard = conn_arc.write().await;
                    let _ = guard.stop().await;
                }
            }
            connections.write().await.retain(|e| e.id != id);
            let _ = tx.send(UiEvent::ConnectionRemoved(id));
        });
        ctx.request_repaint();
    }

    fn build_project(&self) -> SlaveProject {
        let mut proj = SlaveProject::new();
        for snap in &self.conn_snapshot {
            let (host, port) = self
                .connections
                .try_read()
                .ok()
                .and_then(|list| {
                    list.iter().find(|e| e.id == snap.id).and_then(|e| {
                        e.connection.try_read().ok().map(|c| match &c.transport {
                            Transport::Tcp { host, port } => (host.clone(), *port),
                            _ => ("0.0.0.0".to_string(), 502),
                        })
                    })
                })
                .unwrap_or_else(|| ("0.0.0.0".to_string(), 502));

            let devices: Vec<SlaveDeviceSave> = snap
                .devices
                .iter()
                .map(|d| SlaveDeviceSave {
                    slave_id: d.slave_id,
                    name: d.name.clone(),
                    max_address: if d.counts.holding_registers > 0 {
                        Some((d.counts.holding_registers.saturating_sub(1)) as u16)
                    } else {
                        None
                    },
                })
                .collect();

            proj.connections.push(SlaveConnectionSave {
                label: snap.label.clone(),
                tcp: TcpSpec { host, port },
                devices,
            });
        }
        proj
    }

    fn save_project(&self, ctx: egui::Context) {
        let proj = self.build_project();
        let tx = self.events_tx.clone();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.rt.spawn(async move {
            let Some(path) = rfd::AsyncFileDialog::new()
                .set_file_name(&format!("slave_{}.modbusproj", ts))
                .add_filter("ModbusProj", &["modbusproj"])
                .save_file()
                .await
            else {
                return;
            };
            match serialize_slave(&proj) {
                Ok(json) => match tokio::fs::write(path.path(), json).await {
                    Ok(()) => {
                        let _ = tx.send(UiEvent::Info(format!("已保存：{}", path.path().display())));
                    }
                    Err(e) => {
                        let _ = tx.send(UiEvent::Error(format!("写入失败: {e}")));
                    }
                },
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("序列化失败: {e}")));
                }
            }
        });
        ctx.request_repaint();
    }

    fn load_project(&mut self, ctx: egui::Context) {
        let tx = self.events_tx.clone();
        let rt = self.rt.clone();
        let connections_arc = self.connections.clone();
        let next_seq = self.next_conn_seq.clone();
        let ctx2 = ctx.clone();
        rt.spawn(async move {
            let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter("ModbusProj", &["modbusproj"])
                .pick_file()
                .await
            else {
                return;
            };
            let text = match tokio::fs::read_to_string(file.path()).await {
                Ok(t) => t,
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("读取失败: {e}")));
                    return;
                }
            };
            let project = match deserialize_slave(&text) {
                Ok(p) => p,
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("解析失败: {e}")));
                    return;
                }
            };
            for c in project.connections {
                let id = format!("slave_{}", next_seq.fetch_add(1, Ordering::Relaxed));
                let label = c.label.clone();
                let log_collector = Arc::new(LogCollector::new());
                let connection = SlaveConnection::new(Transport::Tcp {
                    host: c.tcp.host.clone(),
                    port: c.tcp.port,
                })
                .with_log_collector(log_collector.clone());

                let mut device_snapshots = Vec::new();
                for ds in &c.devices {
                    let device = match ds.max_address {
                        Some(max) => {
                            SlaveDevice::with_default_registers(ds.slave_id, ds.name.clone(), max)
                        }
                        None => SlaveDevice::new(ds.slave_id, ds.name.clone()),
                    };
                    let snap = DeviceSnapshot {
                        slave_id: ds.slave_id,
                        name: ds.name.clone(),
                        counts: RegCounts::from_device(&device),
                        expanded: true,
                    };
                    if let Err(e) = connection.add_device(device).await {
                        let _ = tx.send(UiEvent::Error(format!("加载设备失败: {e}")));
                        continue;
                    }
                    device_snapshots.push(snap);
                }

                connections_arc.write().await.push(SlaveConnectionEntry {
                    id: id.clone(),
                    label: label.clone(),
                    connection: Arc::new(RwLock::new(connection)),
                    log_collector,
                });
                let _ = tx.send(UiEvent::ConnectionCreated {
                    id,
                    label,
                    devices: device_snapshots,
                });
            }
            let _ = tx.send(UiEvent::Info(format!("已加载：{}", file.path().display())));
            ctx2.request_repaint();
        });
    }

    fn drain_events(&mut self) {
        while let Ok(ev) = self.events_rx.try_recv() {
            match ev {
                UiEvent::ConnectionCreated { id, label, devices } => {
                    self.conn_snapshot.push(ConnSnapshot {
                        id,
                        label,
                        state: ConnectionState::Stopped,
                        expanded: true,
                        devices,
                    });
                }
                UiEvent::ConnectionStarted(id) => {
                    if let Some(s) = self.conn_snapshot.iter_mut().find(|s| s.id == id) {
                        s.state = ConnectionState::Running;
                    }
                }
                UiEvent::ConnectionStopped(id) => {
                    if let Some(s) = self.conn_snapshot.iter_mut().find(|s| s.id == id) {
                        s.state = ConnectionState::Stopped;
                    }
                }
                UiEvent::ConnectionRemoved(id) => {
                    self.conn_snapshot.retain(|s| s.id != id);
                }
                UiEvent::DeviceCountsUpdated { conn_id, slave_id, counts } => {
                    if let Some(s) = self.conn_snapshot.iter_mut().find(|s| s.id == conn_id) {
                        if let Some(d) = s.devices.iter_mut().find(|d| d.slave_id == slave_id) {
                            d.counts = counts;
                        }
                    }
                    // Force cache refresh next frame.
                    self.reg_view_last_refresh = None;
                }
                UiEvent::DeviceAdded { conn_id, device } => {
                    if let Some(s) = self.conn_snapshot.iter_mut().find(|s| s.id == conn_id) {
                        s.devices.push(device);
                        s.devices.sort_by_key(|d| d.slave_id);
                    }
                }
                UiEvent::DeviceRemoved { conn_id, slave_id } => {
                    if let Some(s) = self.conn_snapshot.iter_mut().find(|s| s.id == conn_id) {
                        s.devices.retain(|d| d.slave_id != slave_id);
                    }
                    let refs = matches!(&self.selection,
                        Selection::Device { conn_id: c, slave_id: sid }
                        | Selection::RegisterGroup { conn_id: c, slave_id: sid, .. }
                            if c == &conn_id && *sid == slave_id);
                    if refs {
                        self.selection = Selection::Connection(conn_id);
                        self.pending_edits.clear();
                    }
                }
                UiEvent::Info(msg) => {
                    self.status_msg = Some(msg);
                }
                UiEvent::Error(msg) => self.last_error = Some(msg),
            }
        }
    }
}

enum TreeAction {
    ToggleConn(String),
    ToggleDevice { conn_id: String, slave_id: u8 },
    SelectConn(String),
    SelectDevice { conn_id: String, slave_id: u8 },
    SelectGroup { conn_id: String, slave_id: u8, reg_type: RegisterType },
    StartConn(String),
    StopConn(String),
    RemoveConn(String),
    Create,
}

const REG_GROUPS: &[(RegisterType, &str)] = &[
    (RegisterType::Coil, "FC01 线圈"),
    (RegisterType::DiscreteInput, "FC02 离散输入"),
    (RegisterType::InputRegister, "FC04 输入寄存器"),
    (RegisterType::HoldingRegister, "FC03 保持寄存器"),
];

/// Which interpretation to apply when rendering u16 register values.
/// U16/I16 are single-word; F32/U32/I32 consume 2 words; F64 consumes 4 words.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ValueDisplayMode {
    U16,
    I16,
    F32(Endian),
    U32(Endian),
    I32(Endian),
    F64(F64Order),
}

impl ValueDisplayMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::U16 => "U16",
            Self::I16 => "I16",
            Self::F32(Endian::Big) => "F32 AB CD",
            Self::F32(Endian::Little) => "F32 CD AB",
            Self::F32(Endian::MidBig) => "F32 BA DC",
            Self::F32(Endian::MidLittle) => "F32 DC BA",
            Self::U32(Endian::Big) => "U32 AB CD",
            Self::U32(Endian::Little) => "U32 CD AB",
            Self::U32(Endian::MidBig) => "U32 BA DC",
            Self::U32(Endian::MidLittle) => "U32 DC BA",
            Self::I32(Endian::Big) => "I32 AB CD",
            Self::I32(Endian::Little) => "I32 CD AB",
            Self::I32(Endian::MidBig) => "I32 BA DC",
            Self::I32(Endian::MidLittle) => "I32 DC BA",
            Self::F64(F64Order::Abcdefgh) => "F64 ABCDEFGH",
            Self::F64(F64Order::Hgfedcba) => "F64 HGFEDCBA",
            Self::F64(F64Order::Badcfehg) => "F64 BADCFEHG",
            Self::F64(F64Order::Ghefcdab) => "F64 GHEFCDAB",
        }
    }
    pub fn is_multi_word(&self) -> bool {
        matches!(self, Self::F32(_) | Self::U32(_) | Self::I32(_) | Self::F64(_))
    }
    pub fn stride(&self) -> usize {
        match self {
            Self::F64(_) => 4,
            Self::F32(_) | Self::U32(_) | Self::I32(_) => 2,
            _ => 1,
        }
    }
}

const DISPLAY_MODES: &[ValueDisplayMode] = &[
    ValueDisplayMode::U16,
    ValueDisplayMode::I16,
    ValueDisplayMode::F32(Endian::Big),
    ValueDisplayMode::F32(Endian::Little),
    ValueDisplayMode::F32(Endian::MidBig),
    ValueDisplayMode::F32(Endian::MidLittle),
    ValueDisplayMode::U32(Endian::Big),
    ValueDisplayMode::U32(Endian::Little),
    ValueDisplayMode::I32(Endian::Big),
    ValueDisplayMode::I32(Endian::Little),
    ValueDisplayMode::F64(F64Order::Abcdefgh),
    ValueDisplayMode::F64(F64Order::Hgfedcba),
    ValueDisplayMode::F64(F64Order::Badcfehg),
    ValueDisplayMode::F64(F64Order::Ghefcdab),
];

const DATA_TYPES: &[DataType] = &[
    DataType::Bool,
    DataType::UInt16,
    DataType::Int16,
    DataType::UInt32,
    DataType::Int32,
    DataType::Float32,
];

const ENDIANS: &[Endian] = &[Endian::Big, Endian::Little, Endian::MidBig, Endian::MidLittle];

fn selection_conn_id(s: &Selection) -> Option<&str> {
    match s {
        Selection::Connection(id)
        | Selection::Device { conn_id: id, .. }
        | Selection::RegisterGroup { conn_id: id, .. } => Some(id.as_str()),
        Selection::None => None,
    }
}

fn reg_type_label(rt: RegisterType) -> &'static str {
    REG_GROUPS
        .iter()
        .find(|(r, _)| *r == rt)
        .map(|(_, l)| *l)
        .unwrap_or("?")
}

fn data_type_label(dt: DataType) -> &'static str {
    match dt {
        DataType::Bool => "Bool",
        DataType::UInt16 => "UInt16",
        DataType::Int16 => "Int16",
        DataType::UInt32 => "UInt32",
        DataType::Int32 => "Int32",
        DataType::Float32 => "Float32",
    }
}

fn endian_label(e: Endian) -> &'static str {
    match e {
        Endian::Big => "AB CD",
        Endian::Little => "CD AB",
        Endian::MidBig => "BA DC",
        Endian::MidLittle => "DC BA",
    }
}

impl SlaveApp {
    fn render_tree(&mut self, ui: &mut egui::Ui) -> Option<TreeAction> {
        let mut action: Option<TreeAction> = None;

        for snap in &self.conn_snapshot {
            let conn_is_selected = matches!(&self.selection, Selection::Connection(c) if c == &snap.id);
            let state_tag = match snap.state {
                ConnectionState::Running => "运行中",
                ConnectionState::Stopped => "已停止",
            };

            ui.horizontal(|ui| {
                let arrow = if snap.expanded { "▼" } else { "▶" };
                if ui.small_button(arrow).clicked() {
                    action = Some(TreeAction::ToggleConn(snap.id.clone()));
                }
                if ui
                    .selectable_label(conn_is_selected, format!("{} [{}]", snap.label, state_tag))
                    .clicked()
                {
                    action = Some(TreeAction::SelectConn(snap.id.clone()));
                }
            });
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                match snap.state {
                    ConnectionState::Stopped => {
                        if ui.small_button("启动").clicked() {
                            action = Some(TreeAction::StartConn(snap.id.clone()));
                        }
                    }
                    ConnectionState::Running => {
                        if ui.small_button("停止").clicked() {
                            action = Some(TreeAction::StopConn(snap.id.clone()));
                        }
                    }
                }
                if ui.small_button("删除").clicked() {
                    action = Some(TreeAction::RemoveConn(snap.id.clone()));
                }
            });

            if snap.expanded {
                for dev in &snap.devices {
                    let dev_is_selected = matches!(&self.selection,
                        Selection::Device { conn_id, slave_id }
                            if conn_id == &snap.id && *slave_id == dev.slave_id);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        let arrow = if dev.expanded { "▼" } else { "▶" };
                        if ui.small_button(arrow).clicked() {
                            action = Some(TreeAction::ToggleDevice {
                                conn_id: snap.id.clone(),
                                slave_id: dev.slave_id,
                            });
                        }
                        if ui
                            .selectable_label(
                                dev_is_selected,
                                format!("从站 {} · {}", dev.slave_id, dev.name),
                            )
                            .clicked()
                        {
                            action = Some(TreeAction::SelectDevice {
                                conn_id: snap.id.clone(),
                                slave_id: dev.slave_id,
                            });
                        }
                    });
                    if dev.expanded {
                        for (reg_type, label) in REG_GROUPS {
                            let grp_is_selected = matches!(&self.selection,
                                Selection::RegisterGroup { conn_id, slave_id, reg_type: rt }
                                    if conn_id == &snap.id && *slave_id == dev.slave_id && rt == reg_type);
                            ui.horizontal(|ui| {
                                ui.add_space(32.0);
                                let text = format!("{} ({})", label, dev.counts.count_for(*reg_type));
                                if ui.selectable_label(grp_is_selected, text).clicked() {
                                    action = Some(TreeAction::SelectGroup {
                                        conn_id: snap.id.clone(),
                                        slave_id: dev.slave_id,
                                        reg_type: *reg_type,
                                    });
                                }
                            });
                        }
                    }
                }
            }
            ui.separator();
        }

        action
    }

    fn apply_tree_action(&mut self, action: TreeAction, ctx: &egui::Context) {
        match action {
            TreeAction::ToggleConn(id) => {
                if let Some(s) = self.conn_snapshot.iter_mut().find(|s| s.id == id) {
                    s.expanded = !s.expanded;
                }
            }
            TreeAction::ToggleDevice { conn_id, slave_id } => {
                if let Some(s) = self.conn_snapshot.iter_mut().find(|s| s.id == conn_id) {
                    if let Some(d) = s.devices.iter_mut().find(|d| d.slave_id == slave_id) {
                        d.expanded = !d.expanded;
                    }
                }
            }
            TreeAction::SelectConn(id) => {
                self.selection = Selection::Connection(id);
                self.pending_edits.clear();
                self.selected_addrs.clear();
                self.click_anchor = None;
            }
            TreeAction::SelectDevice { conn_id, slave_id } => {
                self.selection = Selection::Device { conn_id, slave_id };
                self.pending_edits.clear();
                self.selected_addrs.clear();
                self.click_anchor = None;
            }
            TreeAction::SelectGroup { conn_id, slave_id, reg_type } => {
                self.selection = Selection::RegisterGroup { conn_id, slave_id, reg_type };
                self.pending_edits.clear();
                self.selected_addrs.clear();
                self.click_anchor = None;
            }
            TreeAction::StartConn(id) => self.start_connection(&id, ctx.clone()),
            TreeAction::StopConn(id) => self.stop_connection(&id, ctx.clone()),
            TreeAction::RemoveConn(id) => self.remove_connection(&id, ctx.clone()),
            TreeAction::Create => self.create_tcp_connection(ctx.clone()),
        }
    }

    fn render_add_device_modal(&mut self, ctx: &egui::Context) {
        if self.add_device_modal.is_none() { return; }

        enum Act { Submit, Close }
        let mut act: Option<Act> = None;
        let mut is_open = true;

        egui::Window::new("新增从站")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut is_open)
            .show(ctx, |ui| {
                let Some(st) = self.add_device_modal.as_mut() else { return };
                egui::Grid::new("add_device_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("连接");
                        ui.label(&st.conn_id);
                        ui.end_row();

                        ui.label("从站 ID (1-247)");
                        let mut sid = st.slave_id as u32;
                        ui.add(egui::DragValue::new(&mut sid).range(1..=247));
                        st.slave_id = sid as u8;
                        ui.end_row();

                        ui.label("名称");
                        ui.text_edit_singleline(&mut st.name);
                        ui.end_row();

                        ui.label("初始化模式");
                        egui::ComboBox::from_id_salt("add_device_init")
                            .selected_text(match st.init_mode {
                                DeviceInitMode::Empty => "空",
                                DeviceInitMode::Default => "默认值（全 0）",
                                DeviceInitMode::Random => "随机",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut st.init_mode, DeviceInitMode::Empty, "空");
                                ui.selectable_value(&mut st.init_mode, DeviceInitMode::Default, "默认值（全 0）");
                                ui.selectable_value(&mut st.init_mode, DeviceInitMode::Random, "随机");
                            });
                        ui.end_row();

                        if !matches!(st.init_mode, DeviceInitMode::Empty) {
                            ui.label("最大地址");
                            ui.add(egui::DragValue::new(&mut st.max_address).range(0..=65535));
                            ui.end_row();
                        }
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.add_enabled(!st.busy, egui::Button::new("确认")).clicked() {
                        act = Some(Act::Submit);
                    }
                    if ui.button("取消").clicked() {
                        act = Some(Act::Close);
                    }
                    if st.busy { ui.spinner(); }
                });
            });

        if !is_open { act = Some(Act::Close); }
        match act {
            Some(Act::Submit) => self.submit_add_device(ctx.clone()),
            Some(Act::Close) => { self.add_device_modal = None; }
            None => {}
        }
    }

    fn render_batch_modal(&mut self, ctx: &egui::Context) {
        if self.batch_modal.is_none() { return; }

        enum ModalAction { Submit, Close }
        let mut action: Option<ModalAction> = None;
        let mut is_open = true;

        egui::Window::new("批量添加寄存器")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut is_open)
            .show(ctx, |ui| {
                let Some(state) = self.batch_modal.as_mut() else { return };
                egui::Grid::new("batch_add_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("连接 / 从站");
                        ui.label(format!("{} · 从站 {}", state.conn_id, state.slave_id));
                        ui.end_row();

                        ui.label("起始地址");
                        ui.add(egui::DragValue::new(&mut state.start_addr).range(0..=65535));
                        ui.end_row();

                        ui.label("结束地址");
                        ui.add(egui::DragValue::new(&mut state.end_addr).range(0..=65535));
                        ui.end_row();

                        ui.label("寄存器类型");
                        egui::ComboBox::from_id_salt("batch_reg_type")
                            .selected_text(reg_type_label(state.reg_type))
                            .show_ui(ui, |ui| {
                                for (rt, label) in REG_GROUPS {
                                    ui.selectable_value(&mut state.reg_type, *rt, *label);
                                }
                            });
                        ui.end_row();

                        ui.label("数据类型");
                        egui::ComboBox::from_id_salt("batch_data_type")
                            .selected_text(data_type_label(state.data_type))
                            .show_ui(ui, |ui| {
                                for dt in DATA_TYPES {
                                    ui.selectable_value(&mut state.data_type, *dt, data_type_label(*dt));
                                }
                            });
                        ui.end_row();

                        ui.label("字节序");
                        egui::ComboBox::from_id_salt("batch_endian")
                            .selected_text(endian_label(state.endian))
                            .show_ui(ui, |ui| {
                                for e in ENDIANS {
                                    ui.selectable_value(&mut state.endian, *e, endian_label(*e));
                                }
                            });
                        ui.end_row();

                        ui.label("名称前缀（可选）");
                        ui.text_edit_singleline(&mut state.name_prefix);
                        ui.end_row();
                    });

                let stride = state.data_type.register_count().max(1) as u32;
                let raw_count = if state.end_addr >= state.start_addr {
                    (state.end_addr - state.start_addr) / stride + 1
                } else { 0 };
                ui.separator();
                ui.horizontal(|ui| {
                    if raw_count == 0 {
                        ui.colored_label(egui::Color32::RED, "范围无效");
                    } else if raw_count > 50_000 {
                        ui.colored_label(egui::Color32::RED, format!("范围过大（最多 50000，当前 {raw_count}）"));
                    } else {
                        ui.label(format!("将添加 {raw_count} 个条目"));
                    }
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(!state.busy && raw_count > 0 && raw_count <= 50_000,
                                      egui::Button::new("确认添加")).clicked() {
                        action = Some(ModalAction::Submit);
                    }
                    if ui.button("取消").clicked() {
                        action = Some(ModalAction::Close);
                    }
                    if state.busy { ui.spinner(); }
                });
            });

        if !is_open { action = Some(ModalAction::Close); }
        match action {
            Some(ModalAction::Submit) => {
                self.submit_batch_add(ctx.clone());
                self.batch_modal = None;
            }
            Some(ModalAction::Close) => { self.batch_modal = None; }
            None => {}
        }
    }

    fn render_main(&mut self, ui: &mut egui::Ui) {
        // Snapshot what's selected; later we might mutate self.batch_modal.
        let selection = self.selection.clone();
        match &selection {
            Selection::None => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.heading(format!("{}  ModbusSlave", icons::CPU));
                    uikit::caption(ui, self.flavor, "从左侧创建或选中一个连接 / 设备 / 寄存器组。");
                });
            }
            Selection::Connection(id) => {
                let exists = self.conn_snapshot.iter().any(|s| &s.id == id);
                if exists {
                    let (label, state, dev_count) = {
                        let s = self.conn_snapshot.iter().find(|s| &s.id == id).unwrap();
                        (s.label.clone(), s.state, s.devices.len())
                    };
                    ui.horizontal(|ui| {
                        ui.heading(format!("{}  {}", icons::BROADCAST, label));
                        let (txt, color) = match state {
                            ConnectionState::Running => (
                                format!("{}  运行中", icons::CIRCLE),
                                theme::success(self.flavor),
                            ),
                            ConnectionState::Stopped => (
                                format!("{}  已停止", icons::PAUSE),
                                theme::subtext(self.flavor),
                            ),
                        };
                        uikit::status_pill(ui, txt, color);
                    });
                    uikit::caption(ui, self.flavor, format!("设备数: {}", dev_count));
                    ui.add_space(8.0);
                    if uikit::primary_button(
                        ui,
                        self.flavor,
                        format!("{}  新增从站", icons::PLUS_CIRCLE),
                    )
                    .clicked()
                    {
                        self.open_add_device_modal(id.clone());
                    }
                } else {
                    ui.label("连接已不存在。");
                }
            }
            Selection::Device { conn_id, slave_id } => {
                let conn = self.conn_snapshot.iter().find(|s| &s.id == conn_id);
                match conn.and_then(|c| c.devices.iter().find(|d| d.slave_id == *slave_id)) {
                    Some(d) => {
                        ui.heading(format!("从站 {} · {}", d.slave_id, d.name));
                        egui::Grid::new("dev_summary")
                            .num_columns(2)
                            .spacing([16.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("FC01 线圈");
                                ui.label(d.counts.coils.to_string());
                                ui.end_row();
                                ui.label("FC02 离散输入");
                                ui.label(d.counts.discrete_inputs.to_string());
                                ui.end_row();
                                ui.label("FC04 输入寄存器");
                                ui.label(d.counts.input_registers.to_string());
                                ui.end_row();
                                ui.label("FC03 保持寄存器");
                                ui.label(d.counts.holding_registers.to_string());
                                ui.end_row();
                            });
                        ui.add_space(8.0);
                        let flavor = self.flavor;
                        ui.horizontal(|ui| {
                            if uikit::primary_button(
                                ui,
                                flavor,
                                format!("{}  批量添加", icons::PLUS_CIRCLE),
                            )
                            .clicked()
                            {
                                self.open_batch_modal(
                                    conn_id.clone(),
                                    *slave_id,
                                    RegisterType::HoldingRegister,
                                );
                            }
                            if uikit::danger_button(
                                ui,
                                flavor,
                                format!("{}  删除从站", icons::TRASH),
                            )
                            .clicked()
                            {
                                self.remove_device(conn_id.clone(), *slave_id, ui.ctx().clone());
                            }
                        });

                        // ----- Jitter card (run-time register mutation for master-stress) -----
                        ui.separator();
                        // Pass empty icon: phosphor font isn't actually embedded, so any
                        // phosphor glyph renders as a placeholder box (same as "✕" did).
                        uikit::section_heading(ui, "", "寄存器抖动（压测）");

                        let cur_jitter: modbussim_core::jitter::JitterConfig = {
                            let conns = self.connections.blocking_read();
                            let entry = conns.iter().find(|e| e.id == *conn_id);
                            let jitter_opt = entry.and_then(|e| {
                                let conn = e.connection.blocking_read();
                                let devs = conn.devices.blocking_read();
                                devs.get(slave_id).map(|d| d.jitter.clone())
                            });
                            jitter_opt.unwrap_or_default()
                        };
                        let mut new_jitter = cur_jitter.clone();
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut new_jitter.enabled, "启用");
                        });
                        let mut interval = new_jitter.interval_ms as i32;
                        let mut rate = new_jitter.mutation_rate as i32;
                        let mut delta = new_jitter.delta_percent as i32;
                        egui::Grid::new("jitter_grid")
                            .num_columns(2)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                ui.label("周期");
                                ui.add(
                                    egui::Slider::new(&mut interval, 100..=5000)
                                        .suffix(" ms"),
                                );
                                ui.end_row();
                                ui.label("变位率");
                                ui.add(
                                    egui::Slider::new(&mut rate, 0..=100)
                                        .suffix(" %"),
                                );
                                ui.end_row();
                                ui.label("漂移幅度");
                                ui.add(
                                    egui::Slider::new(&mut delta, 0..=100)
                                        .suffix(" %"),
                                );
                                ui.end_row();
                            });
                        new_jitter.interval_ms = interval as u64;
                        new_jitter.mutation_rate = rate as u8;
                        new_jitter.delta_percent = delta as u8;
                        ui.horizontal(|ui| {
                            ui.label("影响范围");
                            ui.checkbox(&mut new_jitter.affect_coils, "线圈");
                            ui.checkbox(&mut new_jitter.affect_discrete, "离散");
                            ui.checkbox(&mut new_jitter.affect_holding, "保持");
                            ui.checkbox(&mut new_jitter.affect_input, "输入");
                        });

                        if new_jitter != cur_jitter {
                            self.set_device_jitter(conn_id.clone(), *slave_id, new_jitter);
                        }

                        ui.separator();
                        uikit::section_heading(ui, icons::GEAR, "数据源");

                        // Snapshot sources belonging to this device.
                        let snap: Vec<(u64, u16, RegisterType, String, u64, bool)> =
                            match self.data_sources.try_lock() {
                                Ok(g) => g
                                    .iter()
                                    .filter(|s| s.conn_id == *conn_id && s.slave_id == *slave_id)
                                    .map(|s| {
                                        (
                                            s.id,
                                            s.addr,
                                            s.reg_type,
                                            source_short_desc(&s.state.config.source),
                                            s.state.config.update_interval_ms,
                                            s.enabled,
                                        )
                                    })
                                    .collect(),
                                Err(_) => Vec::new(),
                            };

                        let mut toggle_id: Option<u64> = None;
                        let mut remove_id: Option<u64> = None;
                        if snap.is_empty() {
                            ui.label("（无）");
                        } else {
                            egui::Grid::new("ds_list")
                                .num_columns(6)
                                .spacing([10.0, 4.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.strong("类型区");
                                    ui.strong("地址");
                                    ui.strong("源");
                                    ui.strong("间隔 (ms)");
                                    ui.strong("启用");
                                    ui.strong("");
                                    ui.end_row();
                                    for (sid, addr, rt, desc, ivl, en) in &snap {
                                        ui.monospace(match rt {
                                            RegisterType::HoldingRegister => "FC03",
                                            RegisterType::InputRegister => "FC04",
                                            _ => "?",
                                        });
                                        ui.monospace(format!("{}", addr));
                                        ui.label(desc);
                                        ui.monospace(format!("{}", ivl));
                                        let mut en_mut = *en;
                                        if ui.checkbox(&mut en_mut, "").clicked() {
                                            toggle_id = Some(*sid);
                                        }
                                        if ui.small_button("删除").clicked() {
                                            remove_id = Some(*sid);
                                        }
                                        ui.end_row();
                                    }
                                });
                        }

                        let mut do_add = false;
                        ui.horizontal(|ui| {
                            ui.label("+");
                            ui.add(
                                egui::DragValue::new(&mut self.ds_add_addr)
                                    .range(0..=65535)
                                    .prefix("地址 "),
                            );
                            egui::ComboBox::from_id_salt("ds_kind")
                                .selected_text(self.ds_add_kind.label())
                                .show_ui(ui, |ui| {
                                    for k in [
                                        DsKind::Counter,
                                        DsKind::Sine,
                                        DsKind::Sawtooth,
                                        DsKind::Triangle,
                                        DsKind::Random,
                                        DsKind::Fixed,
                                        DsKind::CsvPlayback,
                                    ] {
                                        ui.selectable_value(&mut self.ds_add_kind, k, k.label());
                                    }
                                });
                            ui.add(
                                egui::DragValue::new(&mut self.ds_add_interval_ms)
                                    .range(50..=60_000)
                                    .suffix(" ms"),
                            );
                            if ui.button("添加到 FC03").clicked() {
                                do_add = true;
                            }
                        });

                        if let Some(sid) = toggle_id {
                            self.toggle_data_source(sid);
                        }
                        if let Some(sid) = remove_id {
                            self.remove_data_source(sid);
                        }
                        if do_add {
                            self.add_data_source(
                                conn_id.clone(),
                                *slave_id,
                                RegisterType::HoldingRegister,
                                self.ds_add_addr as u16,
                                self.ds_add_kind,
                                self.ds_add_interval_ms,
                            );
                        }
                    }
                    None => {
                        ui.label("设备不存在。");
                    }
                }
            }
            Selection::RegisterGroup { conn_id, slave_id, reg_type } => {
                let group_label = REG_GROUPS
                    .iter()
                    .find(|(rt, _)| rt == reg_type)
                    .map(|(_, l)| *l)
                    .unwrap_or("?");
                let flavor = self.flavor;
                let reg_icon = match reg_type {
                    RegisterType::Coil => icons::TOGGLE_LEFT,
                    RegisterType::DiscreteInput => icons::LIST_BULLETS,
                    RegisterType::InputRegister => icons::DATABASE,
                    RegisterType::HoldingRegister => icons::HARD_DRIVES,
                };
                let mut open_batch = false;
                let search_key = (conn_id.clone(), *slave_id, *reg_type);
                let mut search_text = self
                    .search_buf
                    .get(&search_key)
                    .cloned()
                    .unwrap_or_default();
                let mut want_focus = self.want_focus_search;
                uikit::region(
                    ui,
                    flavor,
                    theme::Layer::L1,
                    egui::Margin::symmetric(14.0, 10.0),
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.heading(format!("{}  {}", reg_icon, group_label));
                            uikit::caption(
                                ui,
                                flavor,
                                format!("连接 {} · 从站 {}", conn_id, slave_id),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if uikit::primary_button(
                                        ui,
                                        flavor,
                                        format!("{}  批量添加", icons::PLUS_CIRCLE),
                                    )
                                    .clicked()
                                    {
                                        open_batch = true;
                                    }
                                    ui.add_space(8.0);
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut search_text)
                                            .hint_text("地址 / 名称…")
                                            .desired_width(160.0),
                                    );
                                    if want_focus {
                                        resp.request_focus();
                                        // Also select-all so the next keystroke overwrites.
                                        if let Some(mut state) =
                                            egui::TextEdit::load_state(ui.ctx(), resp.id)
                                        {
                                            let cc = egui::text::CCursor::new(search_text.chars().count());
                                            state.cursor.set_char_range(Some(
                                                egui::text::CCursorRange::two(
                                                    egui::text::CCursor::new(0),
                                                    cc,
                                                ),
                                            ));
                                            state.store(ui.ctx(), resp.id);
                                        }
                                        want_focus = false;
                                    }
                                },
                            );
                        });
                    },
                );
                self.search_buf.insert(search_key.clone(), search_text.clone());
                self.want_focus_search = want_focus;
                let search_intent = parse_search_intent(&search_text);
                if open_batch {
                    self.open_batch_modal(conn_id.clone(), *slave_id, *reg_type);
                }

                let Some(view) = &self.reg_view else {
                    ui.label("正在加载…（或未选中有效组）");
                    return;
                };
                if view.conn_id != *conn_id
                    || view.slave_id != *slave_id
                    || view.reg_type != *reg_type
                {
                    ui.label("正在刷新…");
                    return;
                }

                let is_bool = matches!(
                    reg_type,
                    RegisterType::Coil | RegisterType::DiscreteInput
                );

                // 只有 16-bit 寄存器区（FC03 / FC04）支持多字节序显示；线圈区强制 U16。
                let mode = if is_bool {
                    ValueDisplayMode::U16
                } else {
                    self.reg_display_mode
                };

                ui.horizontal(|ui| {
                    ui.label(format!("共 {} 行", view.row_count));
                    if !is_bool {
                        ui.separator();
                        ui.label("格式");
                        egui::ComboBox::from_id_salt("reg_display_mode")
                            .selected_text(mode.label())
                            .show_ui(ui, |ui| {
                                for m in DISPLAY_MODES {
                                    ui.selectable_value(&mut self.reg_display_mode, *m, m.label());
                                }
                            });
                    }
                });
                if !is_bool && mode.is_multi_word() {
                    ui.label("多字格式 · 只读显示；要编辑请切回 U16");
                }

                let row_h = 20.0;
                let reg_type_v = *reg_type;
                let conn_id_for_writes = conn_id.clone();
                let slave_id_v = *slave_id;
                let pending = &mut self.pending_edits;
                let selected_addrs = &self.selected_addrs;
                let mut writes: Vec<(u16, u16)> = Vec::new();
                // Collected row clicks with modifier state — applied after
                // the TableBuilder closure releases borrows.
                let mut row_clicks: Vec<(u16, egui::Modifiers)> = Vec::new();

                // Left = table (~62% wide, fills vertical), right = ValuePanel.
                // StripBuilder is the right primitive here — a plain
                // ui.horizontal + allocate_ui collapses to 0 height inside a
                // CentralPanel and draws the debug red warning box.
                use egui_extras::{Size, StripBuilder};
                StripBuilder::new(ui)
                    .size(Size::relative(0.62).at_least(360.0))
                    .size(Size::exact(8.0))
                    .size(Size::remainder().at_least(260.0))
                    .horizontal(|mut strip| {
                        strip.cell(|ui| {
                            uikit::region(ui, flavor, theme::Layer::L2, egui::Margin::symmetric(8.0, 6.0), |ui| {
                if !is_bool && mode.is_multi_word() {
                    let stride = mode.stride();
                    let group_rows = view.row_count / stride;
                    let endian = match mode {
                        ValueDisplayMode::F32(e)
                        | ValueDisplayMode::U32(e)
                        | ValueDisplayMode::I32(e) => e,
                        _ => Endian::Big,
                    };
                    let f64_order = match mode {
                        ValueDisplayMode::F64(o) => o,
                        _ => F64Order::Abcdefgh,
                    };
                    let avail_h = ui.available_height();
                    TableBuilder::new(ui)
                        .striped(true)
                        .resizable(true)
                        .max_scroll_height(avail_h)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(Column::exact(110.0))
                        .column(Column::exact(220.0))
                        .column(Column::exact(200.0))
                        .column(Column::remainder())
                        .header(22.0, |mut h| {
                            h.col(|ui| { ui.strong("地址"); });
                            h.col(|ui| { ui.strong(mode.label()); });
                            h.col(|ui| { ui.strong("Raw (Hex)"); });
                            h.col(|ui| { ui.strong(""); });
                        })
                        .body(|body| {
                            body.rows(row_h, group_rows, |mut row| {
                                let base = row.index() as u16 * stride as u16;
                                // Gather stride u16 values.
                                let mut ws: Vec<u16> = Vec::with_capacity(stride);
                                let mut all_present = true;
                                for i in 0..stride as u16 {
                                    match view
                                        .u16_map
                                        .as_ref()
                                        .and_then(|m| m.get(&(base + i)).copied())
                                    {
                                        Some(v) => ws.push(v),
                                        None => { all_present = false; break; }
                                    }
                                }
                                row.col(|ui| {
                                    let sel = (0..stride as u16)
                                        .any(|i| selected_addrs.contains(&(base + i)));
                                    let label = if stride == 4 {
                                        format!("{}..{}", base, base + 3)
                                    } else {
                                        format!("{}..{}", base, base + 1)
                                    };
                                    let resp = ui.add(egui::SelectableLabel::new(
                                        sel,
                                        egui::RichText::new(label).monospace(),
                                    ));
                                    if resp.clicked() {
                                        row_clicks.push((base, resp.ctx.input(|i| i.modifiers)));
                                    }
                                });
                                row.col(|ui| {
                                    if !all_present {
                                        ui.monospace("—");
                                        return;
                                    }
                                    let text = match mode {
                                        ValueDisplayMode::F32(_) => {
                                            let d = decode_value(&ws, DataType::Float32, endian)
                                                .unwrap_or(f64::NAN);
                                            format!("{:.6}", d as f32)
                                        }
                                        ValueDisplayMode::U32(_) => {
                                            let d = decode_value(&ws, DataType::UInt32, endian)
                                                .unwrap_or(f64::NAN);
                                            format!("{}", d as u32)
                                        }
                                        ValueDisplayMode::I32(_) => {
                                            let d = decode_value(&ws, DataType::Int32, endian)
                                                .unwrap_or(f64::NAN);
                                            format!("{}", d as i32)
                                        }
                                        ValueDisplayMode::F64(_) => {
                                            let v = value_panel::decode_f64(&ws, f64_order);
                                            if v.is_finite() {
                                                format!("{:.9}", v)
                                            } else {
                                                "NaN / Inf".to_string()
                                            }
                                        }
                                        _ => "?".to_string(),
                                    };
                                    ui.monospace(text);
                                });
                                row.col(|ui| {
                                    if !all_present {
                                        ui.monospace("");
                                        return;
                                    }
                                    let joined = ws
                                        .iter()
                                        .map(|w| format!("{:04X}", w))
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    ui.monospace(joined);
                                });
                                row.col(|_| {});
                            });
                        });
                } else {
                    // Apply search intent: Jump sets a one-shot scroll_to + highlight;
                    // Filter builds a reduced addr list that drives body.rows.
                    let filtered_addrs: Option<Vec<u16>> = match &search_intent {
                        SearchIntent::None | SearchIntent::Jump(_) => None,
                        SearchIntent::Filter(q) => {
                            let ndl = q.as_str();
                            let v: Vec<u16> = (0..view.row_count as u16)
                                .filter(|a| a.to_string().contains(ndl))
                                .collect();
                            Some(v)
                        }
                    };
                    let mut scroll_to_row: Option<usize> = None;
                    if let SearchIntent::Jump(addr) = search_intent {
                        if (addr as usize) < view.row_count {
                            // Only start a new highlight if the target changed — prevents
                            // re-scroll on every keystroke once the user stops typing.
                            let new_key = (conn_id.clone(), *slave_id, *reg_type, addr);
                            let same = self
                                .highlight
                                .as_ref()
                                .map(|h| (h.0.clone(), h.1, h.2, h.3) == new_key)
                                .unwrap_or(false);
                            if !same {
                                self.highlight =
                                    Some((new_key.0, new_key.1, new_key.2, addr, Instant::now()));
                                scroll_to_row = Some(addr as usize);
                            }
                        }
                    }

                    if let Some(list) = &filtered_addrs {
                        if list.is_empty() {
                            ui.add_space(8.0);
                            uikit::caption(ui, flavor, "无匹配寄存器");
                            return;
                        }
                    }

                    let body_row_count = filtered_addrs.as_ref().map(|v| v.len()).unwrap_or(view.row_count);
                    let avail_h = ui.available_height();
                    let mut tb = TableBuilder::new(ui)
                        .striped(true)
                        .resizable(true)
                        .max_scroll_height(avail_h)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(Column::exact(80.0))
                        .column(Column::exact(110.0))
                        .column(Column::exact(100.0))
                        .column(Column::exact(140.0))
                        .column(Column::remainder());
                    if let Some(idx) = scroll_to_row {
                        tb = tb.scroll_to_row(idx, Some(egui::Align::Center));
                    }
                    let highlight_addr: Option<u16> = self.highlight.as_ref().and_then(|h| {
                        if &h.0 == conn_id && h.1 == *slave_id && h.2 == *reg_type {
                            Some(h.3)
                        } else {
                            None
                        }
                    });
                    tb
                        .header(22.0, |mut header| {
                            header.col(|ui| { ui.strong("地址"); });
                            header.col(|ui| {
                                ui.strong(if is_bool { "布尔" } else { mode.label() });
                            });
                            header.col(|ui| {
                                ui.strong(if is_bool { "—" } else { "Hex" });
                            });
                            header.col(|ui| {
                                ui.strong(if is_bool { "—" } else { "Binary" });
                            });
                            header.col(|ui| { ui.strong(""); });
                        })
                        .body(|body| {
                            body.rows(row_h, body_row_count, |mut row| {
                                let addr = if let Some(list) = &filtered_addrs {
                                    list[row.index()]
                                } else {
                                    row.index() as u16
                                };
                                if Some(addr) == highlight_addr {
                                    row.set_selected(true);
                                }
                                row.col(|ui| {
                                    let sel = selected_addrs.contains(&addr);
                                    let resp = ui.add(egui::SelectableLabel::new(
                                        sel,
                                        egui::RichText::new(format!("{}", addr)).monospace(),
                                    ));
                                    if resp.clicked() {
                                        row_clicks.push((addr, resp.ctx.input(|i| i.modifiers)));
                                    }
                                });

                                let cache_u16 = view
                                    .u16_map
                                    .as_ref()
                                    .and_then(|m| m.get(&addr).copied())
                                    .unwrap_or(0);
                                let cache_bool = view
                                    .bool_map
                                    .as_ref()
                                    .and_then(|m| m.get(&addr).copied())
                                    .unwrap_or(false);
                                let key = (reg_type_v, addr);

                                if is_bool {
                                    row.col(|ui| {
                                        let current = pending
                                            .get(&key)
                                            .map(|v| *v != 0)
                                            .unwrap_or(cache_bool);
                                        // Self-drawn ○/● + ON/OFF — no Button frame.
                                        let (glyph, label, dot_color, text_color) = if current {
                                            (
                                                "●",
                                                "ON",
                                                theme::success(flavor),
                                                egui::Color32::from_rgb(220, 223, 228),
                                            )
                                        } else {
                                            (
                                                "○",
                                                "OFF",
                                                egui::Color32::from_rgb(139, 143, 151),
                                                egui::Color32::from_rgb(139, 143, 151),
                                            )
                                        };
                                        let resp = ui
                                            .horizontal(|ui| {
                                                ui.spacing_mut().item_spacing.x = 6.0;
                                                ui.add(egui::Label::new(
                                                    egui::RichText::new(glyph)
                                                        .color(dot_color)
                                                        .size(13.0),
                                                ).sense(egui::Sense::click()));
                                                ui.add(egui::Label::new(
                                                    egui::RichText::new(label)
                                                        .color(text_color)
                                                        .size(12.0)
                                                        .monospace(),
                                                ).sense(egui::Sense::click()))
                                            })
                                            .inner;
                                        if resp.clicked() {
                                            let new_val = !current;
                                            writes.push((addr, if new_val { 1 } else { 0 }));
                                            pending.remove(&key);
                                        }
                                    });
                                    row.col(|_| {});
                                    row.col(|_| {});
                                    row.col(|_| {});
                                } else {
                                    row.col(|ui| {
                                        let (min_i, max_i) = match mode {
                                            ValueDisplayMode::I16 => (i16::MIN as i32, i16::MAX as i32),
                                            _ => (0, u16::MAX as i32),
                                        };
                                        let cache_as_display = match mode {
                                            ValueDisplayMode::I16 => cache_u16 as i16 as i32,
                                            _ => cache_u16 as i32,
                                        };
                                        let mut tmp: i32 = pending
                                            .get(&key)
                                            .copied()
                                            .unwrap_or(cache_as_display);
                                        let resp = ui.add(
                                            egui::DragValue::new(&mut tmp).range(min_i..=max_i),
                                        );
                                        let active = resp.has_focus()
                                            || resp.dragged()
                                            || resp.drag_started()
                                            || resp.gained_focus();
                                        if active {
                                            pending.insert(key, tmp);
                                        } else if let Some(prev) = pending.remove(&key) {
                                            let v = match mode {
                                                ValueDisplayMode::I16 => {
                                                    prev.clamp(i16::MIN as i32, i16::MAX as i32)
                                                        as i16 as u16
                                                }
                                                _ => prev.clamp(0, 65535) as u16,
                                            };
                                            if v != cache_u16 {
                                                writes.push((addr, v));
                                            }
                                        }
                                    });
                                    let display_u16 = pending
                                        .get(&key)
                                        .copied()
                                        .map(|v| match mode {
                                            ValueDisplayMode::I16 => {
                                                v.clamp(i16::MIN as i32, i16::MAX as i32) as i16
                                                    as u16
                                            }
                                            _ => v.clamp(0, 65535) as u16,
                                        })
                                        .unwrap_or(cache_u16);
                                    row.col(|ui| {
                                        ui.monospace(format_u16(display_u16, U16Format::Hex));
                                    });
                                    row.col(|ui| {
                                        ui.monospace(format_u16(display_u16, U16Format::Binary));
                                    });
                                    row.col(|_| {});
                                }
                            });
                        });
                }

                            }); // end left region
                        }); // end StripBuilder left cell
                        strip.cell(|_ui| { });
                        strip.cell(|ui| {
                            uikit::region(ui, flavor, theme::Layer::L1, egui::Margin::symmetric(12.0, 10.0), |ui| {
                            let mut selected_vals: Vec<u16> = Vec::new();
                            let mut base: Option<u16> = None;
                            // Only take up to 4 selected, in address order, and
                            // require them to be contiguous for multi-word analysis.
                            let ordered: Vec<u16> = selected_addrs.iter().copied().take(4).collect();
                            for (i, a) in ordered.iter().enumerate() {
                                if i == 0 {
                                    base = Some(*a);
                                } else if *a != ordered[i - 1] + 1 {
                                    // Non-contiguous: stop collecting so ValuePanel
                                    // only shows formats it can compute safely.
                                    break;
                                }
                                if let Some(v) = view.u16_map.as_ref().and_then(|m| m.get(a).copied()) {
                                    selected_vals.push(v);
                                } else if let Some(b) = view.bool_map.as_ref().and_then(|m| m.get(a).copied()) {
                                    selected_vals.push(if b { 1 } else { 0 });
                                }
                            }
                            if let Some(vp_writes) = value_panel::render(ui, flavor, &selected_vals, base) {
                                for w in vp_writes {
                                    writes.push(w);
                                }
                            }
                            }); // end right region
                        });
                    }); // end StripBuilder horizontal

                // Apply row clicks to selected_addrs with modifier semantics.
                // Plain / ctrl click sets (or updates) click_anchor.
                // Shift click always ranges [anchor, addr] — anchor never moves
                // during consecutive shift clicks, so extending a range works.
                if !row_clicks.is_empty() {
                    for (addr, modifiers) in row_clicks {
                        if modifiers.shift {
                            let anchor = self.click_anchor.unwrap_or(addr);
                            let (a, b) = if anchor <= addr { (anchor, addr) } else { (addr, anchor) };
                            self.selected_addrs.clear();
                            for x in a..=b {
                                self.selected_addrs.insert(x);
                                if self.selected_addrs.len() >= 16 { break; }
                            }
                        } else if modifiers.command || modifiers.ctrl {
                            if !self.selected_addrs.remove(&addr) {
                                self.selected_addrs.insert(addr);
                            }
                            self.click_anchor = Some(addr);
                        } else {
                            self.selected_addrs.clear();
                            self.selected_addrs.insert(addr);
                            self.click_anchor = Some(addr);
                        }
                    }
                }

                for (addr, val) in writes {
                    self.commit_write(
                        conn_id_for_writes.clone(),
                        slave_id_v,
                        reg_type_v,
                        addr,
                        val,
                        ui.ctx().clone(),
                    );
                }
            }
        }
    }
}

impl SlaveApp {
    fn render_log_panel(&mut self, ctx: &egui::Context) {
        let action = log_panel::render(
            ctx,
            self.flavor,
            &mut self.log_state,
            &self.log_cache,
            self.log_cache_conn_id.as_deref(),
        );
        match action {
            LogPanelAction::Clear => self.clear_logs_for_selection(),
            LogPanelAction::Export => self.export_logs_for_selection(ctx.clone()),
            LogPanelAction::Close => self.log_state.open = false,
            LogPanelAction::None => {}
        }
    }
}

impl eframe::App for SlaveApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "flavor_v3", &self.flavor);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        self.refresh_reg_view();
        self.refresh_log_cache();

        // Cmd+F / Ctrl+F focuses the RegisterGroup search box (if that view is
        // active). COMMAND maps to ⌘ on macOS / Ctrl elsewhere. Consume up-front
        // so the window system doesn't swallow it.
        let find_shortcut = egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::F,
        );
        if ctx.input_mut(|i| i.consume_shortcut(&find_shortcut))
            && matches!(self.selection, Selection::RegisterGroup { .. })
        {
            self.want_focus_search = true;
        }

        // Fade highlights: drop any stale ones older than 2s.
        if let Some((_, _, _, _, t)) = &self.highlight {
            if t.elapsed().as_secs_f32() > 2.0 {
                self.highlight = None;
            } else {
                ctx.request_repaint();
            }
        }

        let mut do_save = false;
        let mut do_load = false;
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("文件", |ui| {
                    if ui.button("保存工程…").clicked() {
                        do_save = true;
                        ui.close_menu();
                    }
                    if ui.button("加载工程…").clicked() {
                        do_load = true;
                        ui.close_menu();
                    }
                });
                ui.menu_button("视图", |ui| {
                    if ui.checkbox(&mut self.log_state.open, "显示日志面板").clicked() {
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.label("主题 (Catppuccin)");
                    for f in [Flavor::Mocha, Flavor::Macchiato, Flavor::Frappe, Flavor::Latte] {
                        if ui.radio_value(&mut self.flavor, f, f.label()).clicked() {
                            theme::apply(ctx, self.flavor);
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    let zoom = ctx.zoom_factor();
                    if ui.button(format!("放大  ({:.0}%)", zoom * 100.0)).clicked() {
                        ctx.set_zoom_factor((zoom + 0.1).min(3.0));
                    }
                    if ui.button("缩小").clicked() {
                        ctx.set_zoom_factor((zoom - 0.1).max(0.5));
                    }
                    if ui.button("重置缩放").clicked() {
                        ctx.set_zoom_factor(1.0);
                    }
                });
                ui.menu_button("帮助", |ui| {
                    ui.label("ModbusSlave (egui) · 开发预览");
                    ui.hyperlink_to(
                        "GitHub",
                        "https://github.com/kelsoprotein-lab/ModbusSim",
                    );
                });
            });
        });

        let mut tree_action: Option<TreeAction> = None;

        egui::SidePanel::left("connections")
            .resizable(true)
            .default_width(320.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::none()
                    .fill(theme::bg_of(self.flavor, theme::Layer::L0))
                    .inner_margin(egui::Margin::symmetric(12.0, 10.0)),
            )
            .show(ctx, |ui| {
                ui.heading("连接");
                ui.separator();

                ui.collapsing("新建 TCP 连接", |ui| {
                    egui::Grid::new("new_tcp_form")
                        .num_columns(2)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            ui.label("Host");
                            ui.text_edit_singleline(&mut self.new_host);
                            ui.end_row();
                            ui.label("Port");
                            ui.text_edit_singleline(&mut self.new_port);
                            ui.end_row();
                        });
                    if ui.button("创建").clicked() {
                        tree_action = Some(TreeAction::Create);
                    }
                });

                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(a) = self.render_tree(ui) {
                        tree_action = Some(a);
                    }
                });
            });

        let mut clear_error = false;
        let mut clear_status = false;
        egui::TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .show(ctx, |ui| {
                if let Some(err) = &self.last_error {
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::RED, err);
                        if ui.small_button("清除").clicked() {
                            clear_error = true;
                        }
                    });
                } else if let Some(msg) = &self.status_msg {
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::from_rgb(60, 140, 60), msg);
                        if ui.small_button("清除").clicked() {
                            clear_status = true;
                        }
                    });
                } else {
                    ui.label("就绪");
                }
            });
        if clear_error { self.last_error = None; }
        if clear_status { self.status_msg = None; }

        self.render_log_panel(ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(theme::bg_of(self.flavor, theme::Layer::L1))
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0)),
            )
            .show(ctx, |ui| {
                self.render_main(ui);
            });

        // Modals
        self.render_batch_modal(ctx);
        self.render_add_device_modal(ctx);

        if let Some(a) = tree_action {
            self.apply_tree_action(a, ctx);
        }

        if do_save {
            self.save_project(ctx.clone());
        }
        if do_load {
            self.load_project(ctx.clone());
        }

        if !self.events_rx.is_empty() {
            ctx.request_repaint();
        }

        // Poll for register value updates while a group is selected.
        if matches!(self.selection, Selection::RegisterGroup { .. }) {
            ctx.request_repaint_after(std::time::Duration::from_millis(
                self.reg_view_refresh_interval_ms,
            ));
        }
    }
}
