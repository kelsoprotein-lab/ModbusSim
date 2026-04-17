use std::sync::Arc;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use modbussim_core::log_collector::LogCollector;
use modbussim_core::register::RegisterType;
use modbussim_core::slave::{ConnectionState, SlaveConnection, SlaveDevice};
use modbussim_core::transport::Transport;
use modbussim_ui_shared::format::{format_u16, U16Format};
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

/// Per-frame cache of the currently displayed register group.
pub struct RegViewCache {
    pub conn_id: String,
    pub slave_id: u8,
    pub reg_type: RegisterType,
    /// Values indexed by address. For FC01/FC02 only the first element per
    /// address is populated (as u16 = 0/1). Sized lazily to max_addr+1.
    pub values: Vec<Option<u16>>,
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
    next_conn_seq: u64,

    // Register table view
    reg_view: Option<RegViewCache>,
    reg_row_limit: usize,
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
            next_conn_seq: 1,
            reg_view: None,
            reg_row_limit: 1000,
        }
    }

    /// Try to refresh the register view cache from the authoritative state.
    /// Uses sync `try_read` on the async RwLocks — skip this frame if contended.
    fn refresh_reg_view(&mut self) {
        let Selection::RegisterGroup { conn_id, slave_id, reg_type } = self.selection.clone() else {
            self.reg_view = None;
            return;
        };
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

        let limit = self.reg_row_limit;
        let map = &dev.register_map;
        let mut values: Vec<Option<u16>> = vec![None; limit];
        match reg_type {
            RegisterType::HoldingRegister => {
                for addr in 0..limit as u16 {
                    if let Some(v) = map.holding_registers.get(&addr) {
                        values[addr as usize] = Some(*v);
                    }
                }
            }
            RegisterType::InputRegister => {
                for addr in 0..limit as u16 {
                    if let Some(v) = map.input_registers.get(&addr) {
                        values[addr as usize] = Some(*v);
                    }
                }
            }
            RegisterType::Coil => {
                for addr in 0..limit as u16 {
                    if let Some(b) = map.coils.get(&addr) {
                        values[addr as usize] = Some(if *b { 1 } else { 0 });
                    }
                }
            }
            RegisterType::DiscreteInput => {
                for addr in 0..limit as u16 {
                    if let Some(b) = map.discrete_inputs.get(&addr) {
                        values[addr as usize] = Some(if *b { 1 } else { 0 });
                    }
                }
            }
        }

        self.reg_view = Some(RegViewCache {
            conn_id,
            slave_id,
            reg_type,
            values,
        });
    }

    fn allocate_connection(&mut self) -> (String, String) {
        let id = format!("slave_{}", self.next_conn_seq);
        self.next_conn_seq += 1;
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
            let id = format!("slave_{}", self.next_conn_seq);
            self.next_conn_seq += 1;
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
            TreeAction::SelectConn(id) => self.selection = Selection::Connection(id),
            TreeAction::SelectDevice { conn_id, slave_id } => {
                self.selection = Selection::Device { conn_id, slave_id }
            }
            TreeAction::SelectGroup { conn_id, slave_id, reg_type } => {
                self.selection = Selection::RegisterGroup { conn_id, slave_id, reg_type }
            }
            TreeAction::StartConn(id) => self.start_connection(&id, ctx.clone()),
            TreeAction::StopConn(id) => self.stop_connection(&id, ctx.clone()),
            TreeAction::RemoveConn(id) => self.remove_connection(&id, ctx.clone()),
            TreeAction::Create => self.create_tcp_connection(ctx.clone()),
        }
    }

    fn render_main(&self, ui: &mut egui::Ui) {
        match &self.selection {
            Selection::None => {
                ui.heading("ModbusSlave — egui edition");
                ui.label("从左侧创建或选中一个连接/设备/寄存器组。");
            }
            Selection::Connection(id) => match self.conn_snapshot.iter().find(|s| &s.id == id) {
                Some(s) => {
                    ui.heading(&s.label);
                    ui.label(format!(
                        "状态: {} · 设备数: {}",
                        match s.state {
                            ConnectionState::Running => "运行中",
                            ConnectionState::Stopped => "已停止",
                        },
                        s.devices.len()
                    ));
                }
                None => {
                    ui.label("连接已不存在。");
                }
            },
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
                ui.heading(format!("{} · 连接 {} · 从站 {}", group_label, conn_id, slave_id));
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

                ui.label(format!(
                    "显示前 {} 个地址 · 只读视图（S2 将加内联编辑）",
                    view.values.len()
                ));
                ui.separator();

                let row_h = 20.0;
                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::exact(80.0))
                    .column(Column::exact(100.0))
                    .column(Column::exact(100.0))
                    .column(Column::exact(140.0))
                    .column(Column::remainder())
                    .header(22.0, |mut header| {
                        header.col(|ui| { ui.strong("地址"); });
                        header.col(|ui| {
                            ui.strong(if is_bool { "布尔" } else { "Unsigned" });
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
                        body.rows(row_h, view.values.len(), |mut row| {
                            let addr = row.index();
                            row.col(|ui| {
                                ui.monospace(format!("{}", addr));
                            });
                            let val = view.values.get(addr).copied().flatten();
                            row.col(|ui| match val {
                                Some(v) if is_bool => {
                                    ui.monospace(if v != 0 { "true" } else { "false" });
                                }
                                Some(v) => {
                                    ui.monospace(format_u16(v, U16Format::Unsigned));
                                }
                                None => {
                                    ui.monospace("—");
                                }
                            });
                            row.col(|ui| match val {
                                Some(v) if !is_bool => {
                                    ui.monospace(format_u16(v, U16Format::Hex));
                                }
                                _ => { ui.monospace(""); }
                            });
                            row.col(|ui| match val {
                                Some(v) if !is_bool => {
                                    ui.monospace(format_u16(v, U16Format::Binary));
                                }
                                _ => { ui.monospace(""); }
                            });
                            row.col(|_| {});
                        });
                    });
            }
        }
    }
}

impl eframe::App for SlaveApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        self.refresh_reg_view();

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
        egui::TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .show(ctx, |ui| match &self.last_error {
                Some(err) => {
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::RED, err);
                        if ui.small_button("清除").clicked() {
                            clear_error = true;
                        }
                    });
                }
                None => {
                    ui.label("就绪");
                }
            });
        if clear_error {
            self.last_error = None;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_main(ui);
        });

        if let Some(a) = tree_action {
            self.apply_tree_action(a, ctx);
        }

        if !self.events_rx.is_empty() {
            ctx.request_repaint();
        }
    }
}
