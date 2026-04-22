//! Shared communication-log panel: a bottom TopBottomPanel showing TX/RX
//! entries with filter + search. Used by both the Slave and Master egui apps.

use egui::RichText;
use egui_extras::{Column, TableBuilder};
use modbussim_core::log_entry::{Direction, LogEntry};

use crate::i18n::{tr, tr2, Lang};

pub struct LogPanelState {
    pub open: bool,
    pub collapsed: bool,
    pub show_rx: bool,
    pub show_tx: bool,
    pub filter_text: String,
}

impl LogPanelState {
    pub fn new() -> Self {
        Self {
            open: true,
            collapsed: false,
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
    flavor: crate::theme::Flavor,
    lang: Lang,
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
        .default_height(240.0)
        .min_height(90.0)
        // Panel bg = L0 chrome layer; inner padding substitutes for the old card stroke.
        .frame(
            egui::Frame::new()
                .fill(crate::theme::bg_of(flavor, crate::theme::Layer::L0))
                .inner_margin(egui::Margin::symmetric(14.0 as i8, 10.0 as i8)),
        )
        .show(ctx, |ui| {
            // 单行 header：折叠箭头 + 标题 + 计数 + 右侧操作组
            ui.horizontal(|ui| {
                let chev = if state.collapsed { "▶" } else { "▼" };
                if ui
                    .add(
                        egui::Label::new(
                            RichText::new(chev)
                                .size(11.0)
                                .color(crate::theme::text_muted(flavor)),
                        )
                        .sense(egui::Sense::click()),
                    )
                    .clicked()
                {
                    state.collapsed = !state.collapsed;
                }
                ui.label(
                    RichText::new(tr(lang, "log.title"))
                        .strong()
                        .size(12.5)
                        .color(crate::theme::text_primary(flavor)),
                );
                if let Some(label) = conn_label {
                    crate::theme::text::crumb(
                        ui,
                        flavor,
                        &tr2(lang, "log.count_suffix_fmt", label, cache.len()),
                    );
                } else {
                    crate::theme::text::crumb(ui, flavor, tr(lang, "log.no_conn"));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::ui::link_action(ui, flavor, tr(lang, "log.close"), false).clicked() {
                        action = LogPanelAction::Close;
                    }
                    if crate::ui::link_action(ui, flavor, tr(lang, "log.export_csv"), false)
                        .clicked()
                    {
                        action = LogPanelAction::Export;
                    }
                    if crate::ui::link_action(ui, flavor, tr(lang, "log.clear"), false).clicked() {
                        action = LogPanelAction::Clear;
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut state.filter_text)
                            .hint_text(tr(lang, "log.filter_hint"))
                            .desired_width(160.0),
                    );
                    ui.checkbox(&mut state.show_tx, "TX");
                    ui.checkbox(&mut state.show_rx, "RX");
                });
            });

            if state.collapsed {
                return;
            }
            ui.add_space(6.0);

            let entries: Vec<&LogEntry> =
                cache.iter().rev().filter(|e| accepts(state, e)).collect();

            TableBuilder::new(ui)
                .striped(false)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::exact(150.0))
                .column(Column::exact(28.0))
                .column(Column::exact(60.0))
                .column(Column::remainder())
                .header(22.0, |mut h| {
                    h.col(|ui| crate::theme::text::tiny_caps(ui, flavor, tr(lang, "log.col.time")));
                    h.col(|ui| {
                        crate::theme::text::tiny_caps(ui, flavor, tr(lang, "log.col.direction"))
                    });
                    h.col(|ui| crate::theme::text::tiny_caps(ui, flavor, tr(lang, "log.col.fc")));
                    h.col(|ui| crate::theme::text::tiny_caps(ui, flavor, tr(lang, "log.col.detail")));
                })
                .body(|body| {
                    body.rows(18.0, entries.len(), |mut row| {
                        let e = entries[row.index()];
                        row.col(|ui| {
                            ui.add(egui::Label::new(
                                RichText::new(e.timestamp.format("%H:%M:%S%.3f").to_string())
                                    .monospace()
                                    .color(crate::theme::text_muted(flavor)),
                            ));
                        });
                        row.col(|ui| {
                            let (sym, c) = match e.direction {
                                Direction::Rx => ("←", crate::theme::success(flavor)),
                                Direction::Tx => ("→", crate::theme::accent_fg(flavor)),
                            };
                            ui.add(egui::Label::new(
                                RichText::new(sym).color(c).strong().monospace(),
                            ));
                        });
                        row.col(|ui| {
                            ui.add(egui::Label::new(
                                RichText::new(e.function_code.name())
                                    .monospace()
                                    .color(crate::theme::warn(flavor)),
                            ));
                        });
                        row.col(|ui| {
                            ui.add(egui::Label::new(
                                RichText::new(&e.detail)
                                    .monospace()
                                    .color(crate::theme::text_body(flavor)),
                            ));
                        });
                    });
                });
        });

    action
}
