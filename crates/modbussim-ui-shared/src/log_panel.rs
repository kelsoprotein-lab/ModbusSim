//! Shared communication-log panel: a bottom TopBottomPanel showing TX/RX
//! entries with filter + search. Used by both the Slave and Master egui apps.

use egui::Color32;
use egui_extras::{Column, TableBuilder};
use modbussim_core::log_entry::{Direction, LogEntry};

pub struct LogPanelState {
    pub open: bool,
    pub show_rx: bool,
    pub show_tx: bool,
    pub filter_text: String,
}

impl LogPanelState {
    pub fn new() -> Self {
        Self {
            open: true,
            show_rx: true,
            show_tx: true,
            filter_text: String::new(),
        }
    }
}

impl Default for LogPanelState {
    fn default() -> Self {
        Self::new()
    }
}

pub enum LogPanelAction {
    None,
    Clear,
    Export,
    Close,
}

fn accepts(state: &LogPanelState, e: &LogEntry) -> bool {
    match e.direction {
        Direction::Rx if !state.show_rx => return false,
        Direction::Tx if !state.show_tx => return false,
        _ => {}
    }
    if state.filter_text.is_empty() {
        return true;
    }
    let q = state.filter_text.to_lowercase();
    e.detail.to_lowercase().contains(&q) || e.function_code.name().to_lowercase().contains(&q)
}

/// Render the log panel. Returns an action requested by the user (clear,
/// export, close) or `None` if only filter state changed.
///
/// `conn_label` is shown in the header; pass `None` when no connection is selected.
pub fn render(
    ctx: &egui::Context,
    state: &mut LogPanelState,
    cache: &[LogEntry],
    conn_label: Option<&str>,
) -> LogPanelAction {
    if !state.open {
        return LogPanelAction::None;
    }

    let mut action = LogPanelAction::None;

    egui::TopBottomPanel::bottom("shared_log_panel")
        .resizable(true)
        .default_height(220.0)
        .min_height(80.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("通信日志");
                if let Some(label) = conn_label {
                    ui.label(format!("· {} ({} 条)", label, cache.len()));
                } else {
                    ui.label("（选中连接以查看）");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").on_hover_text("关闭日志面板").clicked() {
                        action = LogPanelAction::Close;
                    }
                    if ui.small_button("导出 CSV").clicked() {
                        action = LogPanelAction::Export;
                    }
                    if ui.small_button("清空").clicked() {
                        action = LogPanelAction::Clear;
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.show_rx, "RX");
                ui.checkbox(&mut state.show_tx, "TX");
                ui.label("过滤");
                ui.text_edit_singleline(&mut state.filter_text);
            });
            ui.separator();

            let entries: Vec<&LogEntry> =
                cache.iter().rev().filter(|e| accepts(state, e)).collect();

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
                                Direction::Rx => ("RX", Color32::from_rgb(80, 160, 255)),
                                Direction::Tx => ("TX", Color32::from_rgb(255, 160, 80)),
                            };
                            ui.colored_label(c, t);
                        });
                        row.col(|ui| { ui.monospace(e.function_code.name()); });
                        row.col(|ui| { ui.monospace(&e.detail); });
                    });
                });
        });

    action
}
