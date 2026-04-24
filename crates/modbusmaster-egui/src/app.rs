use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use eframe::egui;
use modbussim_core::log_collector::LogCollector;
use modbussim_core::log_entry::LogEntry;
use modbussim_core::master::{
    MasterConfig, MasterConnection, MasterState, PollEvent, ReadFunction, ReadResult, ScanGroup,
};
use modbussim_core::transport::Transport;
use modbussim_ui_shared::i18n::{tr, tr1, Lang};
use modbussim_ui_shared::log_panel::{self, LogPanelAction, LogPanelState};
use modbussim_ui_shared::project::{
    deserialize_master, serialize_master, MasterConnectionSave, MasterProject, PollSave, TcpSpec,
};
use modbussim_ui_shared::theme::{self, Flavor};
use modbussim_ui_shared::ui as uikit;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use crate::events::UiEvent;
use crate::result_table::{read_fc_label, render_bool_table, render_u16_table};
use crate::scan_group::ScanGroupUi;

pub struct MasterConnectionEntry {
    pub id: String,
    #[allow(dead_code)]
    pub label: String,
    pub connection: Arc<RwLock<MasterConnection>>,
    pub log_collector: Arc<LogCollector>,
}

pub type SharedConnections = Arc<RwLock<Vec<MasterConnectionEntry>>>;

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
    fn label(self, lang: Lang) -> &'static str {
        let key = match self {
            MasterTab::Read => "tab.read",
            MasterTab::Write => "tab.write",
            MasterTab::Poll => "tab.poll",
        };
        tr(lang, key)
    }
}

/// 侧栏按钮触发的延迟动作;update() 末尾集中 apply。
enum SidebarAction {
    Create,
    Select(String),
    Connect(String),
    Disconnect(String),
    Remove(String),
}

#[derive(Default)]
struct MenuFlags {
    save: bool,
    load: bool,
}

/// 中央面板延迟动作(update 末尾统一 apply,避开中途 &mut self 借用冲突)。
#[derive(Default)]
struct CentralActions {
    read: Option<String>,
    write: Option<String>,
    start_poll: Option<(String, usize)>,
    stop_poll: Option<(String, usize)>,
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
    pub lang: Lang,
    active_tab: MasterTab,
}

