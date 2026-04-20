use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use modbussim_core::log_collector::LogCollector;
use modbussim_core::log_entry::LogEntry;
use modbussim_core::master::{
    MasterConfig, MasterConnection, MasterState, PollEvent, ReadFunction, ReadResult, ScanGroup,
};
use modbussim_core::transport::Transport;
use modbussim_ui_shared::icons;
use modbussim_ui_shared::log_panel::{self, LogPanelAction, LogPanelState};
use modbussim_ui_shared::project::{
    deserialize_master, serialize_master, MasterConnectionSave, MasterProject, PollSave, TcpSpec,
};
use modbussim_ui_shared::theme::{self, Flavor};
use modbussim_ui_shared::ui as uikit;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;


pub struct MasterConnectionEntry {
    pub id: String,
    pub label: String,
    pub connection: Arc<RwLock<MasterConnection>>,
    pub log_collector: Arc<LogCollector>,
}

pub type SharedConnections = Arc<RwLock<Vec<MasterConnectionEntry>>>;

pub enum UiEvent {
    ConnectionCreated { id: String, label: String, slave_id: u8 },
    ConnectionStateChanged { id: String, state: MasterState },
    ConnectionRemoved(String),
    ReadDone { id: String, result: ReadResult },
    PollStarted { id: String, group_id: String },
    PollStopped { id: String, group_id: String },
    PollUpdate { id: String, group_id: String, result: ReadResult },
    PollError { id: String, group_id: String, msg: String },
    PollConfigLoaded {
        id: String,
        group_id: String,
        fc: ReadFunction,
        addr: u16,
        qty: u16,
        interval_ms: u64,
    },
    Info(String),
    Error(String),
}

/// One scan group belonging to a master connection.
#[derive(Clone)]
pub struct ScanGroupUi {
    pub id: String,           // stable id, used as core::ScanGroup.id
    pub name: String,         // user-facing label
    pub fc: ReadFunction,
    pub addr: u16,
    pub qty: u16,
    pub interval_ms: u64,
    pub enabled: bool,
    pub latest: Option<ReadResult>,
    pub last_update: Option<Instant>,
    pub last_error: Option<String>,
}

impl ScanGroupUi {
    pub fn new_with_id(id: String) -> Self {
        Self {
            name: id.clone(),
            id,
            fc: ReadFunction::ReadHoldingRegisters,
            addr: 0,
            qty: 10,
            interval_ms: 500,
            enabled: false,
            latest: None,
            last_update: None,
            last_error: None,
        }
    }
}

#[derive(Clone)]
struct ConnSnap {
    id: String,
    label: String,
    state: MasterState,
    slave_id: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MasterTab {
    Read,
    Write,
    Poll,
}

impl MasterTab {
    fn label(self) -> &'static str {
        match self {
            MasterTab::Read => "读取",
            MasterTab::Write => "写入",
            MasterTab::Poll => "轮询",
        }
    }
}

pub struct MasterApp {
    rt: Arc<Runtime>,
    connections: SharedConnections,
    events_tx: crossbeam_channel::Sender<UiEvent>,
    events_rx: crossbeam_channel::Receiver<UiEvent>,

    selected: Option<String>,
    snap: Vec<ConnSnap>,
    next_seq: Arc<AtomicU64>,

    // New-connection form
    new_host: String,
    new_port: String,
    new_slave_id: u8,
    new_timeout: u64,

    // Read form
    read_fc: ReadFunction,
    read_addr: u16,
    read_qty: u16,
    read_result: Option<ReadResult>,

    // Write form
    write_addr: u16,
    write_value: i32,
    write_is_coil: bool,

    // Polling (per connection): ordered list of scan groups + current selection index
    polling: HashMap<String, Vec<ScanGroupUi>>,
    selected_group: HashMap<String, usize>,
    next_group_seq: u64,

    // Log panel
    log_state: LogPanelState,
    log_cache: Vec<LogEntry>,
    log_cache_conn_id: Option<String>,
    log_last_refresh: Option<Instant>,

    last_error: Option<String>,
    status_msg: Option<String>,

    pub flavor: Flavor,
    active_tab: MasterTab,
}

impl MasterApp {
    pub fn new(rt: Arc<Runtime>, flavor: Flavor) -> Self {
        let (events_tx, events_rx) = crossbeam_channel::unbounded();
        Self {
            rt,
            connections: Arc::new(RwLock::new(Vec::new())),
            events_tx,
            events_rx,
            selected: None,
            snap: Vec::new(),
            next_seq: Arc::new(AtomicU64::new(1)),
            new_host: "127.0.0.1".to_string(),
            new_port: "5502".to_string(),
            new_slave_id: 1,
            new_timeout: 3000,
            read_fc: ReadFunction::ReadHoldingRegisters,
            read_addr: 0,
            read_qty: 10,
            read_result: None,
            write_addr: 0,
            write_value: 0,
            write_is_coil: false,
            polling: HashMap::new(),
            selected_group: HashMap::new(),
            next_group_seq: 1,
            log_state: LogPanelState::new(),
            log_cache: Vec::new(),
            log_cache_conn_id: None,
            log_last_refresh: None,
            last_error: None,
            status_msg: None,
            flavor,
            active_tab: MasterTab::Read,
        }
    }

