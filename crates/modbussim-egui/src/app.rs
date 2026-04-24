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
use modbussim_core::transport::{SlaveTlsConfig, Transport};
use modbussim_ui_shared::format::{format_u16, U16Format};
use modbussim_ui_shared::hero_anim::{show_welcome_hero, HeroPulseFeed};
use modbussim_ui_shared::i18n::{tr, tr1, tr2, Lang};
use modbussim_ui_shared::icons;
use modbussim_ui_shared::log_panel::{self, LogPanelAction, LogPanelState};
use modbussim_ui_shared::project::{
    deserialize_slave, serialize_slave, SlaveConnectionSave, SlaveDeviceSave, SlaveProject,
    TcpSpec, TlsSpec,
};
use modbussim_ui_shared::theme::{self, Flavor};
use modbussim_ui_shared::ui as uikit;
use modbussim_ui_shared::value_panel::{self, F64Order};
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
    pub fn label(&self, lang: Lang) -> &'static str {
        tr(
            lang,
            match self {
                DsKind::Counter => "ds.kind.counter",
                DsKind::Sine => "ds.kind.sine",
                DsKind::Sawtooth => "ds.kind.sawtooth",
                DsKind::Triangle => "ds.kind.triangle",
                DsKind::Random => "ds.kind.random",
                DsKind::Fixed => "ds.kind.fixed",
                DsKind::CsvPlayback => "ds.kind.csv",
            },
        )
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
            DsKind::Random => DataSource::Random { min: 0, max: 65535 },
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
        DataSource::Sine {
            amplitude,
            frequency,
            offset,
            ..
        } => {
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
    /// 延迟格式化:drain_events 阶段 tr1(self.lang, key, arg);
    /// 用于 async 上下文里构造用户可见的运行时错误,不需要在 spawn 里捕获 lang。
    ErrorKey {
        key: &'static str,
        arg: String,
    },
    InfoKey {
        key: &'static str,
        arg: String,
    },
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

/// 空状态 Hero 动画的心跳采样器。每 100ms 从所有连接的 LogCollector
/// 读取最近 1s 的 TX/RX 条数，归一化到 0..=1 作为振幅乘子。
struct HeroPulseState {
    /// 最近一次采样得到的总条数（所有连接聚合）。
    recent_count: u32,
    /// 上次采样时刻；None 表示从未采样。
    last_sample: Option<std::time::Instant>,
}

impl HeroPulseState {
    const WINDOW: std::time::Duration = std::time::Duration::from_secs(1);
    const SAMPLE_EVERY: std::time::Duration = std::time::Duration::from_millis(100);
    const SATURATION: u32 = 40;

    fn new() -> Self {
        Self {
            recent_count: 0,
            last_sample: None,
        }
    }

    /// 若距上次采样已 >= 100ms，则重新遍历所有 connection 的 log_collector
    /// 累加最近 1s 的条数，返回归一化后的振幅（未经 gain 混合）。
    fn sample(&mut self, connections: &SharedConnections) -> f32 {
        let due = self
            .last_sample
            .map_or(true, |t| t.elapsed() >= Self::SAMPLE_EVERY);
        if due {
            if let Ok(entries) = connections.try_read() {
                let total: usize = entries
                    .iter()
                    .filter_map(|e| e.log_collector.try_count_within(Self::WINDOW))
                    .sum();
                self.recent_count = total.min(u32::MAX as usize) as u32;
                self.last_sample = Some(std::time::Instant::now());
            }
            // 写锁冲突 / 初次读失败：沿用上次 recent_count
        }
        amp_from_counts(self.recent_count)
    }

    fn feed(&mut self, connections: &SharedConnections) -> HeroPulseFeed {
        HeroPulseFeed {
            amp: self.sample(connections),
            has_error: false,
            disabled: false,
        }
    }
}

/// 把"最近 1s 总条数"归一化到 0..=1。SATURATION=40 即约 40 条/秒满振。
fn amp_from_counts(total: u32) -> f32 {
    (total as f32 / HeroPulseState::SATURATION as f32).clamp(0.0, 1.0)
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
    show_new_tcp_dialog: bool,
    // —— 新建连接的 TLS 表单字段 ——
    new_use_tls: bool,
    /// PEM cert 路径（与 pkcs12_file 互斥；同时填则 PKCS#12 优先）
    new_cert_file: String,
    new_key_file: String,
    new_ca_file: String,
    new_require_client_cert: bool,
    new_pkcs12_file: String,
    new_pkcs12_password: String,
    /// 删除连接二次确认状态：(conn_id, 首次点击时刻)。
    /// 3 秒内同一连接再次点删除按钮 → 真删；否则按钮 label 自动恢复。
    pending_delete: Option<(String, std::time::Instant)>,
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
    pub lang: Lang,

    // Register table display mode
    reg_display_mode: ValueDisplayMode,

    // Log panel
    log_state: LogPanelState,
    log_cache: Vec<LogEntry>,
    log_cache_conn_id: Option<String>,
    log_last_refresh: Option<Instant>,

    // Welcome-screen hero animation pulse sampler.
    hero_pulse: HeroPulseState,

    /// 右侧值解析面板是否显示。默认 true 兼容现有用户预期；可由
    /// `V` 快捷键 / 视图菜单 / 工具栏 toggle 关掉以让表格全宽。
    pub value_parse_open: bool,
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
    pub fn new(rt: Arc<Runtime>, flavor: Flavor, lang: Lang) -> Self {
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
                    if srcs.is_empty() {
                        continue;
                    }
                    let now = Instant::now();
                    // Collect writes first so we don't hold the sources lock while touching connections.
                    let mut updates: Vec<(String, u8, RegisterType, u16, u16)> = Vec::new();
                    for s in srcs.iter_mut() {
                        if !s.enabled {
                            continue;
                        }
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
                    if updates.is_empty() {
                        continue;
                    }
                    let conns = connections.read().await;
                    for (conn_id, slave_id, rtype, addr, v) in updates {
                        let Some(entry) = conns.iter().find(|e| e.id == conn_id) else {
                            continue;
                        };
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
                                RegisterType::Coil => {
                                    // u16 → bool：非零视为 true，零为 false。
                                    dev.register_map.coils.insert(addr, v != 0);
                                }
                                RegisterType::DiscreteInput => {
                                    dev.register_map.discrete_inputs.insert(addr, v != 0);
                                }
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
            show_new_tcp_dialog: false,
            new_use_tls: false,
            new_cert_file: String::new(),
            new_key_file: String::new(),
            new_ca_file: String::new(),
            new_require_client_cert: false,
            new_pkcs12_file: String::new(),
            new_pkcs12_password: String::new(),
            pending_delete: None,
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
            lang,
            reg_display_mode: ValueDisplayMode::U16,
            log_state: LogPanelState::new(),
            log_cache: Vec::new(),
            log_cache_conn_id: None,
            log_last_refresh: None,
            hero_pulse: HeroPulseState::new(),
            value_parse_open: true,
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

        let Ok(entries) = self.connections.try_read() else {
            return;
        };
        let Some(entry) = entries.iter().find(|e| e.id == id) else {
            return;
        };
        let Some(mut all) = entry.log_collector.try_get_all() else {
            return;
        };
        let start = all.len().saturating_sub(500);
        self.log_cache = all.drain(start..).collect();
        self.log_cache_conn_id = Some(id.to_string());
        self.log_last_refresh = Some(Instant::now());
    }

    fn clear_logs_for_selection(&self) {
        let Some(id) = selection_conn_id(&self.selection) else {
            return;
        };
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
        let Some(id) = selection_conn_id(&self.selection) else {
            return;
        };
        let id = id.to_string();
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        self.rt.spawn(async move {
            let entries = connections.read().await;
            let Some(entry) = entries.iter().find(|e| e.id == id) else {
                let _ = tx.send(UiEvent::ErrorKey {
                    key: "err.conn_not_found_fmt",
                    arg: id.to_string(),
                });
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
                    let _ = tx.send(UiEvent::InfoKey {
                        key: "err.log_exported_fmt",
                        arg: path.path().display().to_string(),
                    });
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::ErrorKey {
                        key: "err.export_failed_fmt",
                        arg: e.to_string(),
                    });
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
                let _ = tx.send(UiEvent::ErrorKey {
                    key: "err.conn_not_found_fmt",
                    arg: conn_id.to_string(),
                });
                return;
            };
            let new_counts = {
                let conn = conn_arc.read().await;
                let mut devs = conn.devices.write().await;
                let Some(dev) = devs.get_mut(&slave_id) else {
                    let _ = tx.send(UiEvent::ErrorKey {
                        key: "err.slave_not_found_fmt",
                        arg: slave_id.to_string(),
                    });
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
            let Some(entry) = conns.iter().find(|e| e.id == conn_id) else {
                return;
            };
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
                (1u8..=247).find(|id| !ids.contains(id)).unwrap_or(1)
            })
            .unwrap_or(1);

        self.add_device_modal = Some(AddDeviceModalState {
            conn_id,
            slave_id: next_slave_id,
            name: tr1(self.lang, "slave.default_name_fmt", next_slave_id),
            init_mode: DeviceInitMode::Default,
            max_address: 20000,
            busy: false,
        });
    }

    fn submit_add_device(&mut self, ctx: egui::Context) {
        let Some(state) = self.add_device_modal.as_mut() else {
            return;
        };
        if state.busy {
            return;
        }
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
                let _ = tx.send(UiEvent::ErrorKey {
                    key: "err.conn_not_found_fmt",
                    arg: conn_id.to_string(),
                });
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
                    let _ = tx.send(UiEvent::DeviceAdded {
                        conn_id,
                        device: snap,
                    });
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::ErrorKey {
                        key: "err.add_slave_failed_fmt",
                        arg: e.to_string(),
                    });
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
                let _ = tx.send(UiEvent::ErrorKey {
                    key: "err.conn_not_found_fmt",
                    arg: conn_id.to_string(),
                });
                return;
            };
            let conn = conn_arc.read().await;
            match conn.remove_device(slave_id).await {
                Ok(_) => {
                    let _ = tx.send(UiEvent::DeviceRemoved { conn_id, slave_id });
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::ErrorKey {
                        key: "err.remove_slave_failed_fmt",
                        arg: e.to_string(),
                    });
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
        let Some(state) = self.batch_modal.as_mut() else {
            return;
        };
        if state.busy {
            return;
        }
        if state.end_addr < state.start_addr {
            self.last_error = Some("结束地址必须 ≥ 起始地址".to_string());
            return;
        }
        let count = state.end_addr - state.start_addr + 1;
        if count > 50_000 {
            self.last_error = Some(tr1(self.lang, "err.range_too_large_fmt", count));
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
                let _ = tx.send(UiEvent::ErrorKey {
                    key: "err.conn_not_found_fmt",
                    arg: conn_id.to_string(),
                });
                return;
            };

            let stride = data_type.register_count().max(1);
            let mut added = 0usize;

            let new_counts = {
                let conn = conn_arc.read().await;
                let mut devs = conn.devices.write().await;
                let Some(dev) = devs.get_mut(&slave_id) else {
                    let _ = tx.send(UiEvent::ErrorKey {
                        key: "err.slave_not_found_fmt",
                        arg: slave_id.to_string(),
                    });
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
        let Selection::RegisterGroup {
            conn_id,
            slave_id,
            reg_type,
        } = self.selection.clone()
        else {
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

        let Ok(entries) = self.connections.try_read() else {
            return;
        };
        let Some(entry) = entries.iter().find(|e| e.id == conn_id) else {
            self.reg_view = None;
            return;
        };
        let Ok(conn) = entry.connection.try_read() else {
            return;
        };
        let Ok(devs) = conn.devices.try_read() else {
            return;
        };
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
            if d.register_type == reg_type && (!d.name.is_empty() || !d.comment.is_empty()) {
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
        let proto = if self.new_use_tls { "TLS" } else { "TCP" };
        let label = format!(
            "{} {}:{}",
            proto,
            self.new_host.trim(),
            self.new_port.trim()
        );
        (id, label)
    }

    fn spawn_create_tcp(
        &self,
        id: String,
        label: String,
        host: String,
        port: u16,
        tls: Option<SlaveTlsConfig>,
    ) {
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        self.rt.spawn(async move {
            let log_collector = Arc::new(LogCollector::new());
            let transport = if tls.is_some() {
                Transport::TcpTls { host, port }
            } else {
                Transport::Tcp { host, port }
            };
            let mut connection =
                SlaveConnection::new(transport).with_log_collector(log_collector.clone());
            if let Some(cfg) = tls {
                connection = connection.with_tls_config(cfg);
            }
            let device = SlaveDevice::with_default_registers(1, "从站 1", 20000);
            let device_snap = DeviceSnapshot {
                slave_id: device.slave_id,
                name: device.name.clone(),
                counts: RegCounts::from_device(&device),
                expanded: true,
            };
            if let Err(e) = connection.add_device(device).await {
                let _ = tx.send(UiEvent::ErrorKey {
                    key: "err.add_default_device_failed_fmt",
                    arg: e.to_string(),
                });
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
                self.last_error = Some(tr1(self.lang, "err.invalid_port_fmt", &self.new_port));
                return;
            }
        };
        let tls = if self.new_use_tls {
            let has_pem =
                !self.new_cert_file.trim().is_empty() && !self.new_key_file.trim().is_empty();
            let has_pkcs12 = !self.new_pkcs12_file.trim().is_empty();
            if !has_pem && !has_pkcs12 {
                self.last_error =
                    Some("启用 TLS 需要填写 cert+key（PEM）或 pkcs12 文件路径".to_string());
                return;
            }
            Some(SlaveTlsConfig {
                enabled: true,
                cert_file: self.new_cert_file.trim().to_string(),
                key_file: self.new_key_file.trim().to_string(),
                ca_file: self.new_ca_file.trim().to_string(),
                require_client_cert: self.new_require_client_cert,
                pkcs12_file: self.new_pkcs12_file.trim().to_string(),
                pkcs12_password: self.new_pkcs12_password.clone(),
            })
        } else {
            None
        };
        let (id, label) = self.allocate_connection();
        self.spawn_create_tcp(id, label, host, port, tls);
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
                let _ = tx.send(UiEvent::ErrorKey {
                    key: "err.add_default_device_failed_fmt",
                    arg: e.to_string(),
                });
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
                    let _ = tx.send(UiEvent::ErrorKey {
                        key: "err.auto_start_failed_fmt",
                        arg: e.to_string(),
                    });
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
                let _ = tx.send(UiEvent::ErrorKey {
                    key: "err.conn_not_found_fmt",
                    arg: id.to_string(),
                });
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
                    let _ = tx.send(UiEvent::ErrorKey {
                        key: "err.start_failed_fmt",
                        arg: e.to_string(),
                    });
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
                let _ = tx.send(UiEvent::ErrorKey {
                    key: "err.conn_not_found_fmt",
                    arg: id.to_string(),
                });
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
                    let _ = tx.send(UiEvent::ErrorKey {
                        key: "err.stop_failed_fmt",
                        arg: e.to_string(),
                    });
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
            // 同时取出 host/port 与可选 TLS 配置，单次 try_read 完成
            let (host, port, tls) = self
                .connections
                .try_read()
                .ok()
                .and_then(|list| {
                    list.iter().find(|e| e.id == snap.id).and_then(|e| {
                        e.connection.try_read().ok().map(|c| {
                            let (h, p) = match &c.transport {
                                Transport::Tcp { host, port }
                                | Transport::TcpTls { host, port } => (host.clone(), *port),
                                _ => ("0.0.0.0".to_string(), 502),
                            };
                            let tls = if matches!(c.transport, Transport::TcpTls { .. })
                                && c.tls_config.enabled
                            {
                                Some(TlsSpec {
                                    cert_file: c.tls_config.cert_file.clone(),
                                    key_file: c.tls_config.key_file.clone(),
                                    ca_file: c.tls_config.ca_file.clone(),
                                    require_client_cert: c.tls_config.require_client_cert,
                                    pkcs12_file: c.tls_config.pkcs12_file.clone(),
                                    pkcs12_password: c.tls_config.pkcs12_password.clone(),
                                })
                            } else {
                                None
                            };
                            (h, p, tls)
                        })
                    })
                })
                .unwrap_or_else(|| ("0.0.0.0".to_string(), 502, None));

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
                tcp: TcpSpec { host, port, tls },
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
                .set_file_name(format!("slave_{}.modbusproj", ts))
                .add_filter("ModbusProj", &["modbusproj"])
                .save_file()
                .await
            else {
                return;
            };
            match serialize_slave(&proj) {
                Ok(json) => match tokio::fs::write(path.path(), json).await {
                    Ok(()) => {
                        let _ = tx.send(UiEvent::InfoKey {
                            key: "err.saved_fmt",
                            arg: path.path().display().to_string(),
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(UiEvent::ErrorKey {
                            key: "err.save_failed_fmt",
                            arg: e.to_string(),
                        });
                    }
                },
                Err(e) => {
                    let _ = tx.send(UiEvent::ErrorKey {
                        key: "err.serialize_failed_fmt",
                        arg: e.to_string(),
                    });
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
                    let _ = tx.send(UiEvent::ErrorKey {
                        key: "err.read_file_failed_fmt",
                        arg: e.to_string(),
                    });
                    return;
                }
            };
            let project = match deserialize_slave(&text) {
                Ok(p) => p,
                Err(e) => {
                    let _ = tx.send(UiEvent::ErrorKey {
                        key: "err.parse_failed_fmt",
                        arg: e.to_string(),
                    });
                    return;
                }
            };
            for c in project.connections {
                let id = format!("slave_{}", next_seq.fetch_add(1, Ordering::Relaxed));
                let label = c.label.clone();
                let log_collector = Arc::new(LogCollector::new());
                let transport = if c.tcp.tls.is_some() {
                    Transport::TcpTls {
                        host: c.tcp.host.clone(),
                        port: c.tcp.port,
                    }
                } else {
                    Transport::Tcp {
                        host: c.tcp.host.clone(),
                        port: c.tcp.port,
                    }
                };
                let mut connection =
                    SlaveConnection::new(transport).with_log_collector(log_collector.clone());
                if let Some(tls) = c.tcp.tls.as_ref() {
                    connection = connection.with_tls_config(SlaveTlsConfig {
                        enabled: true,
                        cert_file: tls.cert_file.clone(),
                        key_file: tls.key_file.clone(),
                        ca_file: tls.ca_file.clone(),
                        require_client_cert: tls.require_client_cert,
                        pkcs12_file: tls.pkcs12_file.clone(),
                        pkcs12_password: tls.pkcs12_password.clone(),
                    });
                }

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
                        let _ = tx.send(UiEvent::ErrorKey {
                            key: "err.load_device_failed_fmt",
                            arg: e.to_string(),
                        });
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
            let _ = tx.send(UiEvent::InfoKey {
                key: "err.loaded_fmt",
                arg: file.path().display().to_string(),
            });
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
                UiEvent::DeviceCountsUpdated {
                    conn_id,
                    slave_id,
                    counts,
                } => {
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
                UiEvent::ErrorKey { key, arg } => {
                    self.last_error = Some(tr1(self.lang, key, arg));
                }
                UiEvent::InfoKey { key, arg } => {
                    self.status_msg = Some(tr1(self.lang, key, arg));
                }
            }
        }
    }
}

enum TreeAction {
    ToggleConn(String),
    ToggleDevice {
        conn_id: String,
        slave_id: u8,
    },
    SelectConn(String),
    SelectDevice {
        conn_id: String,
        slave_id: u8,
    },
    SelectGroup {
        conn_id: String,
        slave_id: u8,
        reg_type: RegisterType,
    },
    StartConn(String),
    StopConn(String),
    RemoveConn(String),
    Create,
}

const REG_GROUPS: &[(RegisterType, &str)] = &[
    (RegisterType::Coil, "reg.fc01"),
    (RegisterType::DiscreteInput, "reg.fc02"),
    (RegisterType::InputRegister, "reg.fc04"),
    (RegisterType::HoldingRegister, "reg.fc03"),
];

fn reg_group_label(rt: RegisterType, lang: Lang) -> &'static str {
    let key = REG_GROUPS
        .iter()
        .find(|(r, _)| *r == rt)
        .map(|(_, k)| *k)
        .unwrap_or("?");
    tr(lang, key)
}

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
        matches!(
            self,
            Self::F32(_) | Self::U32(_) | Self::I32(_) | Self::F64(_)
        )
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

const ENDIANS: &[Endian] = &[
    Endian::Big,
    Endian::Little,
    Endian::MidBig,
    Endian::MidLittle,
];

fn selection_conn_id(s: &Selection) -> Option<&str> {
    match s {
        Selection::Connection(id)
        | Selection::Device { conn_id: id, .. }
        | Selection::RegisterGroup { conn_id: id, .. } => Some(id.as_str()),
        Selection::None => None,
    }
}

fn reg_type_label(rt: RegisterType, lang: Lang) -> &'static str {
    reg_group_label(rt, lang)
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
        let flavor = self.flavor;
        let acc_color = theme::accent(flavor);
        let acc_fg = theme::accent_fg(flavor);
        let acc_fill = egui::Color32::from_rgba_unmultiplied(0x1f, 0x6f, 0xeb, 0x26);
        let text_color = theme::text_body(flavor);
        let muted_color = theme::text_muted(flavor);

        // Paint a 2px left stripe + light-blue fill for an active row rect.
        let paint_active_row = |ui: &egui::Ui, rect: egui::Rect| {
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, acc_fill);
            let stripe_rect =
                egui::Rect::from_min_size(rect.left_top(), egui::vec2(2.0, rect.height()));
            painter.rect_filled(stripe_rect, 0.0, acc_color);
        };

        // 整行 8% alpha success 染色，仅用于运行中且未选中的 connection row。
        let paint_running_row = |ui: &egui::Ui, rect: egui::Rect| {
            let s = theme::success(flavor);
            ui.painter().rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), 0x14),
            );
        };

        for snap in &self.conn_snapshot {
            let conn_is_selected =
                matches!(&self.selection, Selection::Connection(c) if c == &snap.id);
            let (state_text, state_color) = match snap.state {
                ConnectionState::Running => {
                    (tr(self.lang, "conn.state.running"), theme::success(flavor))
                }
                ConnectionState::Stopped => (
                    tr(self.lang, "conn.state.stopped"),
                    theme::text_muted(flavor),
                ),
            };
            let is_running = matches!(snap.state, ConnectionState::Running);

            // Connection row
            ui.horizontal(|ui| {
                let arrow = if snap.expanded { "▼" } else { "▶" };
                if ui.small_button(arrow).clicked() {
                    action = Some(TreeAction::ToggleConn(snap.id.clone()));
                }
                let row_resp = ui.allocate_response(
                    egui::vec2(ui.available_width(), 22.0),
                    egui::Sense::click(),
                );

                // 优先级：selected > running 染色 > hover；三者互斥
                if conn_is_selected {
                    paint_active_row(ui, row_resp.rect);
                } else if is_running {
                    paint_running_row(ui, row_resp.rect);
                } else if row_resp.hovered() {
                    ui.painter()
                        .rect_filled(row_resp.rect, 0.0, theme::bg_hover(flavor));
                }

                // 状态圆点（左侧 8px 偏移、半径 3.5）
                let dot_center = row_resp.rect.left_center() + egui::vec2(8.0, 0.0);
                if is_running {
                    let phase = (ui.input(|i| i.time) * (2.0 * std::f64::consts::PI / 1.5)).sin()
                        * 0.5
                        + 0.5;
                    let alpha = (180.0 + 75.0 * phase) as u8;
                    let s = theme::success(flavor);
                    let c = egui::Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), alpha);
                    ui.painter().circle_filled(dot_center, 3.5, c);
                } else {
                    ui.painter().circle_stroke(
                        dot_center,
                        3.5,
                        egui::Stroke::new(1.0, theme::text_muted(flavor)),
                    );
                }

                // label + tag 分两次绘制
                let label_color = if conn_is_selected { acc_fg } else { text_color };
                let label_pos = row_resp.rect.left_center() + egui::vec2(20.0, 0.0);
                let label_galley = ui.painter().layout_no_wrap(
                    snap.label.clone(),
                    egui::FontId::proportional(12.5),
                    label_color,
                );
                let label_w = label_galley.size().x;
                let galley_top = label_pos - egui::vec2(0.0, label_galley.size().y / 2.0);
                ui.painter().galley(galley_top, label_galley, label_color);
                let tag_pos = label_pos + egui::vec2(label_w + 6.0, 0.0);
                ui.painter().text(
                    tag_pos,
                    egui::Align2::LEFT_CENTER,
                    state_text,
                    egui::FontId::proportional(11.0),
                    state_color,
                );

                if row_resp.clicked() {
                    action = Some(TreeAction::SelectConn(snap.id.clone()));
                }
            });

            // Per-connection: 单个状态相关按钮（启动/停止），删除挪到 footer
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                let (icon, label_text, color, act): (&str, &str, egui::Color32, TreeAction) =
                    match snap.state {
                        ConnectionState::Stopped => (
                            "▶",
                            tr(self.lang, "conn.start"),
                            theme::success(flavor),
                            TreeAction::StartConn(snap.id.clone()),
                        ),
                        ConnectionState::Running => (
                            "■",
                            tr(self.lang, "conn.stop"),
                            theme::warn(flavor),
                            TreeAction::StopConn(snap.id.clone()),
                        ),
                    };
                ui.label(egui::RichText::new(icon).color(color).size(12.0));
                if uikit::secondary_button_sm(ui, flavor, label_text).clicked() {
                    action = Some(act);
                }
            });

            if snap.expanded {
                for dev in &snap.devices {
                    let dev_is_selected = matches!(&self.selection,
                        Selection::Device { conn_id, slave_id }
                            if conn_id == &snap.id && *slave_id == dev.slave_id);
                    let dev_label =
                        tr2(self.lang, "slave.with_id_name_fmt", dev.slave_id, &dev.name);

                    // Device row
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        let arrow = if dev.expanded { "▼" } else { "▶" };
                        if ui.small_button(arrow).clicked() {
                            action = Some(TreeAction::ToggleDevice {
                                conn_id: snap.id.clone(),
                                slave_id: dev.slave_id,
                            });
                        }
                        let row_resp = ui.allocate_response(
                            egui::vec2(ui.available_width(), 22.0),
                            egui::Sense::click(),
                        );
                        if dev_is_selected {
                            paint_active_row(ui, row_resp.rect);
                        } else if row_resp.hovered() {
                            ui.painter()
                                .rect_filled(row_resp.rect, 0.0, theme::bg_hover(flavor));
                        }
                        let label_color = if dev_is_selected { acc_fg } else { text_color };
                        ui.painter().text(
                            row_resp.rect.left_center() + egui::vec2(4.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            &dev_label,
                            egui::FontId::proportional(12.5),
                            label_color,
                        );
                        if row_resp.clicked() {
                            action = Some(TreeAction::SelectDevice {
                                conn_id: snap.id.clone(),
                                slave_id: dev.slave_id,
                            });
                        }
                    });

                    if dev.expanded {
                        for (reg_type, key) in REG_GROUPS {
                            let grp_is_selected = matches!(&self.selection,
                                Selection::RegisterGroup { conn_id, slave_id, reg_type: rt }
                                    if conn_id == &snap.id && *slave_id == dev.slave_id && rt == reg_type);
                            let count = dev.counts.count_for(*reg_type);
                            let grp_label = format!("{} ({})", tr(self.lang, *key), count);

                            // Register group row
                            ui.horizontal(|ui| {
                                ui.add_space(32.0);
                                let row_resp = ui.allocate_response(
                                    egui::vec2(ui.available_width(), 22.0),
                                    egui::Sense::click(),
                                );
                                if grp_is_selected {
                                    paint_active_row(ui, row_resp.rect);
                                } else if row_resp.hovered() {
                                    ui.painter().rect_filled(
                                        row_resp.rect,
                                        0.0,
                                        theme::bg_hover(flavor),
                                    );
                                }
                                let label_color =
                                    if grp_is_selected { acc_fg } else { muted_color };
                                ui.painter().text(
                                    row_resp.rect.left_center() + egui::vec2(4.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    &grp_label,
                                    egui::FontId::proportional(12.5),
                                    label_color,
                                );
                                if row_resp.clicked() {
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
            TreeAction::SelectGroup {
                conn_id,
                slave_id,
                reg_type,
            } => {
                self.selection = Selection::RegisterGroup {
                    conn_id,
                    slave_id,
                    reg_type,
                };
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
        if self.add_device_modal.is_none() {
            return;
        }

        enum Act {
            Submit,
            Close,
        }
        let mut act: Option<Act> = None;
        let mut is_open = true;

        let lang = self.lang;
        egui::Window::new(tr(lang, "dlg.add_slave.title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut is_open)
            .show(ctx, |ui| {
                let Some(st) = self.add_device_modal.as_mut() else {
                    return;
                };
                egui::Grid::new("add_device_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(tr(lang, "dlg.add_slave.connection"));
                        ui.label(&st.conn_id);
                        ui.end_row();

                        ui.label(tr(lang, "dlg.add_slave.slave_id"));
                        let mut sid = st.slave_id as u32;
                        ui.add(egui::DragValue::new(&mut sid).range(1..=247));
                        st.slave_id = sid as u8;
                        ui.end_row();

                        ui.label(tr(lang, "dlg.add_slave.name"));
                        ui.text_edit_singleline(&mut st.name);
                        ui.end_row();

                        ui.label(tr(lang, "dlg.add_slave.init_mode"));
                        egui::ComboBox::from_id_salt("add_device_init")
                            .selected_text(tr(
                                lang,
                                match st.init_mode {
                                    DeviceInitMode::Empty => "dlg.add_slave.init.empty",
                                    DeviceInitMode::Default => "dlg.add_slave.init.default",
                                    DeviceInitMode::Random => "dlg.add_slave.init.random",
                                },
                            ))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut st.init_mode,
                                    DeviceInitMode::Empty,
                                    tr(lang, "dlg.add_slave.init.empty"),
                                );
                                ui.selectable_value(
                                    &mut st.init_mode,
                                    DeviceInitMode::Default,
                                    tr(lang, "dlg.add_slave.init.default"),
                                );
                                ui.selectable_value(
                                    &mut st.init_mode,
                                    DeviceInitMode::Random,
                                    tr(lang, "dlg.add_slave.init.random"),
                                );
                            });
                        ui.end_row();

                        if !matches!(st.init_mode, DeviceInitMode::Empty) {
                            ui.label(tr(lang, "dlg.add_slave.max_addr"));
                            ui.add(egui::DragValue::new(&mut st.max_address).range(0..=65535));
                            ui.end_row();
                        }
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!st.busy, egui::Button::new(tr(lang, "dlg.ok")))
                        .clicked()
                    {
                        act = Some(Act::Submit);
                    }
                    if ui.button(tr(lang, "dlg.cancel")).clicked() {
                        act = Some(Act::Close);
                    }
                    if st.busy {
                        ui.spinner();
                    }
                });
            });

        if !is_open {
            act = Some(Act::Close);
        }
        match act {
            Some(Act::Submit) => self.submit_add_device(ctx.clone()),
            Some(Act::Close) => {
                self.add_device_modal = None;
            }
            None => {}
        }
    }

    fn render_batch_modal(&mut self, ctx: &egui::Context) {
        if self.batch_modal.is_none() {
            return;
        }

        enum ModalAction {
            Submit,
            Close,
        }
        let mut action: Option<ModalAction> = None;
        let mut is_open = true;

        let lang = self.lang;
        egui::Window::new(tr(lang, "dlg.batch.title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut is_open)
            .show(ctx, |ui| {
                let Some(state) = self.batch_modal.as_mut() else {
                    return;
                };
                egui::Grid::new("batch_add_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(tr(lang, "dlg.batch.conn_slave"));
                        ui.label(tr2(lang, "conn.label_fmt", &state.conn_id, state.slave_id));
                        ui.end_row();

                        ui.label(tr(lang, "dlg.batch.start"));
                        ui.add(egui::DragValue::new(&mut state.start_addr).range(0..=65535));
                        ui.end_row();

                        ui.label(tr(lang, "dlg.batch.end"));
                        ui.add(egui::DragValue::new(&mut state.end_addr).range(0..=65535));
                        ui.end_row();

                        ui.label(tr(self.lang, "dlg.batch.reg_type"));
                        egui::ComboBox::from_id_salt("batch_reg_type")
                            .selected_text(reg_type_label(state.reg_type, self.lang))
                            .show_ui(ui, |ui| {
                                for (rt, key) in REG_GROUPS {
                                    ui.selectable_value(
                                        &mut state.reg_type,
                                        *rt,
                                        tr(self.lang, *key),
                                    );
                                }
                            });
                        ui.end_row();

                        ui.label(tr(lang, "dlg.batch.data_type"));
                        egui::ComboBox::from_id_salt("batch_data_type")
                            .selected_text(data_type_label(state.data_type))
                            .show_ui(ui, |ui| {
                                for dt in DATA_TYPES {
                                    ui.selectable_value(
                                        &mut state.data_type,
                                        *dt,
                                        data_type_label(*dt),
                                    );
                                }
                            });
                        ui.end_row();

                        ui.label(tr(lang, "dlg.batch.endian"));
                        egui::ComboBox::from_id_salt("batch_endian")
                            .selected_text(endian_label(state.endian))
                            .show_ui(ui, |ui| {
                                for e in ENDIANS {
                                    ui.selectable_value(&mut state.endian, *e, endian_label(*e));
                                }
                            });
                        ui.end_row();

                        ui.label(tr(lang, "dlg.batch.name_prefix"));
                        ui.text_edit_singleline(&mut state.name_prefix);
                        ui.end_row();
                    });

                let stride = state.data_type.register_count().max(1) as u32;
                let raw_count = if state.end_addr >= state.start_addr {
                    (state.end_addr - state.start_addr) / stride + 1
                } else {
                    0
                };
                ui.separator();
                ui.horizontal(|ui| {
                    if raw_count == 0 {
                        ui.colored_label(egui::Color32::RED, tr(lang, "dlg.batch.invalid_range"));
                    } else if raw_count > 50_000 {
                        ui.colored_label(
                            egui::Color32::RED,
                            tr1(lang, "err.range_too_large_fmt", raw_count),
                        );
                    } else {
                        ui.label(tr1(lang, "dlg.batch.will_add_fmt", raw_count));
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !state.busy && raw_count > 0 && raw_count <= 50_000,
                            egui::Button::new(tr(lang, "dlg.batch.confirm")),
                        )
                        .clicked()
                    {
                        action = Some(ModalAction::Submit);
                    }
                    if ui.button(tr(lang, "dlg.cancel")).clicked() {
                        action = Some(ModalAction::Close);
                    }
                    if state.busy {
                        ui.spinner();
                    }
                });
            });

        if !is_open {
            action = Some(ModalAction::Close);
        }
        match action {
            Some(ModalAction::Submit) => {
                self.submit_batch_add(ctx.clone());
                self.batch_modal = None;
            }
            Some(ModalAction::Close) => {
                self.batch_modal = None;
            }
            None => {}
        }
    }

    fn render_main(&mut self, ui: &mut egui::Ui) {
        // Snapshot what's selected; later we might mutate self.batch_modal.
        let selection = self.selection.clone();
        match &selection {
            Selection::None => {
                let feed = self.hero_pulse.feed(&self.connections);
                show_welcome_hero(
                    ui,
                    self.flavor,
                    icons::CPU,
                    "ModbusSlave",
                    tr(self.lang, "hero.empty_main"),
                    feed,
                );
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
                                format!(
                                    "{}  {}",
                                    icons::CIRCLE,
                                    tr(self.lang, "conn.state.running")
                                ),
                                theme::success(self.flavor),
                            ),
                            ConnectionState::Stopped => (
                                format!(
                                    "{}  {}",
                                    icons::PAUSE,
                                    tr(self.lang, "conn.state.stopped")
                                ),
                                theme::subtext(self.flavor),
                            ),
                        };
                        uikit::status_pill(ui, txt, color);
                    });
                    uikit::caption(
                        ui,
                        self.flavor,
                        tr1(self.lang, "device.count_fmt", dev_count),
                    );
                    ui.add_space(8.0);
                    if uikit::primary_button(
                        ui,
                        self.flavor,
                        format!(
                            "{}  {}",
                            icons::PLUS_CIRCLE,
                            tr(self.lang, "regtable.new_slave")
                        ),
                    )
                    .clicked()
                    {
                        self.open_add_device_modal(id.clone());
                    }
                } else {
                    ui.label(tr(self.lang, "hero.conn_missing"));
                }
            }
            Selection::Device { conn_id, slave_id } => {
                let conn = self.conn_snapshot.iter().find(|s| &s.id == conn_id);
                match conn.and_then(|c| c.devices.iter().find(|d| d.slave_id == *slave_id)) {
                    Some(d) => {
                        ui.heading(tr2(
                            self.lang,
                            "slave.with_id_name_fmt",
                            d.slave_id,
                            &d.name,
                        ));
                        egui::Grid::new("dev_summary")
                            .num_columns(2)
                            .spacing([16.0, 4.0])
                            .show(ui, |ui| {
                                ui.label(tr(self.lang, "reg.fc01"));
                                ui.label(d.counts.coils.to_string());
                                ui.end_row();
                                ui.label(tr(self.lang, "reg.fc02"));
                                ui.label(d.counts.discrete_inputs.to_string());
                                ui.end_row();
                                ui.label(tr(self.lang, "reg.fc04"));
                                ui.label(d.counts.input_registers.to_string());
                                ui.end_row();
                                ui.label(tr(self.lang, "reg.fc03"));
                                ui.label(d.counts.holding_registers.to_string());
                                ui.end_row();
                            });
                        ui.add_space(8.0);
                        let flavor = self.flavor;
                        let lang = self.lang;
                        ui.horizontal(|ui| {
                            if uikit::secondary_button_sm(
                                ui,
                                flavor,
                                format!(
                                    "{}  {}",
                                    icons::PLUS_CIRCLE,
                                    tr(lang, "regtable.batch_add")
                                ),
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
                                format!("{}  {}", icons::TRASH, tr(lang, "regtable.delete_slave")),
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
                        uikit::section_heading(ui, "", tr(self.lang, "jitter.title"));

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
                            ui.checkbox(&mut new_jitter.enabled, tr(self.lang, "jitter.enabled"));
                        });
                        let mut interval = new_jitter.interval_ms as i32;
                        let mut rate = new_jitter.mutation_rate as i32;
                        let mut delta = new_jitter.delta_percent as i32;
                        egui::Grid::new("jitter_grid")
                            .num_columns(2)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                ui.label(tr(self.lang, "jitter.period"));
                                ui.add(egui::Slider::new(&mut interval, 100..=5000).suffix(" ms"));
                                ui.end_row();
                                ui.label(tr(self.lang, "jitter.change_rate"));
                                ui.add(egui::Slider::new(&mut rate, 0..=100).suffix(" %"));
                                ui.end_row();
                                ui.label(tr(self.lang, "jitter.drift"));
                                ui.add(egui::Slider::new(&mut delta, 0..=100).suffix(" %"));
                                ui.end_row();
                            });
                        new_jitter.interval_ms = interval as u64;
                        new_jitter.mutation_rate = rate as u8;
                        new_jitter.delta_percent = delta as u8;
                        ui.horizontal(|ui| {
                            ui.label(tr(self.lang, "jitter.scope"));
                            ui.checkbox(
                                &mut new_jitter.affect_coils,
                                tr(self.lang, "jitter.coils"),
                            );
                            ui.checkbox(
                                &mut new_jitter.affect_discrete,
                                tr(self.lang, "jitter.discrete"),
                            );
                            ui.checkbox(
                                &mut new_jitter.affect_holding,
                                tr(self.lang, "jitter.holding"),
                            );
                            ui.checkbox(
                                &mut new_jitter.affect_input,
                                tr(self.lang, "jitter.input"),
                            );
                        });

                        if new_jitter != cur_jitter {
                            self.set_device_jitter(conn_id.clone(), *slave_id, new_jitter);
                        }

                        ui.separator();
                        uikit::section_heading(ui, icons::GEAR, tr(self.lang, "ds.panel"));

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
                            ui.label(tr(self.lang, "ds.none"));
                        } else {
                            egui::Grid::new("ds_list")
                                .num_columns(6)
                                .spacing([10.0, 4.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.strong(tr(self.lang, "ds.col.type"));
                                    ui.strong(tr(self.lang, "ds.col.address"));
                                    ui.strong(tr(self.lang, "ds.col.source"));
                                    ui.strong(tr(self.lang, "ds.col.interval_ms"));
                                    ui.strong(tr(self.lang, "ds.col.enabled"));
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
                                        if ui.small_button(tr(self.lang, "ds.delete")).clicked() {
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
                                    .prefix(tr(self.lang, "ds.address_prefix")),
                            );
                            egui::ComboBox::from_id_salt("ds_kind")
                                .selected_text(self.ds_add_kind.label(self.lang))
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
                                        ui.selectable_value(
                                            &mut self.ds_add_kind,
                                            k,
                                            k.label(self.lang),
                                        );
                                    }
                                });
                            ui.add(
                                egui::DragValue::new(&mut self.ds_add_interval_ms)
                                    .range(50..=60_000)
                                    .suffix(" ms"),
                            );
                            if ui.button(tr(self.lang, "ds.add_to_fc03")).clicked() {
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
                        ui.label(tr(self.lang, "hero.device_missing"));
                    }
                }
            }
            Selection::RegisterGroup {
                conn_id,
                slave_id,
                reg_type,
            } => {
                let lang = self.lang;
                let group_label = reg_group_label(*reg_type, lang);
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
                    egui::Margin::symmetric(14.0 as i8, 10.0 as i8),
                    |ui| {
                        ui.horizontal(|ui| {
                            uikit::panel_header(
                                ui,
                                flavor,
                                &format!("{}  {}", reg_icon, group_label),
                                Some(&tr2(lang, "conn.label_fmt", conn_id, slave_id)),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if uikit::secondary_button_sm(
                                        ui,
                                        flavor,
                                        format!(
                                            "{}  {}",
                                            icons::PLUS_CIRCLE,
                                            tr(lang, "regtable.batch_add")
                                        ),
                                    )
                                    .clicked()
                                    {
                                        open_batch = true;
                                    }
                                    ui.add_space(8.0);
                                    let toggle_label = if self.value_parse_open {
                                        tr(lang, "regtable.collapse_value_panel")
                                    } else {
                                        tr(lang, "regtable.expand_value_panel")
                                    };
                                    if uikit::link_action(ui, flavor, toggle_label, false).clicked()
                                    {
                                        self.value_parse_open = !self.value_parse_open;
                                    }
                                    ui.add_space(8.0);
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut search_text)
                                            .hint_text(tr(lang, "regtable.search_hint"))
                                            .desired_width(220.0),
                                    );
                                    if want_focus {
                                        resp.request_focus();
                                        // Also select-all so the next keystroke overwrites.
                                        if let Some(mut state) =
                                            egui::TextEdit::load_state(ui.ctx(), resp.id)
                                        {
                                            let cc = egui::text::CCursor::new(
                                                search_text.chars().count(),
                                            );
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
                self.search_buf
                    .insert(search_key.clone(), search_text.clone());
                self.want_focus_search = want_focus;
                let search_intent = parse_search_intent(&search_text);
                if open_batch {
                    self.open_batch_modal(conn_id.clone(), *slave_id, *reg_type);
                }

                let Some(view) = &self.reg_view else {
                    ui.label(tr(lang, "hero.loading"));
                    return;
                };
                if view.conn_id != *conn_id
                    || view.slave_id != *slave_id
                    || view.reg_type != *reg_type
                {
                    ui.label(tr(lang, "hero.refreshing"));
                    return;
                }

                let is_bool = matches!(reg_type, RegisterType::Coil | RegisterType::DiscreteInput);

                // 只有 16-bit 寄存器区（FC03 / FC04）支持多字节序显示；线圈区强制 U16。
                let mode = if is_bool {
                    ValueDisplayMode::U16
                } else {
                    self.reg_display_mode
                };

                // ── fmt-pill 工具栏 ──────────────────────────────────────────
                ui.horizontal(|ui| {
                    if !is_bool {
                        theme::text::tiny_caps(ui, flavor, tr(lang, "regtable.format"));
                        ui.add_space(4.0);
                        egui::Frame::new()
                            .fill(theme::bg_of(flavor, theme::Layer::L2))
                            .stroke(egui::Stroke::new(1.0, theme::border_strong(flavor)))
                            .corner_radius(12.0)
                            .inner_margin(egui::Margin::symmetric(8, 2))
                            .show(ui, |ui| {
                                egui::ComboBox::from_id_salt("reg_display_mode")
                                    .selected_text(
                                        egui::RichText::new(mode.label())
                                            .color(theme::accent_fg(flavor))
                                            .monospace()
                                            .size(11.5),
                                    )
                                    .show_ui(ui, |ui| {
                                        for m in DISPLAY_MODES {
                                            ui.selectable_value(
                                                &mut self.reg_display_mode,
                                                *m,
                                                m.label(),
                                            );
                                        }
                                    });
                            });
                        ui.add_space(12.0);
                    }
                    theme::text::crumb(
                        ui,
                        flavor,
                        &format!(
                            "{}{}",
                            tr1(lang, "regtable.total_fmt", view.row_count),
                            if self.selected_addrs.is_empty() {
                                String::new()
                            } else {
                                tr1(lang, "regtable.selected_fmt", self.selected_addrs.len())
                            }
                        ),
                    );
                    if !is_bool {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if uikit::link_action(
                                ui,
                                flavor,
                                tr(lang, "regtable.clear_selected"),
                                false,
                            )
                            .clicked()
                            {
                                self.selected_addrs.clear();
                                self.click_anchor = None;
                            }
                        });
                    }
                });
                if !is_bool && mode.is_multi_word() {
                    theme::text::crumb(ui, flavor, tr(lang, "regtable.multi_readonly"));
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

                // Left = table, right = ValuePanel (按 self.value_parse_open 切换)。
                // StripBuilder is the right primitive here — a plain
                // ui.horizontal + allocate_ui collapses to 0 height inside a
                // CentralPanel and draws the debug red warning box.
                use egui_extras::{Size, StripBuilder};
                let value_open = self.value_parse_open;
                let (table_size, gap_size, panel_size) = if value_open {
                    (
                        Size::relative(0.62).at_least(360.0),
                        Size::exact(8.0),
                        Size::remainder().at_least(260.0),
                    )
                } else {
                    // Hidden: 表格独占全宽；右 cell 仍占 0 宽，保持三段链结构。
                    (
                        Size::remainder().at_least(360.0),
                        Size::exact(0.0),
                        Size::exact(0.0),
                    )
                };
                StripBuilder::new(ui)
                    .size(table_size)
                    .size(gap_size)
                    .size(panel_size)
                    .horizontal(|mut strip| {
                        strip.cell(|ui| {
                            uikit::region(
                                ui,
                                flavor,
                                theme::Layer::L2,
                                egui::Margin::symmetric(8.0 as i8, 6.0 as i8),
                                |ui| {
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
                                            .striped(false)
                                            .resizable(true)
                                            .max_scroll_height(avail_h)
                                            .cell_layout(egui::Layout::left_to_right(
                                                egui::Align::Center,
                                            ))
                                            .column(Column::exact(110.0))
                                            .column(Column::exact(220.0))
                                            .column(Column::exact(200.0))
                                            .column(Column::remainder())
                                            .header(26.0, |mut h| {
                                                h.col(|ui| {
                                                    theme::text::tiny_caps(
                                                        ui,
                                                        flavor,
                                                        tr(lang, "regtable.address"),
                                                    )
                                                });
                                                h.col(|ui| {
                                                    theme::text::tiny_caps(ui, flavor, mode.label())
                                                });
                                                h.col(|ui| {
                                                    theme::text::tiny_caps(ui, flavor, "Raw HEX")
                                                });
                                                h.col(|ui| {
                                                    // 表头下方 2px 蓝线
                                                    let r = ui.max_rect();
                                                    let y = r.max.y - 1.0;
                                                    ui.painter().line_segment(
                                                        [
                                                            egui::pos2(-8000.0, y),
                                                            egui::pos2(8000.0, y),
                                                        ],
                                                        egui::Stroke::new(
                                                            2.0,
                                                            theme::accent(flavor),
                                                        ),
                                                    );
                                                });
                                            })
                                            .body(|body| {
                                                body.rows(row_h, group_rows, |mut row| {
                                                    let base = row.index() as u16 * stride as u16;
                                                    // Gather stride u16 values.
                                                    let mut ws: Vec<u16> =
                                                        Vec::with_capacity(stride);
                                                    let mut all_present = true;
                                                    for i in 0..stride as u16 {
                                                        match view.u16_map.as_ref().and_then(|m| {
                                                            m.get(&(base + i)).copied()
                                                        }) {
                                                            Some(v) => ws.push(v),
                                                            None => {
                                                                all_present = false;
                                                                break;
                                                            }
                                                        }
                                                    }
                                                    row.col(|ui| {
                                                        let sel = (0..stride as u16).any(|i| {
                                                            selected_addrs.contains(&(base + i))
                                                        });
                                                        let label = if stride == 4 {
                                                            format!("{}..{}", base, base + 3)
                                                        } else {
                                                            format!("{}..{}", base, base + 1)
                                                        };
                                                        let resp =
                                                            ui.add(egui::Button::selectable(
                                                                sel,
                                                                egui::RichText::new(label)
                                                                    .monospace()
                                                                    .color(theme::text_muted(
                                                                        flavor,
                                                                    )),
                                                            ));
                                                        if resp.clicked() {
                                                            row_clicks.push((
                                                                base,
                                                                resp.ctx.input(|i| i.modifiers),
                                                            ));
                                                        }
                                                    });
                                                    row.col(|ui| {
                                                        if !all_present {
                                                            ui.monospace("—");
                                                            return;
                                                        }
                                                        let text = match mode {
                                                            ValueDisplayMode::F32(_) => {
                                                                let d = decode_value(
                                                                    &ws,
                                                                    DataType::Float32,
                                                                    endian,
                                                                )
                                                                .unwrap_or(f64::NAN);
                                                                format!("{:.6}", d as f32)
                                                            }
                                                            ValueDisplayMode::U32(_) => {
                                                                let d = decode_value(
                                                                    &ws,
                                                                    DataType::UInt32,
                                                                    endian,
                                                                )
                                                                .unwrap_or(f64::NAN);
                                                                format!("{}", d as u32)
                                                            }
                                                            ValueDisplayMode::I32(_) => {
                                                                let d = decode_value(
                                                                    &ws,
                                                                    DataType::Int32,
                                                                    endian,
                                                                )
                                                                .unwrap_or(f64::NAN);
                                                                format!("{}", d as i32)
                                                            }
                                                            ValueDisplayMode::F64(_) => {
                                                                let v = value_panel::decode_f64(
                                                                    &ws, f64_order,
                                                                );
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
                                                        ui.add(egui::Label::new(
                                                            egui::RichText::new(joined)
                                                                .monospace()
                                                                .color(theme::warn(flavor)),
                                                        ));
                                                    });
                                                    row.col(|_| {});
                                                });
                                            });
                                    } else {
                                        // Apply search intent: Jump sets a one-shot scroll_to + highlight;
                                        // Filter builds a reduced addr list that drives body.rows.
                                        let filtered_addrs: Option<Vec<u16>> = match &search_intent
                                        {
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
                                                let new_key =
                                                    (conn_id.clone(), *slave_id, *reg_type, addr);
                                                let same = self
                                                    .highlight
                                                    .as_ref()
                                                    .map(|h| {
                                                        (h.0.clone(), h.1, h.2, h.3) == new_key
                                                    })
                                                    .unwrap_or(false);
                                                if !same {
                                                    self.highlight = Some((
                                                        new_key.0,
                                                        new_key.1,
                                                        new_key.2,
                                                        addr,
                                                        Instant::now(),
                                                    ));
                                                    scroll_to_row = Some(addr as usize);
                                                }
                                            }
                                        }

                                        if let Some(list) = &filtered_addrs {
                                            if list.is_empty() {
                                                ui.add_space(8.0);
                                                uikit::caption(
                                                    ui,
                                                    flavor,
                                                    tr(lang, "regtable.no_match"),
                                                );
                                                return;
                                            }
                                        }

                                        let body_row_count = filtered_addrs
                                            .as_ref()
                                            .map(|v| v.len())
                                            .unwrap_or(view.row_count);
                                        let avail_h = ui.available_height();
                                        // Column layout differs for bool: (地址 / 值 / 名称 / 注释)
                                        // vs u16: (地址 / 值 / Hex / Binary / 空).
                                        // Column::exact() hard-locks range(w..=w), defeating resizable(true).
                                        // Use initial() + at_least() + clip(true) so users can drag column
                                        // dividers; at_least prevents dragging to 0 (would hide column).
                                        let mut tb = TableBuilder::new(ui)
                                            .striped(false)
                                            .resizable(true)
                                            .max_scroll_height(avail_h)
                                            .cell_layout(egui::Layout::left_to_right(
                                                egui::Align::Center,
                                            ))
                                            .column(
                                                Column::initial(80.0).at_least(60.0).clip(true),
                                            ); // 地址
                                        if is_bool {
                                            tb = tb
                                                .column(
                                                    Column::initial(72.0).at_least(56.0).clip(true),
                                                ) // 值 (48×24 toggle + 余量)
                                                .column(
                                                    Column::initial(200.0)
                                                        .at_least(80.0)
                                                        .clip(true),
                                                ) // 名称
                                                .column(
                                                    Column::remainder().at_least(80.0).clip(true),
                                                ); // 注释
                                        } else {
                                            tb = tb
                                                .column(
                                                    Column::initial(110.0)
                                                        .at_least(72.0)
                                                        .clip(true),
                                                ) // 值
                                                .column(
                                                    Column::initial(100.0)
                                                        .at_least(72.0)
                                                        .clip(true),
                                                ) // Hex
                                                .column(
                                                    Column::initial(140.0)
                                                        .at_least(96.0)
                                                        .clip(true),
                                                ) // Binary
                                                .column(
                                                    Column::remainder().at_least(80.0).clip(true),
                                                ); // 尾
                                        }
                                        if let Some(idx) = scroll_to_row {
                                            tb = tb.scroll_to_row(idx, Some(egui::Align::Center));
                                        }
                                        let highlight_addr: Option<u16> =
                                            self.highlight.as_ref().and_then(|h| {
                                                if &h.0 == conn_id
                                                    && h.1 == *slave_id
                                                    && h.2 == *reg_type
                                                {
                                                    Some(h.3)
                                                } else {
                                                    None
                                                }
                                            });
                                        let defs = view.defs.clone();
                                        tb.header(26.0, |mut header| {
                                            header.col(|ui| {
                                                theme::text::tiny_caps(
                                                    ui,
                                                    flavor,
                                                    tr(lang, "regtable.address"),
                                                )
                                            });
                                            if is_bool {
                                                header.col(|ui| {
                                                    theme::text::tiny_caps(
                                                        ui,
                                                        flavor,
                                                        tr(lang, "regtable.value"),
                                                    )
                                                });
                                                header.col(|ui| {
                                                    theme::text::tiny_caps(
                                                        ui,
                                                        flavor,
                                                        tr(lang, "regtable.name"),
                                                    )
                                                });
                                                header.col(|ui| {
                                                    theme::text::tiny_caps(
                                                        ui,
                                                        flavor,
                                                        tr(lang, "regtable.note"),
                                                    );
                                                    // 表头下方 2px 蓝线（跨整行，画在最后一列底边）
                                                    let r = ui.max_rect();
                                                    let y = r.max.y - 1.0;
                                                    let x0 = -8000.0_f32; // 超出左界以覆盖全行
                                                    let x1 = 8000.0_f32;
                                                    ui.painter().line_segment(
                                                        [egui::pos2(x0, y), egui::pos2(x1, y)],
                                                        egui::Stroke::new(
                                                            2.0,
                                                            theme::accent(flavor),
                                                        ),
                                                    );
                                                });
                                            } else {
                                                header.col(|ui| {
                                                    theme::text::tiny_caps(ui, flavor, mode.label())
                                                });
                                                header.col(|ui| {
                                                    theme::text::tiny_caps(ui, flavor, "HEX")
                                                });
                                                header.col(|ui| {
                                                    theme::text::tiny_caps(
                                                        ui,
                                                        flavor,
                                                        tr(lang, "regtable.binary"),
                                                    )
                                                });
                                                header.col(|ui| {
                                                    // 表头下方 2px 蓝线（画在尾列底边，覆盖全行）
                                                    let r = ui.max_rect();
                                                    let y = r.max.y - 1.0;
                                                    let x0 = -8000.0_f32;
                                                    let x1 = 8000.0_f32;
                                                    ui.painter().line_segment(
                                                        [egui::pos2(x0, y), egui::pos2(x1, y)],
                                                        egui::Stroke::new(
                                                            2.0,
                                                            theme::accent(flavor),
                                                        ),
                                                    );
                                                });
                                            }
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
                                                    let resp = ui.add(egui::Button::selectable(
                                                        sel,
                                                        egui::RichText::new(format!("{}", addr))
                                                            .monospace()
                                                            .color(theme::text_muted(flavor)),
                                                    ));
                                                    if resp.clicked() {
                                                        row_clicks.push((
                                                            addr,
                                                            resp.ctx.input(|i| i.modifiers),
                                                        ));
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
                                                    let current = pending
                                                        .get(&key)
                                                        .map(|v| *v != 0)
                                                        .unwrap_or(cache_bool);
                                                    row.col(|ui| {
                                                        let mut tmp = current;
                                                        let resp = uikit::toggle_switch(
                                                            ui, flavor, &mut tmp,
                                                        );
                                                        if resp.clicked() && tmp != current {
                                                            writes.push((
                                                                addr,
                                                                if tmp { 1 } else { 0 },
                                                            ));
                                                            pending.remove(&key);
                                                        }
                                                    });
                                                    let name = defs
                                                        .get(&addr)
                                                        .map(|(n, _)| n.clone())
                                                        .unwrap_or_default();
                                                    let comment = defs
                                                        .get(&addr)
                                                        .map(|(_, c)| c.clone())
                                                        .unwrap_or_default();
                                                    row.col(|ui| {
                                                        if !name.is_empty() {
                                                            ui.monospace(name);
                                                        }
                                                    });
                                                    row.col(|ui| {
                                                        if !comment.is_empty() {
                                                            ui.monospace(comment);
                                                        }
                                                    });
                                                } else {
                                                    row.col(|ui| {
                                                        let (min_i, max_i) = match mode {
                                                            ValueDisplayMode::I16 => {
                                                                (i16::MIN as i32, i16::MAX as i32)
                                                            }
                                                            _ => (0, u16::MAX as i32),
                                                        };
                                                        let cache_as_display = match mode {
                                                            ValueDisplayMode::I16 => {
                                                                cache_u16 as i16 as i32
                                                            }
                                                            _ => cache_u16 as i32,
                                                        };
                                                        let mut tmp: i32 = pending
                                                            .get(&key)
                                                            .copied()
                                                            .unwrap_or(cache_as_display);
                                                        let resp = ui.add(
                                                            egui::DragValue::new(&mut tmp)
                                                                .range(min_i..=max_i),
                                                        );
                                                        let active = resp.has_focus()
                                                            || resp.dragged()
                                                            || resp.drag_started()
                                                            || resp.gained_focus();
                                                        if active {
                                                            pending.insert(key, tmp);
                                                        } else if let Some(prev) =
                                                            pending.remove(&key)
                                                        {
                                                            let v = match mode {
                                                                ValueDisplayMode::I16 => {
                                                                    prev.clamp(
                                                                        i16::MIN as i32,
                                                                        i16::MAX as i32,
                                                                    )
                                                                        as i16
                                                                        as u16
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
                                                            ValueDisplayMode::I16 => v.clamp(
                                                                i16::MIN as i32,
                                                                i16::MAX as i32,
                                                            )
                                                                as i16
                                                                as u16,
                                                            _ => v.clamp(0, 65535) as u16,
                                                        })
                                                        .unwrap_or(cache_u16);
                                                    row.col(|ui| {
                                                        ui.add(egui::Label::new(
                                                            egui::RichText::new(format_u16(
                                                                display_u16,
                                                                U16Format::Hex,
                                                            ))
                                                            .monospace()
                                                            .color(theme::warn(flavor)),
                                                        ));
                                                    });
                                                    row.col(|ui| {
                                                        ui.add(egui::Label::new(
                                                            egui::RichText::new(format_u16(
                                                                display_u16,
                                                                U16Format::Binary,
                                                            ))
                                                            .monospace()
                                                            .size(11.0)
                                                            .color(theme::text_muted(flavor)),
                                                        ));
                                                    });
                                                    row.col(|_| {});
                                                }
                                            });
                                        });
                                    }
                                },
                            ); // end left region
                        }); // end StripBuilder left cell
                        strip.cell(|_ui| {});
                        strip.cell(|ui| {
                            uikit::region(
                                ui,
                                flavor,
                                theme::Layer::L1,
                                egui::Margin::symmetric(12.0 as i8, 10.0 as i8),
                                |ui| {
                                    let mut selected_vals: Vec<u16> = Vec::new();
                                    let mut base: Option<u16> = None;
                                    // Only take up to 4 selected, in address order, and
                                    // require them to be contiguous for multi-word analysis.
                                    let ordered: Vec<u16> =
                                        selected_addrs.iter().copied().take(4).collect();
                                    for (i, a) in ordered.iter().enumerate() {
                                        if i == 0 {
                                            base = Some(*a);
                                        } else if *a != ordered[i - 1] + 1 {
                                            // Non-contiguous: stop collecting so ValuePanel
                                            // only shows formats it can compute safely.
                                            break;
                                        }
                                        if let Some(v) =
                                            view.u16_map.as_ref().and_then(|m| m.get(a).copied())
                                        {
                                            selected_vals.push(v);
                                        } else if let Some(b) =
                                            view.bool_map.as_ref().and_then(|m| m.get(a).copied())
                                        {
                                            selected_vals.push(if b { 1 } else { 0 });
                                        }
                                    }
                                    if let Some(vp_writes) =
                                        value_panel::render(ui, flavor, lang, &selected_vals, base)
                                    {
                                        for w in vp_writes {
                                            writes.push(w);
                                        }
                                    }
                                },
                            ); // end right region
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
                            let (a, b) = if anchor <= addr {
                                (anchor, addr)
                            } else {
                                (addr, anchor)
                            };
                            self.selected_addrs.clear();
                            for x in a..=b {
                                self.selected_addrs.insert(x);
                                if self.selected_addrs.len() >= 16 {
                                    break;
                                }
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
            self.lang,
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
        eframe::set_value(storage, "lang_v1", &self.lang);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        self.refresh_reg_view();
        self.refresh_log_cache();

        // Cmd+F / Ctrl+F focuses the RegisterGroup search box (if that view is
        // active). COMMAND maps to ⌘ on macOS / Ctrl elsewhere. Consume up-front
        // so the window system doesn't swallow it.
        let find_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::F);
        if ctx.input_mut(|i| i.consume_shortcut(&find_shortcut))
            && matches!(self.selection, Selection::RegisterGroup { .. })
        {
            self.want_focus_search = true;
        }

        // 视图快捷键：仅在没有 TextEdit 持有焦点时生效，避免在搜索框/数值
        // 编辑器里输入字母触发面板切换。
        if !ctx.memory(|m| m.focused().is_some()) {
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, egui::Key::V) {
                    self.value_parse_open = !self.value_parse_open;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::L) {
                    self.log_state.collapsed = !self.log_state.collapsed;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    && !self.selected_addrs.is_empty()
                {
                    self.selected_addrs.clear();
                    self.click_anchor = None;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Slash)
                    && matches!(self.selection, Selection::RegisterGroup { .. })
                {
                    self.want_focus_search = true;
                }
            });
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
            egui::MenuBar::new().ui(ui, |ui| {
                let lang = self.lang;
                ui.menu_button(tr(lang, "menu.file"), |ui| {
                    if ui.button(tr(lang, "menu.file.save")).clicked() {
                        do_save = true;
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui.button(tr(lang, "menu.file.load")).clicked() {
                        do_load = true;
                        ui.close_kind(egui::UiKind::Menu);
                    }
                });
                ui.menu_button(tr(lang, "menu.view"), |ui| {
                    ui.checkbox(
                        &mut self.value_parse_open,
                        tr(lang, "menu.view.show_value_panel"),
                    );
                    if ui
                        .checkbox(
                            &mut self.log_state.open,
                            tr(lang, "menu.view.show_log_panel"),
                        )
                        .clicked()
                    {
                        if !self.log_state.open {
                            self.log_state.collapsed = false;
                        }
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    ui.separator();
                    if ui.button(tr(lang, "menu.view.toggle_theme")).clicked() {
                        self.flavor = if self.flavor.is_dark() {
                            Flavor::Latte
                        } else {
                            Flavor::Mocha
                        };
                        theme::apply(ctx, self.flavor);
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    ui.separator();
                    ui.label(tr(lang, "menu.view.theme_label"));
                    for f in [
                        Flavor::Mocha,
                        Flavor::Macchiato,
                        Flavor::Frappe,
                        Flavor::Latte,
                    ] {
                        if ui.radio_value(&mut self.flavor, f, f.label()).clicked() {
                            theme::apply(ctx, self.flavor);
                            ui.close_kind(egui::UiKind::Menu);
                        }
                    }
                    ui.separator();
                    let zoom = ctx.zoom_factor();
                    if ui
                        .button(format!(
                            "{}  ({:.0}%)",
                            tr(lang, "menu.view.zoom_in"),
                            zoom * 100.0
                        ))
                        .clicked()
                    {
                        ctx.set_zoom_factor((zoom + 0.1).min(3.0));
                    }
                    if ui.button(tr(lang, "menu.view.zoom_out")).clicked() {
                        ctx.set_zoom_factor((zoom - 0.1).max(0.5));
                    }
                    if ui.button(tr(lang, "menu.view.zoom_reset")).clicked() {
                        ctx.set_zoom_factor(1.0);
                    }
                });
                ui.menu_button(tr(lang, "menu.language"), |ui| {
                    if ui
                        .radio_value(&mut self.lang, Lang::Zh, Lang::Zh.native_label())
                        .clicked()
                    {
                        ui.close_kind(egui::UiKind::Menu);
                    }
                    if ui
                        .radio_value(&mut self.lang, Lang::En, Lang::En.native_label())
                        .clicked()
                    {
                        ui.close_kind(egui::UiKind::Menu);
                    }
                });
                ui.menu_button(tr(lang, "menu.help"), |ui| {
                    ui.label(tr(lang, "menu.help.about"));
                    ui.hyperlink_to("GitHub", "https://github.com/kelsoprotein-lab/ModbusSim");
                });
            });
        });

        let mut tree_action: Option<TreeAction> = None;

        egui::SidePanel::left("connections")
            .resizable(true)
            .default_width(240.0)
            .min_width(200.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::bg_of(self.flavor, theme::Layer::L0))
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                ui.allocate_ui_with_layout(
                    ui.available_size(),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        // —— 头部：tiny_caps "连接" + 右上 + 新建 ——
                        egui::Frame::new()
                            .inner_margin(egui::Margin {
                                left: 14,
                                right: 10,
                                top: 12,
                                bottom: 8,
                            })
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    theme::text::tiny_caps(
                                        ui,
                                        self.flavor,
                                        tr(self.lang, "sidebar.connections"),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if uikit::secondary_button_sm(
                                                ui,
                                                self.flavor,
                                                tr(self.lang, "sidebar.new"),
                                            )
                                            .clicked()
                                            {
                                                self.show_new_tcp_dialog =
                                                    !self.show_new_tcp_dialog;
                                            }
                                        },
                                    );
                                });
                            });

                        // —— 新建 TCP 表单（可折叠）——
                        if self.show_new_tcp_dialog {
                            egui::Frame::new()
                                .fill(theme::bg_of(self.flavor, theme::Layer::L2))
                                .inner_margin(egui::Margin {
                                    left: 14,
                                    right: 10,
                                    top: 6,
                                    bottom: 8,
                                })
                                .show(ui, |ui| {
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
                                    ui.add_space(4.0);
                                    ui.checkbox(
                                        &mut self.new_use_tls,
                                        tr(self.lang, "sidebar.enable_tls"),
                                    );
                                    if self.new_use_tls {
                                        ui.add_space(2.0);
                                        egui::Grid::new("new_tcp_tls_form")
                                            .num_columns(2)
                                            .spacing([8.0, 4.0])
                                            .show(ui, |ui| {
                                                ui.label("Cert (PEM)");
                                                ui.text_edit_singleline(&mut self.new_cert_file);
                                                ui.end_row();
                                                ui.label("Key (PEM)");
                                                ui.text_edit_singleline(&mut self.new_key_file);
                                                ui.end_row();
                                                ui.label("PKCS#12");
                                                ui.text_edit_singleline(&mut self.new_pkcs12_file);
                                                ui.end_row();
                                                ui.label(tr(self.lang, "sidebar.pkcs12_password"));
                                                ui.add(
                                                    egui::TextEdit::singleline(
                                                        &mut self.new_pkcs12_password,
                                                    )
                                                    .password(true),
                                                );
                                                ui.end_row();
                                                ui.label(tr(self.lang, "sidebar.ca_optional"));
                                                ui.text_edit_singleline(&mut self.new_ca_file);
                                                ui.end_row();
                                            });
                                        ui.checkbox(
                                            &mut self.new_require_client_cert,
                                            tr(self.lang, "sidebar.require_client_cert"),
                                        );
                                        theme::text::crumb(
                                            ui,
                                            self.flavor,
                                            tr(self.lang, "sidebar.tls_hint"),
                                        );
                                        ui.add_space(4.0);
                                    }
                                    ui.horizontal(|ui| {
                                        if uikit::primary_button(
                                            ui,
                                            self.flavor,
                                            tr(self.lang, "sidebar.create"),
                                        )
                                        .clicked()
                                        {
                                            tree_action = Some(TreeAction::Create);
                                            self.show_new_tcp_dialog = false;
                                        }
                                        if uikit::link_action(
                                            ui,
                                            self.flavor,
                                            tr(self.lang, "sidebar.cancel"),
                                            false,
                                        )
                                        .clicked()
                                        {
                                            self.show_new_tcp_dialog = false;
                                        }
                                    });
                                });
                        }

                        // —— 树：可滚动区（为 footer 留 40px）——
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .max_height(ui.available_height() - 40.0)
                            .show(ui, |ui| {
                                egui::Frame::new()
                                    .inner_margin(egui::Margin {
                                        left: 8,
                                        right: 8,
                                        top: 0,
                                        bottom: 0,
                                    })
                                    .show(ui, |ui| {
                                        if let Some(a) = self.render_tree(ui) {
                                            tree_action = Some(a);
                                        }
                                    });
                            });

                        // —— footer：停止 / 删除连接 ——
                        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                            egui::Frame::new()
                                .fill(theme::bg_of(self.flavor, theme::Layer::L0))
                                .stroke(egui::Stroke::new(1.0, theme::border_subtle(self.flavor)))
                                .inner_margin(egui::Margin {
                                    left: 14,
                                    right: 14,
                                    top: 8,
                                    bottom: 10,
                                })
                                .show(ui, |ui| {
                                    // Derive active connection id + state from selection
                                    let active_conn =
                                        selection_conn_id(&self.selection).and_then(|id| {
                                            self.conn_snapshot.iter().find(|s| s.id == id)
                                        });
                                    if let Some(snap) = active_conn {
                                        let conn_id = snap.id.clone();
                                        let conn_label_short = snap.label.clone();
                                        let now = std::time::Instant::now();
                                        let confirming = self
                                            .pending_delete
                                            .as_ref()
                                            .filter(|(id, t)| {
                                                id == &conn_id
                                                    && now.duration_since(*t).as_secs_f32() < 3.0
                                            })
                                            .is_some();
                                        let label: String = if confirming {
                                            tr(self.lang, "sidebar.confirm_delete").to_string()
                                        } else {
                                            tr1(
                                                self.lang,
                                                "sidebar.delete_conn_fmt",
                                                &conn_label_short,
                                            )
                                        };
                                        ui.horizontal(|ui| {
                                            if uikit::danger_button_sm(ui, self.flavor, label)
                                                .clicked()
                                            {
                                                if confirming {
                                                    tree_action =
                                                        Some(TreeAction::RemoveConn(conn_id));
                                                    self.pending_delete = None;
                                                } else {
                                                    self.pending_delete = Some((conn_id, now));
                                                    ctx.request_repaint_after(
                                                        std::time::Duration::from_millis(3100),
                                                    );
                                                }
                                            }
                                        });
                                    }
                                });
                        });
                    },
                );
            });

        let mut clear_error = false;
        let mut clear_status = false;
        let conn_count = self.conn_snapshot.len();
        let any_running = self
            .conn_snapshot
            .iter()
            .any(|s| matches!(s.state, ConnectionState::Running));
        let zero_conns = conn_count == 0;
        let slave_count: usize = self.conn_snapshot.iter().map(|c| c.devices.len()).sum();
        let flavor = self.flavor;
        if any_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        egui::TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .exact_height(22.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::bg_of(flavor, theme::Layer::L0))
                    .inner_margin(egui::Margin::symmetric(14.0 as i8, 4.0 as i8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(err) = &self.last_error {
                        ui.add(egui::Label::new(
                            egui::RichText::new("●")
                                .color(theme::danger(flavor))
                                .size(11.0),
                        ));
                        ui.add(egui::Label::new(
                            egui::RichText::new(err)
                                .color(theme::danger(flavor))
                                .size(11.0),
                        ));
                        if uikit::link_action(ui, flavor, tr(self.lang, "sidebar.clear"), false)
                            .clicked()
                        {
                            clear_error = true;
                        }
                    } else if let Some(msg) = &self.status_msg {
                        ui.add(egui::Label::new(
                            egui::RichText::new("●")
                                .color(theme::success(flavor))
                                .size(11.0),
                        ));
                        ui.add(egui::Label::new(
                            egui::RichText::new(msg)
                                .color(theme::success(flavor))
                                .size(11.0),
                        ));
                        if uikit::link_action(ui, flavor, tr(self.lang, "sidebar.clear"), false)
                            .clicked()
                        {
                            clear_status = true;
                        }
                    } else {
                        let (dot_color, dot_alpha, status_text, text_color) = if zero_conns {
                            (
                                theme::text_muted(flavor),
                                255u8,
                                tr(self.lang, "conn.state.disconnected"),
                                theme::text_muted(flavor),
                            )
                        } else if any_running {
                            let phase = (ui.input(|i| i.time) * (2.0 * std::f64::consts::PI / 1.5))
                                .sin()
                                * 0.5
                                + 0.5;
                            let alpha = (180.0 + 75.0 * phase) as u8;
                            (
                                theme::success(flavor),
                                alpha,
                                tr(self.lang, "conn.state.running"),
                                theme::success(flavor),
                            )
                        } else {
                            (
                                theme::text_muted(flavor),
                                255u8,
                                tr(self.lang, "conn.state.stopped"),
                                theme::text_muted(flavor),
                            )
                        };
                        let dot = if zero_conns || !any_running {
                            "○"
                        } else {
                            "●"
                        };
                        let dot_color_with_alpha = egui::Color32::from_rgba_unmultiplied(
                            dot_color.r(),
                            dot_color.g(),
                            dot_color.b(),
                            dot_alpha,
                        );
                        ui.add(egui::Label::new(
                            egui::RichText::new(dot)
                                .color(dot_color_with_alpha)
                                .size(11.0),
                        ));
                        ui.add(egui::Label::new(
                            egui::RichText::new(status_text)
                                .color(text_color)
                                .size(11.0),
                        ));
                    }
                    ui.add_space(14.0);
                    theme::text::crumb(
                        ui,
                        flavor,
                        &tr2(self.lang, "conn.summary_fmt", conn_count, slave_count),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        theme::text::crumb(ui, flavor, env!("CARGO_PKG_VERSION"));
                    });
                });
            });
        if clear_error {
            self.last_error = None;
        }
        if clear_status {
            self.status_msg = None;
        }

        self.render_log_panel(ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::bg_of(self.flavor, theme::Layer::L1))
                    .inner_margin(egui::Margin::symmetric(14.0 as i8, 10.0 as i8)),
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

#[cfg(test)]
mod hero_pulse_tests {
    use super::amp_from_counts;

    #[test]
    fn amp_zero_when_silent() {
        assert_eq!(amp_from_counts(0), 0.0);
    }

    #[test]
    fn amp_saturates_at_one() {
        assert_eq!(amp_from_counts(40), 1.0);
        assert_eq!(amp_from_counts(100), 1.0);
        assert_eq!(amp_from_counts(u32::MAX), 1.0);
    }

    #[test]
    fn amp_linear_in_between() {
        // 20 条 → 0.5，允许浮点误差
        let v = amp_from_counts(20);
        assert!((v - 0.5).abs() < 1e-6, "got {}", v);
    }
}
