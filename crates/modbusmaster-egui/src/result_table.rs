//! 读取/轮询结果的 egui 表格渲染 + 功能码人读标签。

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use modbussim_core::master::ReadFunction;
use modbussim_ui_shared::i18n::{tr, Lang};

pub fn read_fc_label(f: ReadFunction, lang: Lang) -> &'static str {
    let key = match f {
        ReadFunction::ReadCoils => "read.fc.coils",
        ReadFunction::ReadDiscreteInputs => "read.fc.discrete",
        ReadFunction::ReadHoldingRegisters => "read.fc.holding",
        ReadFunction::ReadInputRegisters => "read.fc.input",
    };
    tr(lang, key)
}

pub fn render_u16_table(ui: &mut egui::Ui, start: u16, values: &[u16], lang: Lang) {
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .column(Column::exact(80.0))
        .column(Column::exact(100.0))
        .column(Column::exact(90.0))
        .column(Column::remainder())
        .header(20.0, |mut h| {
            h.col(|ui| {
                ui.strong(tr(lang, "regtable.address"));
            });
            h.col(|ui| {
                ui.strong(tr(lang, "result.col.unsigned"));
            });
            h.col(|ui| {
                ui.strong(tr(lang, "result.col.signed"));
            });
            h.col(|ui| {
                ui.strong(tr(lang, "result.col.hex"));
            });
        })
        .body(|body| {
            body.rows(18.0, values.len(), |mut row| {
                let i = row.index();
                let addr = start.wrapping_add(i as u16);
                let v = values[i];
                row.col(|ui| {
                    ui.monospace(format!("{}", addr));
                });
                row.col(|ui| {
                    ui.monospace(v.to_string());
                });
                row.col(|ui| {
                    ui.monospace((v as i16).to_string());
                });
                row.col(|ui| {
                    ui.monospace(format!("0x{:04X}", v));
                });
            });
        });
}

pub fn render_bool_table(ui: &mut egui::Ui, start: u16, values: &[bool], lang: Lang) {
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .column(Column::exact(80.0))
        .column(Column::remainder())
        .header(20.0, |mut h| {
            h.col(|ui| {
                ui.strong(tr(lang, "regtable.address"));
            });
            h.col(|ui| {
                ui.strong(tr(lang, "result.col.bool"));
            });
        })
        .body(|body| {
            body.rows(18.0, values.len(), |mut row| {
                let i = row.index();
                let addr = start.wrapping_add(i as u16);
                row.col(|ui| {
                    ui.monospace(format!("{}", addr));
                });
                row.col(|ui| {
                    ui.monospace(if values[i] { "true" } else { "false" });
                });
            });
        });
}