    fn allocate_id(&self) -> String {
        let n = self.next_seq.fetch_add(1, Ordering::Relaxed);
        format!("master_{}", n)
    }

    fn create_connection(&self, ctx: egui::Context) {
        let host = self.new_host.trim().to_string();
        let port: u16 = match self.new_port.trim().parse() {
            Ok(p) => p,
            Err(_) => {
                let _ = self
                    .events_tx
                    .send(UiEvent::Error(format!("无效端口: {}", self.new_port)));
                return;
            }
        };
        let slave_id = self.new_slave_id;
        let timeout_ms = self.new_timeout;
        let id = self.allocate_id();
        let label = format!("TCP {}:{} · 从站 {}", host, port, slave_id);

        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        let id_c = id.clone();
        let label_c = label.clone();
        self.rt.spawn(async move {
            let log_collector = Arc::new(LogCollector::new());
            let config = MasterConfig {
                target_address: host.clone(),
                port,
                slave_id,
                timeout_ms,
                ..Default::default()
            };
            let connection = MasterConnection::new(config, Transport::Tcp { host, port })
                .with_log_collector(log_collector.clone());
            connections.write().await.push(MasterConnectionEntry {
                id: id_c.clone(),
                label: label_c.clone(),
                connection: Arc::new(RwLock::new(connection)),
                log_collector,
            });
            let _ = tx.send(UiEvent::ConnectionCreated {
                id: id_c,
                label: label_c,
                slave_id,
            });
        });
        ctx.request_repaint();
    }

