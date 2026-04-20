//! Multi-register value analysis panel.
//!
//! Given 0..=4 selected contiguous u16 register values, render all reasonable
//! numeric interpretations side-by-side so engineers can read a Modbus
//! response without doing bit math in their head.
//!
//! - 1 word  → U16 (Unsigned / Signed / Hex / Binary)
//! - 2 words → U32, I32, Float32 — each in 4 byte orders (AB CD / CD AB /
//!             BA DC / DC BA)
//! - 4 words → Float64 in 4 byte orders
//!
//! Read-only for the MVP. Editing can be layered on top later by exposing a
//! "edit this row" click-through.

use egui::RichText;
use modbussim_core::register::{decode_value, DataType, Endian};

use crate::theme::Flavor;

const ENDIANS: [(Endian, &str); 4] = [
    (Endian::Big, "AB CD"),
    (Endian::Little, "CD AB"),
    (Endian::MidBig, "BA DC"),
    (Endian::MidLittle, "DC BA"),
];

/// Render the ValuePanel into `ui`. `values` should contain 0..=4 contiguous
/// u16 register values (caller is responsible for gathering them from the
/// currently-selected rows). `base_addr` is displayed as a caption.
pub fn render(
    ui: &mut egui::Ui,
    flavor: Flavor,
    values: &[u16],
    base_addr: Option<u16>,
) {
    ui.label(RichText::new("值解析").strong().size(13.5));
    match values.len() {
        0 => {
            crate::ui::caption(ui, flavor, "选中 1–4 行寄存器以查看多格式解析");
            return;
        }
        1 => {
            if let Some(a) = base_addr {
                crate::ui::caption(ui, flavor, format!("地址 {} · 1 word", a));
            }
            render_single(ui, values[0]);
        }
        2 => {
            if let Some(a) = base_addr {
                crate::ui::caption(ui, flavor, format!("地址 {}..{} · 2 words", a, a + 1));
            }
            render_double(ui, [values[0], values[1]]);
        }
        3 => {
            if let Some(a) = base_addr {
                crate::ui::caption(ui, flavor, format!("地址 {}..{} · 选 2 或 4 行用于组合", a, a + 2));
            }
            render_single(ui, values[0]);
        }
        _ => {
            // 4+
            if let Some(a) = base_addr {
                crate::ui::caption(ui, flavor, format!("地址 {}..{} · 4 words", a, a + 3));
            }
            render_double(ui, [values[0], values[1]]);
            ui.add_space(4.0);
            ui.separator();
            render_quad(ui, [values[0], values[1], values[2], values[3]]);
        }
    }
}

fn render_single(ui: &mut egui::Ui, v: u16) {
    egui::Grid::new("vp_single")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label("Unsigned");
            ui.monospace(v.to_string());
            ui.end_row();
            ui.label("Signed");
            ui.monospace((v as i16).to_string());
            ui.end_row();
            ui.label("Hex");
            ui.monospace(format!("0x{:04X}", v));
            ui.end_row();
            ui.label("Binary");
            let b = format!("{:016b}", v);
            ui.monospace(format!("{} {} {} {}", &b[0..4], &b[4..8], &b[8..12], &b[12..16]));
            ui.end_row();
        });
}

fn render_double(ui: &mut egui::Ui, words: [u16; 2]) {
    egui::Grid::new("vp_double")
        .num_columns(4)
        .spacing([12.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(RichText::new("字节序").strong());
            ui.label(RichText::new("UInt32").strong());
            ui.label(RichText::new("Int32").strong());
            ui.label(RichText::new("Float32").strong());
            ui.end_row();

            for (e, label) in ENDIANS {
                ui.monospace(label);
                ui.monospace(format_dt(&words, DataType::UInt32, e));
                ui.monospace(format_dt(&words, DataType::Int32, e));
                ui.monospace(format_dt(&words, DataType::Float32, e));
                ui.end_row();
            }
        });
}

fn render_quad(ui: &mut egui::Ui, words: [u16; 4]) {
    ui.label(RichText::new("Double (64-bit)").strong());
    egui::Grid::new("vp_quad")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            for (e, label) in ENDIANS {
                ui.monospace(label);
                ui.monospace(format_f64(&words, e));
                ui.end_row();
            }
        });
}

