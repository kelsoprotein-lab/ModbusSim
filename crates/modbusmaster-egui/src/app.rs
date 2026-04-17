use std::sync::Arc;
use std::time::Instant;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use modbussim_core::log_collector::LogCollector;
use modbussim_core::log_entry::{Direction, LogEntry};
use modbussim_core::master::{MasterConfig, MasterConnection, MasterState, ReadFunction, ReadResult};
use modbussim_core::transport::Transport;
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
    ConnectionCreated { id: String, label: String },
    ConnectionStateChanged { id: String, state: MasterState },
    ConnectionRemoved(String),
    ReadDone { id: String, result: ReadResult },
    Info(String),
    Error(String),
}

#[derive(Clone)]
struct ConnSnap {
    id: String,
    label: String,
    state: MasterState,
    slave_id: u8,
}

pub struct MasterApp {
    rt: Arc<Runtime>,
    connections: SharedConnections,
    events_tx: crossbeam_channel::Sender<UiEvent>,
    events_rx: crossbeam_channel::Receiver<UiEvent>,

    selected: Option<String>,
    snap: Vec<ConnSnap>,
    next_seq: u64,

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

    // Log panel
    log_panel_open: bool,
    log_cache: Vec<LogEntry>,
    log_cache_conn_id: Option<String>,
    log_last_refresh: Option<Instant>,
    log_show_rx: bool,
    log_show_tx: bool,
    log_filter_text: String,

    last_error: Option<String>,
    status_msg: Option<String>,
}

impl MasterApp {
    pub fn new(rt: Arc<Runtime>) -> Self {
        let (events_tx, events_rx) = crossbeam_channel::unbounded();
        Self {
            rt,
            connections: Arc::new(RwLock::new(Vec::new())),
            events_tx,
            events_rx,
            selected: None,
            snap: Vec::new(),
            next_seq: 1,
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
            log_panel_open: true,
            log_cache: Vec::new(),
            log_cache_conn_id: None,
            log_last_refresh: None,
            log_show_rx: true,
            log_show_tx: true,
            log_filter_text: String::new(),
            last_error: None,
            status_msg: None,
        }
    }

    fn allocate_id(&mut self) -> String {
        let id = format!("master_{}", self.next_seq);
        self.next_seq += 1;
        id
    }

    fn create_connection(&mut self, ctx: egui::Context) {
        let host = self.new_host.trim().to_string();
        let port: u16 = match self.new_port.trim().parse() {
            Ok(p) => p,
            Err(_) => {
                self.last_error = Some(format!("无效端口: {}", self.new_port));
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

    fn drain_events(&mut self) {
        while let Ok(ev) = self.events_rx.try_recv() {
            match ev {
                UiEvent::ConnectionCreated { id, label } => {
                    self.snap.push(ConnSnap {
                        id,
                        label,
                        state: MasterState::Disconnected,
                        slave_id: self.new_slave_id,
                    });
                }
                UiEvent::ConnectionStateChanged { id, state } => {
                    if let Some(s) = self.snap.iter_mut().find(|s| s.id == id) {
                        s.state = state;
                    }
                }
                UiEvent::ConnectionRemoved(id) => {
                    self.snap.retain(|s| s.id != id);
                }
                UiEvent::ReadDone { id: _, result } => {
                    self.read_result = Some(result);
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
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        self.refresh_log_cache();

        // Menu
        egui::TopBottomPanel::top("master_menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("视图", |ui| {
                    ui.checkbox(&mut self.log_panel_open, "显示日志面板");
                    ui.separator();
                    if ui.button("深色主题").clicked() {
                        ctx.set_visuals(egui::Visuals::dark());
                        ui.close_menu();
                    }
                    if ui.button("浅色主题").clicked() {
                        ctx.set_visuals(egui::Visuals::light());
                        ui.close_menu();
                    }
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
            .default_width(320.0)
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
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(id) = self.selected.clone() else {
                ui.heading("ModbusMaster — egui edition");
                ui.label("从左侧创建并连接一个会话。");
                return;
            };
            let Some(s) = self.snap.iter().find(|s| s.id == id).cloned() else {
                ui.label("连接已不存在。");
                return;
            };
            ui.heading(&s.label);
            ui.label(format!(
                "状态: {}",
                match s.state {
                    MasterState::Connected => "已连接",
                    MasterState::Disconnected => "未连接",
                    MasterState::Reconnecting => "重连中",
                    MasterState::Error => "错误",
                }
            ));
            ui.separator();

            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.strong("单次读取");
                        egui::Grid::new("read_form")
                            .num_columns(2)
                            .spacing([8.0, 4.0])
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
                        if ui.add_enabled(s.state == MasterState::Connected, egui::Button::new("读取"))
                            .clicked()
                        {
                            do_read_id = Some(id.clone());
                        }
                    });
                });

                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.strong("单次写入");
                        egui::Grid::new("write_form")
                            .num_columns(2)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("类型");
                                ui.horizontal(|ui| {
                                    ui.radio_value(&mut self.write_is_coil, false, "FC06 (保持寄存器)");
                                    ui.radio_value(&mut self.write_is_coil, true, "FC05 (线圈)");
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
                        if ui.add_enabled(s.state == MasterState::Connected, egui::Button::new("写入"))
                            .clicked()
                        {
                            do_write_id = Some(id.clone());
                        }
                    });
                });
            });

            ui.separator();

            // Read result
            if let Some(result) = &self.read_result {
                ui.strong("读取结果");
                match result {
                    ReadResult::HoldingRegisters(vs) | ReadResult::InputRegisters(vs) => {
                        render_u16_table(ui, self.read_addr, vs);
                    }
                    ReadResult::Coils(bs) | ReadResult::DiscreteInputs(bs) => {
                        render_bool_table(ui, self.read_addr, bs);
                    }
                }
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
        if !self.log_panel_open { return; }
        egui::TopBottomPanel::bottom("master_log_panel")
            .resizable(true)
            .default_height(200.0)
            .min_height(80.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("通信日志");
                    if let Some(id) = &self.log_cache_conn_id {
                        ui.label(format!("· {} ({} 条)", id, self.log_cache.len()));
                    }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.log_show_rx, "RX");
                    ui.checkbox(&mut self.log_show_tx, "TX");
                    ui.label("过滤");
                    ui.text_edit_singleline(&mut self.log_filter_text);
                });
                ui.separator();

                let q = self.log_filter_text.to_lowercase();
                let show_rx = self.log_show_rx;
                let show_tx = self.log_show_tx;
                let entries: Vec<&LogEntry> = self
                    .log_cache
                    .iter()
                    .rev()
                    .filter(|e| match e.direction {
                        Direction::Rx if !show_rx => false,
                        Direction::Tx if !show_tx => false,
                        _ => q.is_empty()
                            || e.detail.to_lowercase().contains(&q)
                            || e.function_code.name().to_lowercase().contains(&q),
                    })
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
                                ui.monospace(e.timestamp.format("%H:%M:%S%.3f").to_string());
                            });
                            row.col(|ui| {
                                let (t, c) = match e.direction {
                                    Direction::Rx => ("RX", egui::Color32::from_rgb(80, 160, 255)),
                                    Direction::Tx => ("TX", egui::Color32::from_rgb(255, 160, 80)),
                                };
                                ui.colored_label(c, t);
                            });
                            row.col(|ui| { ui.monospace(e.function_code.name()); });
                            row.col(|ui| { ui.monospace(&e.detail); });
                        });
                    });
            });
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