    fn connect(&self, id: &str, ctx: egui::Context) {
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
                let mut g = conn_arc.write().await;
                g.connect().await
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(UiEvent::ConnectionStateChanged {
                        id,
                        state: MasterState::Connected,
                    });
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("连接失败: {e}")));
                }
            }
        });
        ctx.request_repaint();
    }

    fn disconnect(&self, id: &str, ctx: egui::Context) {
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
                let mut g = conn_arc.write().await;
                g.disconnect().await
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(UiEvent::ConnectionStateChanged {
                        id,
                        state: MasterState::Disconnected,
                    });
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("断开失败: {e}")));
                }
            }
        });
        ctx.request_repaint();
    }

    fn remove_connection(&mut self, id: &str, ctx: egui::Context) {
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
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
                let mut g = conn_arc.write().await;
                let _ = g.disconnect().await;
            }
            connections.write().await.retain(|e| e.id != id);
            let _ = tx.send(UiEvent::ConnectionRemoved(id));
        });
        ctx.request_repaint();
    }

    fn find_group_mut(&mut self, conn_id: &str, group_id: &str) -> Option<&mut ScanGroupUi> {
        self.polling
            .get_mut(conn_id)
            .and_then(|v| v.iter_mut().find(|g| g.id == group_id))
    }

    fn start_poll(&self, id: String, group_idx: usize, ctx: egui::Context) {
        let Some(group) = self
            .polling
            .get(&id)
            .and_then(|v| v.get(group_idx))
            .cloned()
        else {
            return;
        };
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        let ctx2 = ctx.clone();
        let group_id = group.id.clone();
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

            let core_group = ScanGroup {
                id: group.id.clone(),
                name: group.name.clone(),
                function: group.fc,
                start_address: group.addr,
                quantity: group.qty,
                interval_ms: group.interval_ms,
                enabled: true,
                slave_id: None,
            };

            let rx = {
                let mut guard = conn_arc.write().await;
                match guard.start_scan_group(&core_group).await {
                    Ok(rx) => rx,
                    Err(e) => {
                        let _ = tx.send(UiEvent::Error(format!("启动轮询失败: {e}")));
                        return;
                    }
                }
            };

            let _ = tx.send(UiEvent::PollStarted {
                id: id.clone(),
                group_id: group_id.clone(),
            });
            let mut rx = rx;
            while let Some(ev) = rx.recv().await {
                match ev {
                    PollEvent::Data(r) => {
                        let _ = tx.send(UiEvent::PollUpdate {
                            id: id.clone(),
                            group_id: group_id.clone(),
                            result: r,
                        });
                        ctx2.request_repaint();
                    }
                    PollEvent::Error(e) => {
                        let _ = tx.send(UiEvent::PollError {
                            id: id.clone(),
                            group_id: group_id.clone(),
                            msg: e,
                        });
                    }
                }
            }
            let _ = tx.send(UiEvent::PollStopped { id, group_id });
        });
        ctx.request_repaint();
    }

    fn stop_poll(&self, id: String, group_idx: usize, ctx: egui::Context) {
        let Some(gid) = self
            .polling
            .get(&id)
            .and_then(|v| v.get(group_idx))
            .map(|g| g.id.clone())
        else {
            return;
        };
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.connection.clone());
            if let Some(conn_arc) = conn_arc {
                let mut guard = conn_arc.write().await;
                if let Err(e) = guard.stop_scan_group(&gid).await {
                    let _ = tx.send(UiEvent::Error(format!("停止轮询失败: {e}")));
                }
            }
        });
        ctx.request_repaint();
    }

    fn add_scan_group(&mut self, conn_id: String) {
        let gid = format!("group_{}", self.next_group_seq);
        self.next_group_seq += 1;
        let mut g = ScanGroupUi::new_with_id(gid);
        g.name = format!("组 {}", self.polling.get(&conn_id).map(|v| v.len() + 1).unwrap_or(1));
        let list = self.polling.entry(conn_id.clone()).or_default();
        list.push(g);
        let new_idx = list.len() - 1;
        self.selected_group.insert(conn_id, new_idx);
    }

    fn remove_scan_group(&mut self, conn_id: String, group_idx: usize, ctx: egui::Context) {
        // Ensure the group is stopped before removal.
        self.stop_poll(conn_id.clone(), group_idx, ctx);
        if let Some(list) = self.polling.get_mut(&conn_id) {
            if group_idx < list.len() {
                list.remove(group_idx);
                let new_sel = if list.is_empty() { 0 } else { group_idx.saturating_sub(1).min(list.len() - 1) };
                self.selected_group.insert(conn_id, new_sel);
            }
        }
    }

    fn do_read(&self, id: String, ctx: egui::Context) {
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        let fc = self.read_fc;
        let addr = self.read_addr;
        let qty = self.read_qty;
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
            let conn = conn_arc.read().await;
            match conn.read(fc, addr, qty).await {
                Ok(result) => {
                    let _ = tx.send(UiEvent::ReadDone { id, result });
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("读取失败: {e}")));
                }
            }
        });
        ctx.request_repaint();
    }

    fn do_write(&self, id: String, ctx: egui::Context) {
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        let addr = self.write_addr;
        let value = self.write_value;
        let is_coil = self.write_is_coil;
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
            let conn = conn_arc.read().await;
            let result = if is_coil {
                conn.write_single_coil(addr, value != 0).await
            } else {
                let v = value.clamp(0, u16::MAX as i32) as u16;
                conn.write_single_register(addr, v).await
            };
            match result {
                Ok(()) => {
                    let _ = tx.send(UiEvent::Info(format!("写入成功 · {id}")));
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("写入失败: {e}")));
                }
            }
        });
        ctx.request_repaint();
    }

    /// Gather the current connection list + poll configs into a serializable project.
    fn build_project(&self) -> MasterProject {
        let mut proj = MasterProject::new();
        for s in &self.snap {
            // Parse "TCP host:port · 从站 N" back into parts is fragile; instead
            // rebuild host/port from polling's conn entry via the core MasterConnection
            // is awkward. We keep the label plus conservative defaults.
            //
            // For the MVP we store what the UI currently knows: label + slave_id + poll.
            // host/port is unknown at the snapshot level — we fetch from the async side
            // synchronously via try_read. Fallback to 127.0.0.1:502 if contended.
            let (host, port, timeout_ms) = self
                .connections
                .try_read()
                .ok()
                .and_then(|list| {
                    list.iter().find(|e| e.id == s.id).and_then(|e| {
                        e.connection.try_read().ok().map(|c| {
                            let (h, p) = match &c.transport {
                                Transport::Tcp { host, port } => (host.clone(), *port),
                                _ => ("127.0.0.1".to_string(), 502),
                            };
                            (h, p, c.config.timeout_ms)
                        })
                    })
                })
                .unwrap_or_else(|| ("127.0.0.1".to_string(), 502, 3000));

            // Schema v2 currently stores only one poll per connection. When a
            // connection has multiple scan groups, persist the first one only;
            // v3 will upgrade to a `Vec<PollSave>`.
            let poll = self
                .polling
                .get(&s.id)
                .and_then(|v| v.first())
                .map(|p| PollSave {
                    function: match p.fc {
                        ReadFunction::ReadCoils => "read_coils",
                        ReadFunction::ReadDiscreteInputs => "read_discrete_inputs",
                        ReadFunction::ReadHoldingRegisters => "read_holding_registers",
                        ReadFunction::ReadInputRegisters => "read_input_registers",
                    }
                    .to_string(),
                    addr: p.addr,
                    qty: p.qty,
                    interval_ms: p.interval_ms,
                });

            proj.connections.push(MasterConnectionSave {
                label: s.label.clone(),
                tcp: TcpSpec { host, port },
                slave_id: s.slave_id,
                timeout_ms,
                poll,
            });
        }
        proj
    }

    fn save_project(&mut self, ctx: egui::Context) {
        let proj = self.build_project();
        let tx = self.events_tx.clone();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.rt.spawn(async move {
            let Some(path) = rfd::AsyncFileDialog::new()
                .set_file_name(&format!("master_{}.modbusproj", ts))
                .add_filter("ModbusProj", &["modbusproj"])
                .save_file()
                .await
            else {
                return;
            };
            match serialize_master(&proj) {
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
        let next_seq = self.next_seq.clone();
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
            let project = match deserialize_master(&text) {
                Ok(p) => p,
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(format!("解析失败: {e}")));
                    return;
                }
            };
            // Push each saved connection into the app state.
            for c in project.connections {
                let label = c.label.clone();
                let tcp = c.tcp.clone();
                let slave_id = c.slave_id;
                let timeout_ms = c.timeout_ms;
                let log_collector = Arc::new(LogCollector::new());
                let config = MasterConfig {
                    target_address: tcp.host.clone(),
                    port: tcp.port,
                    slave_id,
                    timeout_ms,
                    ..Default::default()
                };
                let connection = MasterConnection::new(
                    config,
                    Transport::Tcp { host: tcp.host.clone(), port: tcp.port },
                )
                .with_log_collector(log_collector.clone());
                let id = format!("master_{}", next_seq.fetch_add(1, Ordering::Relaxed));
                connections_arc.write().await.push(MasterConnectionEntry {
                    id: id.clone(),
                    label: label.clone(),
                    connection: Arc::new(RwLock::new(connection)),
                    log_collector,
                });
                let _ = tx.send(UiEvent::ConnectionCreated {
                    id: id.clone(),
                    label,
                    slave_id,
                });
                if let Some(ps) = &c.poll {
                    let fc = match ps.function.as_str() {
                        "read_coils" => ReadFunction::ReadCoils,
                        "read_discrete_inputs" => ReadFunction::ReadDiscreteInputs,
                        "read_input_registers" => ReadFunction::ReadInputRegisters,
                        _ => ReadFunction::ReadHoldingRegisters,
                    };
                    let _ = tx.send(UiEvent::PollConfigLoaded {
                        id,
                        group_id: "group_loaded_0".to_string(),
                        fc,
                        addr: ps.addr,
                        qty: ps.qty,
                        interval_ms: ps.interval_ms,
                    });
                }
            }
            let _ = tx.send(UiEvent::Info(format!("已加载：{}", file.path().display())));
            ctx2.request_repaint();
        });
    }

    fn drain_events(&mut self) {
        while let Ok(ev) = self.events_rx.try_recv() {
            match ev {
                UiEvent::ConnectionCreated { id, label, slave_id } => {
                    self.snap.push(ConnSnap {
                        id,
                        label,
                        state: MasterState::Disconnected,
                        slave_id,
                    });
                }
                UiEvent::ConnectionStateChanged { id, state } => {
                    if let Some(s) = self.snap.iter_mut().find(|s| s.id == id) {
                        s.state = state;
                    }
                }
                UiEvent::ConnectionRemoved(id) => {
                    self.snap.retain(|s| s.id != id);
                    self.polling.remove(&id);
                    self.selected_group.remove(&id);
                }
                UiEvent::ReadDone { id: _, result } => {
                    self.read_result = Some(result);
                }
                UiEvent::PollStarted { id, group_id } => {
                    if let Some(g) = self.find_group_mut(&id, &group_id) {
                        g.enabled = true;
                        g.last_error = None;
                    }
                }
                UiEvent::PollStopped { id, group_id } => {
                    if let Some(g) = self.find_group_mut(&id, &group_id) {
                        g.enabled = false;
                    }
                }
                UiEvent::PollUpdate { id, group_id, result } => {
                    if let Some(g) = self.find_group_mut(&id, &group_id) {
                        g.latest = Some(result);
                        g.last_update = Some(Instant::now());
                        g.last_error = None;
                    }
                }
                UiEvent::PollError { id, group_id, msg } => {
                    if let Some(g) = self.find_group_mut(&id, &group_id) {
                        g.last_error = Some(msg);
                    }
                }
                UiEvent::PollConfigLoaded { id, group_id, fc, addr, qty, interval_ms } => {
                    let list = self.polling.entry(id).or_default();
                    let mut g = ScanGroupUi::new_with_id(group_id);
                    g.fc = fc;
                    g.addr = addr;
                    g.qty = qty;
                    g.interval_ms = interval_ms;
                    list.push(g);
                }
                UiEvent::Info(msg) => self.status_msg = Some(msg),
                UiEvent::Error(msg) => self.last_error = Some(msg),
            }
        }
    }

    fn refresh_log_cache(&mut self) {
        let Some(id) = self.selected.clone() else {
            self.log_cache.clear();
            self.log_cache_conn_id = None;
            return;
        };
        if self.log_cache_conn_id.as_deref() == Some(id.as_str()) {
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
        self.log_cache_conn_id = Some(id);
        self.log_last_refresh = Some(Instant::now());
    }
}