fn format_dt(words: &[u16], data_type: DataType, endian: Endian) -> String {
    match decode_value(words, data_type, endian) {
        Ok(v) => match data_type {
            DataType::UInt32 => format!("{}", v as u32),
            DataType::Int32 => format!("{}", v as i32),
            DataType::Float32 => format!("{:.6}", v as f32),
            _ => "—".to_string(),
        },
        Err(_) => "—".to_string(),
    }
}

/// Decode 4 u16 words as a 64-bit double under the given endian.
/// `endian` is interpreted as the pairwise order applied to consecutive 2-word
/// pairs: AB CD = [a,b,c,d] → bytes [a_hi,a_lo,b_hi,b_lo,c_hi,c_lo,d_hi,d_lo].
fn format_f64(words: &[u16; 4], endian: Endian) -> String {
    let bytes = apply_endian_4(words, endian);
    let v = f64::from_be_bytes(bytes);
    if v.is_finite() {
        format!("{:.9}", v)
    } else {
        "NaN / Inf".to_string()
    }
}

fn apply_endian_4(words: &[u16; 4], endian: Endian) -> [u8; 8] {
    // Construct a naive big-endian byte sequence from the 4 words, then apply
    // the same AB/CD/BA/DC transformation pairwise for each 16-bit slot.
    let mut b = [0u8; 8];
    let [w0, w1, w2, w3] = *words;
    // Word-level order — match the 2-word endian mapping: (w0, w1) at positions
    // 0..4 and (w2, w3) at 4..8 under the same AB/CD/BA/DC rule.
    let (a0, b0, c0, d0) = word_pair_to_bytes(w0, w1, endian);
    let (a1, b1, c1, d1) = word_pair_to_bytes(w2, w3, endian);
    b[0] = a0; b[1] = b0; b[2] = c0; b[3] = d0;
    b[4] = a1; b[5] = b1; b[6] = c1; b[7] = d1;
    b
}

/// Mirror of `modbussim_core::register::apply_endian_decode` but returns a
/// 4-tuple (big-endian layout) so it can be composed for 64-bit.
fn word_pair_to_bytes(reg0: u16, reg1: u16, endian: Endian) -> (u8, u8, u8, u8) {
    let r0 = reg0.to_be_bytes();
    let r1 = reg1.to_be_bytes();
    match endian {
        Endian::Big => (r0[0], r0[1], r1[0], r1[1]),
        Endian::Little => (r1[0], r1[1], r0[0], r0[1]),
        Endian::MidBig => (r0[1], r0[0], r1[1], r1[0]),
        Endian::MidLittle => (r1[1], r1[0], r0[1], r0[0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_abcd_roundtrip() {
        // 25.0 = 0x41C80000 → AB CD = words [0x41C8, 0x0000]
        let got = format_dt(&[0x41C8, 0x0000], DataType::Float32, Endian::Big);
        assert!(got.starts_with("25."), "got={got}");
    }

    #[test]
    fn u32_big_order() {
        // 0x0001_0002 = 65538
        let got = format_dt(&[0x0001, 0x0002], DataType::UInt32, Endian::Big);
        assert_eq!(got, "65538");
    }

    #[test]
    fn i32_little_is_negative_example() {
        // bytes AB CD = 0x00, 0x00, 0xFF, 0xFF → i32 = 65535
        // but with Endian::Little (CD AB) words [a,b] → bytes [c,d,a,b]
        // so words [0x0000, 0xFFFF] in Little = 0xFFFF_0000 = -65536
        let got = format_dt(&[0x0000, 0xFFFF], DataType::Int32, Endian::Little);
        assert_eq!(got, "-65536");
    }

    #[test]
    fn f64_big_order() {
        // 1.0 = 0x3FF0_0000_0000_0000 → words [0x3FF0, 0x0000, 0x0000, 0x0000]
        let s = format_f64(&[0x3FF0, 0x0000, 0x0000, 0x0000], Endian::Big);
        assert!(s.starts_with("1."), "got={s}");
    }
}