impl MasterApp {
    pub fn new(rt: Arc<Runtime>, flavor: Flavor, lang: Lang) -> Self {
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
            lang,
            active_tab: MasterTab::Read,
        }
    }

    fn allocate_id(&self) -> String {
        let n = self.next_seq.fetch_add(1, Ordering::Relaxed);
        format!("master_{}", n)
    }

    fn create_connection(&self, ctx: egui::Context) {
        let host = self.new_host.trim().to_string();
        let lang = self.lang;
        let port: u16 = match self.new_port.trim().parse() {
            Ok(p) => p,
            Err(_) => {
                let _ = self.events_tx.send(UiEvent::Error(tr1(
                    lang,
                    "err.invalid_port_fmt",
                    &self.new_port,
                )));
                return;
            }
        };
        let slave_id = self.new_slave_id;
        let timeout_ms = self.new_timeout;
        let id = self.allocate_id();
        let mut label = tr(lang, "master.tcp_label_fmt")
            .replacen("{}", &host, 1)
            .replacen("{}", &port.to_string(), 1)
            .replacen("{}", &slave_id.to_string(), 1);
        if label.is_empty() {
            label = format!("TCP {}:{} · {}", host, port, slave_id);
        }

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
        let lang = self.lang;
        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.connection.clone());
            let Some(conn_arc) = conn_arc else {
                let _ = tx.send(UiEvent::Error(tr1(lang, "err.conn_not_found_fmt", &id)));
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
                    let _ = tx.send(UiEvent::Error(tr1(lang, "err.connect_failed_fmt", e)));
                }
            }
        });
        ctx.request_repaint();
    }

    fn disconnect(&self, id: &str, ctx: egui::Context) {
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        let id = id.to_string();
        let lang = self.lang;
        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.connection.clone());
            let Some(conn_arc) = conn_arc else {
                let _ = tx.send(UiEvent::Error(tr1(lang, "err.conn_not_found_fmt", &id)));
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
                    let _ = tx.send(UiEvent::Error(tr1(lang, "err.disconnect_failed_fmt", e)));
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
        let lang = self.lang;
        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.connection.clone());
            let Some(conn_arc) = conn_arc else {
                let _ = tx.send(UiEvent::Error(tr1(lang, "err.conn_not_found_fmt", &id)));
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
                        let _ = tx.send(UiEvent::Error(tr1(lang, "err.poll_start_failed_fmt", e)));
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
        let lang = self.lang;
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
                    let _ = tx.send(UiEvent::Error(tr1(lang, "err.poll_stop_failed_fmt", e)));
                }
            }
        });
        ctx.request_repaint();
    }

    fn add_scan_group(&mut self, conn_id: String) {
        let gid = format!("group_{}", self.next_group_seq);
        self.next_group_seq += 1;
        let mut g = ScanGroupUi::new_with_id(gid);
        let idx = self.polling.get(&conn_id).map(|v| v.len() + 1).unwrap_or(1);
        g.name = tr1(self.lang, "group.default_name_fmt", idx);
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
                let new_sel = if list.is_empty() {
                    0
                } else {
                    group_idx.saturating_sub(1).min(list.len() - 1)
                };
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
        let lang = self.lang;
        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.connection.clone());
            let Some(conn_arc) = conn_arc else {
                let _ = tx.send(UiEvent::Error(tr1(lang, "err.conn_not_found_fmt", &id)));
                return;
            };
            let conn = conn_arc.read().await;
            match conn.read(fc, addr, qty).await {
                Ok(result) => {
                    let _ = tx.send(UiEvent::ReadDone { id, result });
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(tr1(lang, "err.read_failed_fmt", e)));
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
        let lang = self.lang;
        self.rt.spawn(async move {
            let conn_arc = connections
                .read()
                .await
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.connection.clone());
            let Some(conn_arc) = conn_arc else {
                let _ = tx.send(UiEvent::Error(tr1(lang, "err.conn_not_found_fmt", &id)));
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
                    let _ = tx.send(UiEvent::Info(tr1(lang, "err.write_ok_fmt", &id)));
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(tr1(lang, "err.write_failed_fmt", e)));
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
                tcp: TcpSpec {
                    host,
                    port,
                    tls: None,
                },
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
        let lang = self.lang;
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
                        let _ = tx.send(UiEvent::Info(tr1(
                            lang,
                            "err.saved_fmt",
                            path.path().display(),
                        )));
                    }
                    Err(e) => {
                        let _ = tx.send(UiEvent::Error(tr1(lang, "err.save_failed_fmt", e)));
                    }
                },
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(tr1(lang, "err.serialize_failed_fmt", e)));
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
        let lang = self.lang;
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
                    let _ = tx.send(UiEvent::Error(tr1(lang, "err.read_file_failed_fmt", e)));
                    return;
                }
            };
            let project = match deserialize_master(&text) {
                Ok(p) => p,
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(tr1(lang, "err.parse_failed_fmt", e)));
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
                    Transport::Tcp {
                        host: tcp.host.clone(),
                        port: tcp.port,
                    },
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
            let _ = tx.send(UiEvent::Info(tr1(
                lang,
                "err.loaded_fmt",
                file.path().display(),
            )));
            ctx2.request_repaint();
        });
    }

    fn drain_events(&mut self) {
        while let Ok(ev) = self.events_rx.try_recv() {
            match ev {
                UiEvent::ConnectionCreated {
                    id,
                    label,
                    slave_id,
                } => {
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
                UiEvent::PollUpdate {
                    id,
                    group_id,
                    result,
                } => {
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
                UiEvent::PollConfigLoaded {
                    id,
                    group_id,
                    fc,
                    addr,
                    qty,
                    interval_ms,
                } => {
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
        self.log_cache_conn_id = Some(id);
        self.log_last_refresh = Some(Instant::now());
    }
}

impl eframe::App for MasterApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "flavor_v3", &self.flavor);
        eframe::set_value(storage, "lang_v1", &self.lang);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        self.refresh_log_cache();

        let lang = self.lang;
        let flavor = self.flavor;

        let menu = self.render_menu_bar(ctx, lang);
        let sidebar_action = self.render_sidebar(ctx, lang);
        self.render_status_bar(ctx, lang);
        self.render_log_panel(ctx);

        let mut central = CentralActions::default();
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(id) = self.selected.clone() else {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    ui.label(
                        egui::RichText::new(tr(lang, "master.app_title"))
                            .size(18.0)
                            .strong(),
                    );
                    uikit::caption(ui, flavor, tr(lang, "master.empty_hint"));
                });
                return;
            };
            let Some(s) = self.snap.iter().find(|s| s.id == id).cloned() else {
                ui.label(tr(lang, "master.conn_gone"));
                return;
            };
            // Header region: address + status pill. No card stroke, no accent stripe.
            uikit::region(
                ui,
                flavor,
                theme::Layer::L1,
                egui::Margin::symmetric(14.0 as i8, 10.0 as i8),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&s.label).strong().size(13.5));
                        let (txt, color) = match s.state {
                            MasterState::Connected => {
                                (tr(lang, "conn.state.connected"), theme::success(flavor))
                            }
                            MasterState::Disconnected => {
                                (tr(lang, "conn.state.disconnected"), theme::subtext(flavor))
                            }
                            MasterState::Reconnecting => {
                                (tr(lang, "conn.state.reconnecting"), theme::accent(flavor))
                            }
                            MasterState::Error => {
                                (tr(lang, "conn.state.error"), theme::danger(flavor))
                            }
                        };
                        uikit::status_pill(ui, txt, color);
                    });
                },
            );
            ui.add_space(4.0);

            uikit::region(
                ui,
                flavor,
                theme::Layer::L1,
                egui::Margin::symmetric(14.0 as i8, 10.0 as i8),
                |ui| {
                    // Tab bar: Read / Write / Poll
                    ui.horizontal(|ui| {
                        for tab in [MasterTab::Read, MasterTab::Write, MasterTab::Poll] {
                            let selected = self.active_tab == tab;
                            let text = if selected {
                                egui::RichText::new(tab.label(lang))
                                    .strong()
                                    .color(theme::accent(flavor))
                            } else {
                                egui::RichText::new(tab.label(lang)).color(theme::subtext(flavor))
                            };
                            if ui.add(egui::SelectableLabel::new(selected, text)).clicked() {
                                self.active_tab = tab;
                            }
                        }
                    });
                    ui.separator();

                    // Tab content — each tab 抽成独立方法,见下方 impl 块。
                    let connected = s.state == MasterState::Connected;
                    match self.active_tab {
                        MasterTab::Read => {
                            self.render_read_tab(ui, lang, flavor, connected, &id, &mut central);
                        }
                        MasterTab::Write => {
                            self.render_write_tab(ui, lang, flavor, connected, &id, &mut central);
                        }
                        MasterTab::Poll => {
                            self.render_poll_tab(
                                ui,
                                ctx,
                                lang,
                                flavor,
                                connected,
                                &id,
                                &mut central,
                            );
                        }
                    }
                },
            ); // end tab card
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
                uikit::region(
                    ui,
                    flavor,
                    theme::Layer::L2,
                    egui::Margin::symmetric(12.0 as i8, 10.0 as i8),
                    |ui| {
                        let title = if poll_latest.is_some() {
                            tr(lang, "result.poll_title")
                        } else {
                            tr(lang, "result.read_title")
                        };
                        let base = if poll_latest.is_some() {
                            poll_addr
                        } else {
                            self.read_addr
                        };
                        ui.label(egui::RichText::new(title).strong().size(12.5));
                        ui.add_space(4.0);
                        match result {
                            ReadResult::HoldingRegisters(vs) | ReadResult::InputRegisters(vs) => {
                                render_u16_table(ui, base, vs, lang);
                            }
                            ReadResult::Coils(bs) | ReadResult::DiscreteInputs(bs) => {
                                render_bool_table(ui, base, bs, lang);
                            }
                        }
                    },
                ); // end result card
            }
        });

        if let Some(a) = sidebar_action {
            match a {
                SidebarAction::Create => self.create_connection(ctx.clone()),
                SidebarAction::Select(id) => self.selected = Some(id),
                SidebarAction::Connect(id) => self.connect(&id, ctx.clone()),
                SidebarAction::Disconnect(id) => self.disconnect(&id, ctx.clone()),
                SidebarAction::Remove(id) => self.remove_connection(&id, ctx.clone()),
            }
        }
        if let Some(id) = central.read {
            self.do_read(id, ctx.clone());
        }
        if let Some(id) = central.write {
            self.do_write(id, ctx.clone());
        }
        if let Some((id, idx)) = central.start_poll {
            self.start_poll(id, idx, ctx.clone());
        }
        if let Some((id, idx)) = central.stop_poll {
            self.stop_poll(id, idx, ctx.clone());
        }
        if menu.save {
            self.save_project(ctx.clone());
        }
        if menu.load {
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
    fn render_menu_bar(&mut self, ctx: &egui::Context, lang: Lang) -> MenuFlags {
        let mut flags = MenuFlags::default();
        egui::TopBottomPanel::top("master_menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button(tr(lang, "menu.file"), |ui| {
                    if ui.button(tr(lang, "menu.file.save")).clicked() {
                        flags.save = true;
                        ui.close_menu();
                    }
                    if ui.button(tr(lang, "menu.file.load")).clicked() {
                        flags.load = true;
                        ui.close_menu();
                    }
                });
                ui.menu_button(tr(lang, "menu.view"), |ui| {
                    ui.checkbox(
                        &mut self.log_state.open,
                        tr(lang, "menu.view.show_log_panel"),
                    );
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
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    let zoom = ctx.zoom_factor();
                    if ui
                        .button(format!(
                            "{} ({:.0}%)",
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
                    ui.radio_value(&mut self.lang, Lang::Zh, Lang::Zh.native_label());
                    ui.radio_value(&mut self.lang, Lang::En, Lang::En.native_label());
                });
                ui.menu_button(tr(lang, "menu.help"), |ui| {
                    ui.label(tr(lang, "menu.help.about_master"));
                    ui.hyperlink_to("GitHub", "https://github.com/kelsoprotein-lab/ModbusSim");
                });
            });
        });
        flags
    }

    fn render_sidebar(&mut self, ctx: &egui::Context, lang: Lang) -> Option<SidebarAction> {
        let mut action: Option<SidebarAction> = None;
        egui::SidePanel::left("master_connections")
            .resizable(true)
            .default_width(240.0)
            .min_width(200.0)
            .show(ctx, |ui| {
                ui.heading(tr(lang, "sidebar.connections"));
                ui.separator();

                ui.collapsing(tr(lang, "master.new_tcp"), |ui| {
                    egui::Grid::new("master_new_form")
                        .num_columns(2)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(tr(lang, "master.host"));
                            ui.text_edit_singleline(&mut self.new_host);
                            ui.end_row();
                            ui.label(tr(lang, "master.port"));
                            ui.text_edit_singleline(&mut self.new_port);
                            ui.end_row();
                            ui.label(tr(lang, "master.slave_id"));
                            let mut sid = self.new_slave_id as u32;
                            ui.add(egui::DragValue::new(&mut sid).range(1..=247));
                            self.new_slave_id = sid as u8;
                            ui.end_row();
                            ui.label(tr(lang, "master.timeout_ms"));
                            ui.add(egui::DragValue::new(&mut self.new_timeout).range(100..=60_000));
                            ui.end_row();
                        });
                    if ui.button(tr(lang, "sidebar.create")).clicked() {
                        action = Some(SidebarAction::Create);
                    }
                });
                ui.separator();

                for s in &self.snap {
                    let is_sel = self.selected.as_deref() == Some(&s.id);
                    let state_tag = match s.state {
                        MasterState::Connected => tr(lang, "conn.state.connected"),
                        MasterState::Disconnected => tr(lang, "conn.state.disconnected"),
                        MasterState::Reconnecting => tr(lang, "conn.state.reconnecting"),
                        MasterState::Error => tr(lang, "conn.state.error"),
                    };
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(is_sel, format!("{} [{}]", s.label, state_tag))
                            .clicked()
                        {
                            action = Some(SidebarAction::Select(s.id.clone()));
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        match s.state {
                            MasterState::Connected | MasterState::Reconnecting => {
                                if ui.small_button(tr(lang, "conn.disconnect")).clicked() {
                                    action = Some(SidebarAction::Disconnect(s.id.clone()));
                                }
                            }
                            _ => {
                                if ui.small_button(tr(lang, "conn.connect")).clicked() {
                                    action = Some(SidebarAction::Connect(s.id.clone()));
                                }
                            }
                        }
                        if ui.small_button(tr(lang, "conn.delete")).clicked() {
                            action = Some(SidebarAction::Remove(s.id.clone()));
                        }
                    });
                    ui.separator();
                }
            });
        action
    }

    fn render_status_bar(&mut self, ctx: &egui::Context, lang: Lang) {
        let mut clear_err = false;
        let mut clear_status = false;
        egui::TopBottomPanel::bottom("master_status")
            .resizable(false)
            .show(ctx, |ui| {
                if let Some(err) = &self.last_error {
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::RED, err);
                        if ui.small_button(tr(lang, "sidebar.clear")).clicked() {
                            clear_err = true;
                        }
                    });
                } else if let Some(msg) = &self.status_msg {
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::from_rgb(60, 140, 60), msg);
                        if ui.small_button(tr(lang, "sidebar.clear")).clicked() {
                            clear_status = true;
                        }
                    });
                } else {
                    ui.label(tr(lang, "conn.ready"));
                }
            });
        if clear_err {
            self.last_error = None;
        }
        if clear_status {
            self.status_msg = None;
        }
    }

    fn render_read_tab(
        &mut self,
        ui: &mut egui::Ui,
        lang: Lang,
        flavor: Flavor,
        connected: bool,
        id: &str,
        central: &mut CentralActions,
    ) {
        egui::Grid::new("read_form")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label(tr(lang, "read.fc"));
                egui::ComboBox::from_id_salt("read_fc")
                    .selected_text(read_fc_label(self.read_fc, lang))
                    .show_ui(ui, |ui| {
                        for f in [
                            ReadFunction::ReadCoils,
                            ReadFunction::ReadDiscreteInputs,
                            ReadFunction::ReadHoldingRegisters,
                            ReadFunction::ReadInputRegisters,
                        ] {
                            ui.selectable_value(&mut self.read_fc, f, read_fc_label(f, lang));
                        }
                    });
                ui.end_row();
                ui.label(tr(lang, "read.start_addr"));
                let mut a = self.read_addr as u32;
                ui.add(egui::DragValue::new(&mut a).range(0..=65535));
                self.read_addr = a as u16;
                ui.end_row();
                ui.label(tr(lang, "read.count"));
                let mut q = self.read_qty as u32;
                ui.add(egui::DragValue::new(&mut q).range(1..=2000));
                self.read_qty = q as u16;
                ui.end_row();
            });
        ui.add_space(8.0);
        ui.add_enabled_ui(connected, |ui| {
            if uikit::primary_button(ui, flavor, tr(lang, "read.action")).clicked() {
                central.read = Some(id.to_string());
            }
        });
    }

    fn render_write_tab(
        &mut self,
        ui: &mut egui::Ui,
        lang: Lang,
        flavor: Flavor,
        connected: bool,
        id: &str,
        central: &mut CentralActions,
    ) {
        egui::Grid::new("write_form")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label(tr(lang, "write.type"));
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.write_is_coil, false, tr(lang, "write.fc06_reg"));
                    ui.radio_value(&mut self.write_is_coil, true, tr(lang, "write.fc05_coil"));
                });
                ui.end_row();
                ui.label(tr(lang, "write.addr"));
                let mut a = self.write_addr as u32;
                ui.add(egui::DragValue::new(&mut a).range(0..=65535));
                self.write_addr = a as u16;
                ui.end_row();
                ui.label(tr(lang, "write.value"));
                if self.write_is_coil {
                    let mut b = self.write_value != 0;
                    ui.checkbox(&mut b, tr(lang, "write.bool_caption"));
                    self.write_value = if b { 1 } else { 0 };
                } else {
                    ui.add(egui::DragValue::new(&mut self.write_value).range(0..=65535));
                }
                ui.end_row();
            });
        ui.add_space(8.0);
        ui.add_enabled_ui(connected, |ui| {
            if uikit::primary_button(ui, flavor, tr(lang, "write.action")).clicked() {
                central.write = Some(id.to_string());
            }
        });
    }

    fn render_poll_tab(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        lang: Lang,
        flavor: Flavor,
        connected: bool,
        id: &str,
        central: &mut CentralActions,
    ) {
        self.polling.entry(id.to_string()).or_default();
        let sel = *self.selected_group.get(id).unwrap_or(&0);

        // Toolbar
        ui.horizontal(|ui| {
            if uikit::primary_button(ui, flavor, tr(lang, "poll.new_group")).clicked() {
                self.add_scan_group(id.to_string());
            }
            let len = self.polling.get(id).map(|v| v.len()).unwrap_or(0);
            let has_sel = len > 0 && sel < len;
            if has_sel && uikit::danger_button(ui, flavor, tr(lang, "poll.del_group")).clicked() {
                self.remove_scan_group(id.to_string(), sel, ctx.clone());
            }
            uikit::caption(ui, flavor, tr1(lang, "poll.group_count_fmt", len));
        });
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            // Left: group list
            ui.allocate_ui_with_layout(
                egui::vec2(200.0, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        if let Some(list) = self.polling.get(id) {
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
                                    self.selected_group.insert(id.to_string(), i);
                                }
                            }
                        }
                    });
                },
            );
            ui.separator();

            // Right: selected group detail
            ui.vertical(|ui| {
                let Some(list) = self.polling.get_mut(id) else {
                    return;
                };
                if list.is_empty() {
                    uikit::caption(ui, flavor, tr(lang, "poll.empty_hint"));
                    return;
                }
                let idx = sel.min(list.len() - 1);
                let pu = &mut list[idx];
                egui::Grid::new("poll_form")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(tr(lang, "poll.name"));
                        ui.text_edit_singleline(&mut pu.name);
                        ui.end_row();
                        ui.label(tr(lang, "read.fc"));
                        egui::ComboBox::from_id_salt("poll_fc")
                            .selected_text(read_fc_label(pu.fc, lang))
                            .show_ui(ui, |ui| {
                                for f in [
                                    ReadFunction::ReadCoils,
                                    ReadFunction::ReadDiscreteInputs,
                                    ReadFunction::ReadHoldingRegisters,
                                    ReadFunction::ReadInputRegisters,
                                ] {
                                    ui.selectable_value(&mut pu.fc, f, read_fc_label(f, lang));
                                }
                            });
                        ui.end_row();
                        ui.label(tr(lang, "poll.start_addr"));
                        let mut a = pu.addr as u32;
                        ui.add(egui::DragValue::new(&mut a).range(0..=65535));
                        pu.addr = a as u16;
                        ui.end_row();
                        ui.label(tr(lang, "read.count"));
                        let mut q = pu.qty as u32;
                        ui.add(egui::DragValue::new(&mut q).range(1..=2000));
                        pu.qty = q as u16;
                        ui.end_row();
                        ui.label(tr(lang, "poll.interval_ms"));
                        ui.add(egui::DragValue::new(&mut pu.interval_ms).range(50..=60_000));
                        ui.end_row();
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let running = pu.enabled;
                    let last_update = pu.last_update;
                    let last_err = pu.last_error.clone();
                    if running {
                        if uikit::danger_button(ui, flavor, tr(lang, "poll.stop")).clicked() {
                            central.stop_poll = Some((id.to_string(), idx));
                        }
                        uikit::status_pill(ui, tr(lang, "poll.running"), theme::success(flavor));
                    } else {
                        ui.add_enabled_ui(connected, |ui| {
                            if uikit::primary_button(ui, flavor, tr(lang, "poll.start")).clicked() {
                                central.start_poll = Some((id.to_string(), idx));
                            }
                        });
                    }
                    if let Some(t) = last_update {
                        uikit::caption(
                            ui,
                            flavor,
                            tr1(lang, "poll.updated_ms_ago_fmt", t.elapsed().as_millis()),
                        );
                    }
                    if let Some(err) = last_err {
                        ui.colored_label(theme::danger(flavor), err);
                    }
                });
            });
        });
    }

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

    fn clear_logs_for_selection(&self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let connections = self.connections.clone();
        self.rt.spawn(async move {
            let entries = connections.read().await;
            if let Some(entry) = entries.iter().find(|e| e.id == id) {
                entry.log_collector.clear().await;
            }
        });
    }

    fn export_logs_for_selection(&mut self, ctx: egui::Context) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let connections = self.connections.clone();
        let tx = self.events_tx.clone();
        let lang = self.lang;
        self.rt.spawn(async move {
            let entries = connections.read().await;
            let Some(entry) = entries.iter().find(|e| e.id == id) else {
                let _ = tx.send(UiEvent::Error(tr1(lang, "err.conn_not_found_fmt", &id)));
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
                    let _ = tx.send(UiEvent::Info(tr1(
                        lang,
                        "err.log_exported_fmt",
                        path.path().display(),
                    )));
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::Error(tr1(lang, "err.export_failed_fmt", e)));
                }
            }
        });
        ctx.request_repaint();
    }
}