impl eframe::App for MasterApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "flavor", &self.flavor);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        self.refresh_log_cache();

        // Menu
        let mut do_save = false;
        let mut do_load = false;
        egui::TopBottomPanel::top("master_menu").show(ctx, |ui| {
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
                    ui.checkbox(&mut self.log_state.open, "显示日志面板");
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
                    if ui.button(format!("放大 ({:.0}%)", zoom * 100.0)).clicked() {
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
                    ui.label("ModbusMaster (egui) · 开发预览");
                    ui.hyperlink_to("GitHub", "https://github.com/kelsoprotein-lab/ModbusSim");
                });
            });
        });

        enum Action {
            Create,
            Select(String),
            Connect(String),
            Disconnect(String),
            Remove(String),
        }
        let mut action: Option<Action> = None;

        // Left sidebar
        egui::SidePanel::left("master_connections")
            .resizable(true)
            .default_width(240.0)
            .min_width(200.0)
            .show(ctx, |ui| {
                ui.heading("连接");
                ui.separator();

                ui.collapsing("新建 TCP 连接", |ui| {
                    egui::Grid::new("master_new_form")
                        .num_columns(2)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            ui.label("Host");
                            ui.text_edit_singleline(&mut self.new_host);
                            ui.end_row();
                            ui.label("Port");
                            ui.text_edit_singleline(&mut self.new_port);
                            ui.end_row();
                            ui.label("从站 ID");
                            let mut sid = self.new_slave_id as u32;
                            ui.add(egui::DragValue::new(&mut sid).range(1..=247));
                            self.new_slave_id = sid as u8;
                            ui.end_row();
                            ui.label("超时 (ms)");
                            ui.add(egui::DragValue::new(&mut self.new_timeout).range(100..=60_000));
                            ui.end_row();
                        });
                    if ui.button("创建").clicked() {
                        action = Some(Action::Create);
                    }
                });
                ui.separator();

                for s in &self.snap {
                    let is_sel = self.selected.as_deref() == Some(&s.id);
                    let state_tag = match s.state {
                        MasterState::Connected => "已连接",
                        MasterState::Disconnected => "未连接",
                        MasterState::Reconnecting => "重连中",
                        MasterState::Error => "错误",
                    };
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(is_sel, format!("{} [{}]", s.label, state_tag))
                            .clicked()
                        {
                            action = Some(Action::Select(s.id.clone()));
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        match s.state {
                            MasterState::Connected | MasterState::Reconnecting => {
                                if ui.small_button("断开").clicked() {
                                    action = Some(Action::Disconnect(s.id.clone()));
                                }
                            }
                            _ => {
                                if ui.small_button("连接").clicked() {
                                    action = Some(Action::Connect(s.id.clone()));
                                }
                            }
                        }
                        if ui.small_button("删除").clicked() {
                            action = Some(Action::Remove(s.id.clone()));
                        }
                    });
                    ui.separator();
                }
            });

        // Status + log
        let mut clear_err = false;
        let mut clear_status = false;
        egui::TopBottomPanel::bottom("master_status")
            .resizable(false)
            .show(ctx, |ui| {
                if let Some(err) = &self.last_error {
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::RED, err);
                        if ui.small_button("清除").clicked() { clear_err = true; }
                    });
                } else if let Some(msg) = &self.status_msg {
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::from_rgb(60, 140, 60), msg);
                        if ui.small_button("清除").clicked() { clear_status = true; }
                    });
                } else {
                    ui.label("就绪");
                }
            });
        if clear_err { self.last_error = None; }
        if clear_status { self.status_msg = None; }

        self.render_log_panel(ctx);

        // Central: read/write forms + result
        let mut do_read_id: Option<String> = None;
        let mut do_write_id: Option<String> = None;
        let mut do_start_poll_id: Option<(String, usize)> = None;
        let mut do_stop_poll_id: Option<(String, usize)> = None;
        let flavor = self.flavor;
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(id) = self.selected.clone() else {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    ui.label(
                        egui::RichText::new("ModbusMaster")
                            .size(18.0)
                            .strong(),
                    );
                    uikit::caption(ui, flavor, "从左侧创建并连接一个会话。");
                });
                return;
            };
            let Some(s) = self.snap.iter().find(|s| s.id == id).cloned() else {
                ui.label("连接已不存在。");
                return;
            };
            // Header card: address + status pill, with accent stripe
            uikit::accent_card(ui, flavor, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&s.label).strong().size(13.5));
                    let (txt, color) = match s.state {
                        MasterState::Connected => ("已连接", theme::success(flavor)),
                        MasterState::Disconnected => ("未连接", theme::subtext(flavor)),
                        MasterState::Reconnecting => ("重连中", theme::accent(flavor)),
                        MasterState::Error => ("错误", theme::danger(flavor)),
                    };
                    uikit::status_pill(ui, txt, color);
                });
            });
            ui.add_space(4.0);

            uikit::card(ui, flavor, |ui| {
            // Tab bar: Read / Write / Poll
            ui.horizontal(|ui| {
                for tab in [MasterTab::Read, MasterTab::Write, MasterTab::Poll] {
                    let selected = self.active_tab == tab;
                    let text = if selected {
                        egui::RichText::new(tab.label())
                            .strong()
                            .color(theme::accent(flavor))
                    } else {
                        egui::RichText::new(tab.label()).color(theme::subtext(flavor))
                    };
                    if ui
                        .add(egui::SelectableLabel::new(selected, text))
                        .clicked()
                    {
                        self.active_tab = tab;
                    }
                }
            });
            ui.separator();

            // Tab content
            match self.active_tab {
                MasterTab::Read => {
                    egui::Grid::new("read_form")
                        .num_columns(2)
                        .spacing([10.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("功能码");
                            egui::ComboBox::from_id_salt("read_fc")
                                .selected_text(read_fc_label(self.read_fc))
                                .show_ui(ui, |ui| {
                                    for f in [
                                        ReadFunction::ReadCoils,
                                        ReadFunction::ReadDiscreteInputs,
                                        ReadFunction::ReadHoldingRegisters,
                                        ReadFunction::ReadInputRegisters,
                                    ] {
                                        ui.selectable_value(&mut self.read_fc, f, read_fc_label(f));
                                    }
                                });
                            ui.end_row();
                            ui.label("起始地址");
                            let mut a = self.read_addr as u32;
                            ui.add(egui::DragValue::new(&mut a).range(0..=65535));
                            self.read_addr = a as u16;
                            ui.end_row();
                            ui.label("数量");
                            let mut q = self.read_qty as u32;
                            ui.add(egui::DragValue::new(&mut q).range(1..=2000));
                            self.read_qty = q as u16;
                            ui.end_row();
                        });
                    ui.add_space(8.0);
                    ui.add_enabled_ui(s.state == MasterState::Connected, |ui| {
                        if uikit::primary_button(ui, flavor, "读取").clicked() {
                            do_read_id = Some(id.clone());
                        }
                    });
                }
                MasterTab::Write => {
                    egui::Grid::new("write_form")
                        .num_columns(2)
                        .spacing([10.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("类型");
                            ui.horizontal(|ui| {
                                ui.radio_value(&mut self.write_is_coil, false, "FC06 寄存器");
                                ui.radio_value(&mut self.write_is_coil, true, "FC05 线圈");
                            });
                            ui.end_row();
                            ui.label("地址");
                            let mut a = self.write_addr as u32;
                            ui.add(egui::DragValue::new(&mut a).range(0..=65535));
                            self.write_addr = a as u16;
                            ui.end_row();
                            ui.label("值");
                            if self.write_is_coil {
                                let mut b = self.write_value != 0;
                                ui.checkbox(&mut b, "true / false");
                                self.write_value = if b { 1 } else { 0 };
                            } else {
                                ui.add(
                                    egui::DragValue::new(&mut self.write_value).range(0..=65535),
                                );
                            }
                            ui.end_row();
                        });
                    ui.add_space(8.0);
                    ui.add_enabled_ui(s.state == MasterState::Connected, |ui| {
                        if uikit::primary_button(ui, flavor, "写入").clicked() {
                            do_write_id = Some(id.clone());
                        }
                    });
                }
                MasterTab::Poll => {
                    // Ensure connection has a polling list
                    self.polling.entry(id.clone()).or_default();
                    let sel = *self.selected_group.get(&id).unwrap_or(&0);

                    // Toolbar
                    ui.horizontal(|ui| {
                        if uikit::primary_button(ui, flavor, "+ 新建组").clicked() {
                            self.add_scan_group(id.clone());
                        }
                        let len = self.polling.get(&id).map(|v| v.len()).unwrap_or(0);
                        let has_sel = len > 0 && sel < len;
                        if has_sel {
                            if uikit::danger_button(ui, flavor, "- 删除组").clicked() {
                                self.remove_scan_group(id.clone(), sel, ctx.clone());
                            }
                        }
                        uikit::caption(ui, flavor, format!("{} 个扫描组", len));
                    });
                    ui.add_space(6.0);

                    let is_connected = s.state == MasterState::Connected;

                    ui.horizontal(|ui| {
                        // Left: group list
                        ui.allocate_ui_with_layout(
                            egui::vec2(200.0, ui.available_height()),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    if let Some(list) = self.polling.get(&id) {
                                        for (i, g) in list.iter().enumerate() {
                                            let selected = i == sel;
                                            let dot = if g.enabled { "●" } else { "○" };
                                            let color = if g.enabled {
                                                theme::success(flavor)
                                            } else {
                                                theme::subtext(flavor)
                                            };
                                            let label = egui::RichText::new(format!(
                                                "{} {}  {} @{} ×{}",
                                                dot,
                                                g.name,
                                                match g.fc {
                                                    ReadFunction::ReadCoils => "FC01",
                                                    ReadFunction::ReadDiscreteInputs => "FC02",
                                                    ReadFunction::ReadHoldingRegisters => "FC03",
                                                    ReadFunction::ReadInputRegisters => "FC04",
                                                },
                                                g.addr,
                                                g.qty,
                                            ))
                                            .color(color);
                                            if ui
                                                .add(egui::SelectableLabel::new(selected, label))
                                                .clicked()
                                            {
                                                self.selected_group.insert(id.clone(), i);
                                            }
                                        }
                                    }
                                });
                            },
                        );
                        ui.separator();

                        // Right: selected group detail
                        ui.vertical(|ui| {
                            let Some(list) = self.polling.get_mut(&id) else { return };
                            if list.is_empty() {
                                uikit::caption(ui, flavor, "点击左上 + 新建扫描组");
                                return;
                            }
                            let idx = sel.min(list.len() - 1);
                            let pu = &mut list[idx];
                            egui::Grid::new("poll_form")
                                .num_columns(2)
                                .spacing([10.0, 6.0])
                                .show(ui, |ui| {
                                    ui.label("名称");
                                    ui.text_edit_singleline(&mut pu.name);
                                    ui.end_row();
                                    ui.label("功能码");
                                    egui::ComboBox::from_id_salt("poll_fc")
                                        .selected_text(read_fc_label(pu.fc))
                                        .show_ui(ui, |ui| {
                                            for f in [
                                                ReadFunction::ReadCoils,
                                                ReadFunction::ReadDiscreteInputs,
                                                ReadFunction::ReadHoldingRegisters,
                                                ReadFunction::ReadInputRegisters,
                                            ] {
                                                ui.selectable_value(&mut pu.fc, f, read_fc_label(f));
                                            }
                                        });
                                    ui.end_row();
                                    ui.label("起址");
                                    let mut a = pu.addr as u32;
                                    ui.add(egui::DragValue::new(&mut a).range(0..=65535));
                                    pu.addr = a as u16;
                                    ui.end_row();
                                    ui.label("数量");
                                    let mut q = pu.qty as u32;
                                    ui.add(egui::DragValue::new(&mut q).range(1..=2000));
                                    pu.qty = q as u16;
                                    ui.end_row();
                                    ui.label("间隔 (ms)");
                                    ui.add(egui::DragValue::new(&mut pu.interval_ms).range(50..=60_000));
                                    ui.end_row();
                                });
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                let running = pu.enabled;
                                let last_update = pu.last_update;
                                let last_err = pu.last_error.clone();
                                if running {
                                    if uikit::danger_button(ui, flavor, "停止").clicked() {
                                        do_stop_poll_id = Some((id.clone(), idx));
                                    }
                                    uikit::status_pill(ui, "运行中", theme::success(flavor));
                                } else {
                                    ui.add_enabled_ui(is_connected, |ui| {
                                        if uikit::primary_button(ui, flavor, "开始").clicked() {
                                            do_start_poll_id = Some((id.clone(), idx));
                                        }
                                    });
                                }
                                if let Some(t) = last_update {
                                    uikit::caption(
                                        ui,
                                        flavor,
                                        format!("· {} ms 前更新", t.elapsed().as_millis()),
                                    );
                                }
                                if let Some(err) = last_err {
                                    ui.colored_label(theme::danger(flavor), err);
                                }
                            });
                        });
                    });
                }
            }
            }); // end tab card
            ui.add_space(6.0);

            // Result section — shows selected group's latest, or one-shot read result.
            let sel = *self.selected_group.get(&id).unwrap_or(&0);
            let (poll_latest, poll_addr) = self
                .polling
                .get(&id)
                .and_then(|v| v.get(sel))
                .map(|g| (g.latest.clone(), g.addr))
                .unwrap_or((None, 0));
            let show_result = poll_latest.clone().or_else(|| self.read_result.clone());
            if let Some(result) = &show_result {
                uikit::card(ui, flavor, |ui| {
                let title = if poll_latest.is_some() { "轮询结果" } else { "读取结果" };
                let base = if poll_latest.is_some() { poll_addr } else { self.read_addr };
                ui.label(egui::RichText::new(title).strong().size(12.5));
                ui.add_space(4.0);
                match result {
                    ReadResult::HoldingRegisters(vs) | ReadResult::InputRegisters(vs) => {
                        render_u16_table(ui, base, vs);
                    }
                    ReadResult::Coils(bs) | ReadResult::DiscreteInputs(bs) => {
                        render_bool_table(ui, base, bs);
                    }
                }
                }); // end result card
            }
        });

        match action {
            Some(Action::Create) => self.create_connection(ctx.clone()),
            Some(Action::Select(id)) => self.selected = Some(id),
            Some(Action::Connect(id)) => self.connect(&id, ctx.clone()),
            Some(Action::Disconnect(id)) => self.disconnect(&id, ctx.clone()),
            Some(Action::Remove(id)) => self.remove_connection(&id, ctx.clone()),
            None => {}
        }
        if let Some(id) = do_read_id {
            self.do_read(id, ctx.clone());
        }
        if let Some(id) = do_write_id {
            self.do_write(id, ctx.clone());
        }
        if let Some((id, idx)) = do_start_poll_id {
            self.start_poll(id, idx, ctx.clone());
        }
        if let Some((id, idx)) = do_stop_poll_id {
            self.stop_poll(id, idx, ctx.clone());
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
        if self.selected.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}

