use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use modbussim_core::log_collector::LogCollector;
use modbussim_core::log_entry::{Direction, LogEntry};
use modbussim_core::register::{decode_value, DataType, Endian, RegisterDef, RegisterType};
use modbussim_core::slave::{ConnectionState, SlaveConnection, SlaveDevice};
use modbussim_core::transport::Transport;
use modbussim_ui_shared::format::{format_u16, U16Format};
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
}

pub struct SlaveApp {
    rt: Arc<Runtime>,
    connections: SharedConnections,
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

    // Register table display mode
    reg_display_mode: ValueDisplayMode,

    // Log panel
    log_panel_open: bool,
    log_cache: Vec<LogEntry>,
    log_cache_conn_id: Option<String>,
    log_last_refresh: Option<Instant>,
    log_filter: LogFilter,
}

#[derive(Default)]
pub struct LogFilter {
    pub text: String,
    pub show_rx: bool,
    pub show_tx: bool,
}

impl LogFilter {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            show_rx: true,
            show_tx: true,
        }
    }

    pub fn accepts(&self, entry: &LogEntry) -> bool {
        match entry.direction {
            Direction::Rx if !self.show_rx => return false,
            Direction::Tx if !self.show_tx => return false,
            _ => {}
        }
        if !self.text.is_empty() {
            let q = self.text.to_lowercase();
            if !entry.detail.to_lowercase().contains(&q)
                && !entry.function_code.name().to_lowercase().contains(&q)
            {
                return false;
            }
        }
        true
    }
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
    pub fn new(rt: Arc<Runtime>) -> Self {
        let (events_tx, events_rx) = crossbeam_channel::unbounded();
        Self {
            rt,
            connections: Arc::new(RwLock::new(Vec::new())),
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
            reg_display_mode: ValueDisplayMode::U16,
            log_panel_open: true,
            log_cache: Vec::new(),
            log_cache_conn_id: None,
            log_last_refresh: None,
            log_filter: LogFilter::new(),
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

        self.reg_view = Some(RegViewCache {
            conn_id,
            slave_id,
            reg_type,
            row_count,
            u16_map,
            bool_map,
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
/// U16/I16 are single-word; F32/U32/I32 consume 2 consecutive words.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ValueDisplayMode {
    U16,
    I16,
    F32(Endian),
    U32(Endian),
    I32(Endian),
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
        }
    }
    pub fn is_multi_word(&self) -> bool {
        matches!(self, Self::F32(_) | Self::U32(_) | Self::I32(_))
    }
    pub fn stride(&self) -> usize {
        if self.is_multi_word() { 2 } else { 1 }
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
            }
            TreeAction::SelectDevice { conn_id, slave_id } => {
                self.selection = Selection::Device { conn_id, slave_id };
                self.pending_edits.clear();
            }
            TreeAction::SelectGroup { conn_id, slave_id, reg_type } => {
                self.selection = Selection::RegisterGroup { conn_id, slave_id, reg_type };
                self.pending_edits.clear();
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
                ui.heading("ModbusSlave — egui edition");
                ui.label("从左侧创建或选中一个连接/设备/寄存器组。");
            }
            Selection::Connection(id) => {
                let exists = self.conn_snapshot.iter().any(|s| &s.id == id);
                if exists {
                    let (label, state, dev_count) = {
                        let s = self.conn_snapshot.iter().find(|s| &s.id == id).unwrap();
                        (s.label.clone(), s.state, s.devices.len())
                    };
                    ui.heading(label);
                    ui.label(format!(
                        "状态: {} · 设备数: {}",
                        match state {
                            ConnectionState::Running => "运行中",
                            ConnectionState::Stopped => "已停止",
                        },
                        dev_count
                    ));
                    ui.separator();
                    if ui.button("新增从站…").clicked() {
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
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("批量添加寄存器…").clicked() {
                                self.open_batch_modal(
                                    conn_id.clone(),
                                    *slave_id,
                                    RegisterType::HoldingRegister,
                                );
                            }
                            if ui.button("删除此从站").clicked() {
                                self.remove_device(conn_id.clone(), *slave_id, ui.ctx().clone());
                            }
                        });
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
                ui.horizontal(|ui| {
                    ui.heading(format!("{} · 连接 {} · 从站 {}", group_label, conn_id, slave_id));
                    if ui.button("批量添加…").clicked() {
                        self.open_batch_modal(conn_id.clone(), *slave_id, *reg_type);
                    }
                });
                ui.separator();

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
                ui.separator();

                let row_h = 20.0;
                let reg_type_v = *reg_type;
                let conn_id_for_writes = conn_id.clone();
                let slave_id_v = *slave_id;
                let pending = &mut self.pending_edits;
                let mut writes: Vec<(u16, u16)> = Vec::new();

                if !is_bool && mode.is_multi_word() {
                    let stride = mode.stride();
                    let pair_rows = view.row_count / stride;
                    let (dtype, endian) = match mode {
                        ValueDisplayMode::F32(e) => (DataType::Float32, e),
                        ValueDisplayMode::U32(e) => (DataType::UInt32, e),
                        ValueDisplayMode::I32(e) => (DataType::Int32, e),
                        _ => (DataType::UInt16, Endian::Big),
                    };
                    TableBuilder::new(ui)
                        .striped(true)
                        .resizable(true)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(Column::exact(100.0))
                        .column(Column::exact(200.0))
                        .column(Column::exact(160.0))
                        .column(Column::remainder())
                        .header(22.0, |mut h| {
                            h.col(|ui| { ui.strong("地址"); });
                            h.col(|ui| { ui.strong(mode.label()); });
                            h.col(|ui| { ui.strong("Raw (Hex)"); });
                            h.col(|ui| { ui.strong(""); });
                        })
                        .body(|body| {
                            body.rows(row_h, pair_rows, |mut row| {
                                let base = row.index() as u16 * stride as u16;
                                let addr0 = base;
                                let addr1 = base + 1;
                                let r0 = view
                                    .u16_map
                                    .as_ref()
                                    .and_then(|m| m.get(&addr0).copied());
                                let r1 = view
                                    .u16_map
                                    .as_ref()
                                    .and_then(|m| m.get(&addr1).copied());
                                row.col(|ui| {
                                    ui.monospace(format!("{}..{}", addr0, addr1));
                                });
                                row.col(|ui| match (r0, r1) {
                                    (Some(a), Some(b)) => {
                                        let decoded = decode_value(&[a, b], dtype, endian)
                                            .unwrap_or(f64::NAN);
                                        let text = match dtype {
                                            DataType::Float32 => format!("{:.6}", decoded as f32),
                                            DataType::UInt32 => format!("{}", decoded as u32),
                                            DataType::Int32 => format!("{}", decoded as i32),
                                            _ => "?".to_string(),
                                        };
                                        ui.monospace(text);
                                    }
                                    _ => { ui.monospace("—"); }
                                });
                                row.col(|ui| match (r0, r1) {
                                    (Some(a), Some(b)) => {
                                        ui.monospace(format!("{:04X} {:04X}", a, b));
                                    }
                                    _ => { ui.monospace(""); }
                                });
                                row.col(|_| {});
                            });
                        });
                } else {
                    TableBuilder::new(ui)
                        .striped(true)
                        .resizable(true)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(Column::exact(80.0))
                        .column(Column::exact(110.0))
                        .column(Column::exact(100.0))
                        .column(Column::exact(140.0))
                        .column(Column::remainder())
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
                            body.rows(row_h, view.row_count, |mut row| {
                                let addr = row.index() as u16;
                                row.col(|ui| {
                                    ui.monospace(format!("{}", addr));
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
                                        let mut tmp = pending
                                            .get(&key)
                                            .map(|v| *v != 0)
                                            .unwrap_or(cache_bool);
                                        let resp = ui.checkbox(&mut tmp, "");
                                        if resp.clicked() {
                                            writes.push((addr, if tmp { 1 } else { 0 }));
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
        if !self.log_panel_open { return; }

        enum LogAction {
            Clear,
            Export,
            Close,
        }
        let mut action: Option<LogAction> = None;

        egui::TopBottomPanel::bottom("log_panel")
            .resizable(true)
            .default_height(220.0)
            .min_height(80.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("通信日志");
                    if let Some(id) = &self.log_cache_conn_id {
                        ui.label(format!("· 连接 {}", id));
                        ui.label(format!("({} 条)", self.log_cache.len()));
                    } else {
                        ui.label("（选中连接以查看）");
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✕").on_hover_text("关闭日志面板").clicked() {
                            action = Some(LogAction::Close);
                        }
                        if ui.small_button("导出 CSV").clicked() {
                            action = Some(LogAction::Export);
                        }
                        if ui.small_button("清空").clicked() {
                            action = Some(LogAction::Clear);
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.log_filter.show_rx, "RX");
                    ui.checkbox(&mut self.log_filter.show_tx, "TX");
                    ui.label("过滤");
                    ui.text_edit_singleline(&mut self.log_filter.text);
                });
                ui.separator();

                let filter = &self.log_filter;
                let entries: Vec<&LogEntry> = self
                    .log_cache
                    .iter()
                    .rev()
                    .filter(|e| filter.accepts(e))
                    .collect();

                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::exact(150.0))
                    .column(Column::exact(40.0))
                    .column(Column::exact(60.0))
                    .column(Column::remainder())
                    .header(20.0, |mut h| {
                        h.col(|ui| { ui.strong("时间"); });
                        h.col(|ui| { ui.strong("方向"); });
                        h.col(|ui| { ui.strong("FC"); });
                        h.col(|ui| { ui.strong("详情"); });
                    })
                    .body(|body| {
                        body.rows(18.0, entries.len(), |mut row| {
                            let e = entries[row.index()];
                            row.col(|ui| {
                                ui.monospace(
                                    e.timestamp.format("%H:%M:%S%.3f").to_string(),
                                );
                            });
                            row.col(|ui| {
                                let (text, color) = match e.direction {
                                    Direction::Rx => ("RX", egui::Color32::from_rgb(80, 160, 255)),
                                    Direction::Tx => ("TX", egui::Color32::from_rgb(255, 160, 80)),
                                };
                                ui.colored_label(color, text);
                            });
                            row.col(|ui| {
                                ui.monospace(e.function_code.name());
                            });
                            row.col(|ui| {
                                ui.monospace(&e.detail);
                            });
                        });
                    });
            });

        match action {
            Some(LogAction::Clear) => self.clear_logs_for_selection(),
            Some(LogAction::Export) => self.export_logs_for_selection(ctx.clone()),
            Some(LogAction::Close) => self.log_panel_open = false,
            None => {}
        }
    }
}

impl eframe::App for SlaveApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        self.refresh_reg_view();
        self.refresh_log_cache();

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
                    if ui.checkbox(&mut self.log_panel_open, "显示日志面板").clicked() {
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("深色主题").clicked() {
                        ctx.set_visuals(egui::Visuals::dark());
                        ui.close_menu();
                    }
                    if ui.button("浅色主题").clicked() {
                        ctx.set_visuals(egui::Visuals::light());
                        ui.close_menu();
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

        egui::CentralPanel::default().show(ctx, |ui| {
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