impl MasterApp {
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

    fn clear_logs_for_selection(&self) {
        let Some(id) = self.selected.clone() else { return };
        let connections = self.connections.clone();
        self.rt.spawn(async move {
            let entries = connections.read().await;
            if let Some(entry) = entries.iter().find(|e| e.id == id) {
                entry.log_collector.clear().await;
            }
        });
    }

    fn export_logs_for_selection(&mut self, ctx: egui::Context) {
        let Some(id) = self.selected.clone() else { return };
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
            let Some(path) = rfd::AsyncFileDialog::new()
                .set_file_name(&format!("master_log_{}_{}.csv", id, ts))
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
}

fn read_fc_label(f: ReadFunction) -> &'static str {
    match f {
        ReadFunction::ReadCoils => "FC01 读线圈",
        ReadFunction::ReadDiscreteInputs => "FC02 读离散输入",
        ReadFunction::ReadHoldingRegisters => "FC03 读保持寄存器",
        ReadFunction::ReadInputRegisters => "FC04 读输入寄存器",
    }
}

fn render_u16_table(ui: &mut egui::Ui, start: u16, values: &[u16]) {
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .column(Column::exact(80.0))
        .column(Column::exact(100.0))
        .column(Column::exact(90.0))
        .column(Column::remainder())
        .header(20.0, |mut h| {
            h.col(|ui| { ui.strong("地址"); });
            h.col(|ui| { ui.strong("Unsigned"); });
            h.col(|ui| { ui.strong("Signed"); });
            h.col(|ui| { ui.strong("Hex"); });
        })
        .body(|body| {
            body.rows(18.0, values.len(), |mut row| {
                let i = row.index();
                let addr = start.wrapping_add(i as u16);
                let v = values[i];
                row.col(|ui| { ui.monospace(format!("{}", addr)); });
                row.col(|ui| { ui.monospace(v.to_string()); });
                row.col(|ui| { ui.monospace((v as i16).to_string()); });
                row.col(|ui| { ui.monospace(format!("0x{:04X}", v)); });
            });
        });
}

fn render_bool_table(ui: &mut egui::Ui, start: u16, values: &[bool]) {
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .column(Column::exact(80.0))
        .column(Column::remainder())
        .header(20.0, |mut h| {
            h.col(|ui| { ui.strong("地址"); });
            h.col(|ui| { ui.strong("布尔"); });
        })
        .body(|body| {
            body.rows(18.0, values.len(), |mut row| {
                let i = row.index();
                let addr = start.wrapping_add(i as u16);
                row.col(|ui| { ui.monospace(format!("{}", addr)); });
                row.col(|ui| { ui.monospace(if values[i] { "true" } else { "false" }); });
            });
        });
}
