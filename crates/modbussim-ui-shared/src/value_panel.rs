//! Multi-register value analysis & editor panel.
//!
//! Given 0..=4 selected contiguous u16 register values, render every sensible
//! interpretation side-by-side and allow the user to type a new value in any
//! cell. Upon commit (Enter / focus lost) the string is parsed back into 1/2/4
//! `u16` words under the chosen data type + byte order, and returned to the
//! caller as `Vec<(u16_addr, u16_value)>`.
//!
//! - 1 word  → U16 Unsigned / Signed / Hex / Binary
//! - 2 words → U32, I32, Float32 each in 4 byte orders (AB CD / CD AB /
//!             BA DC / DC BA)
//! - 4 words → Float64 in 4 byte orders

use egui::{Id, Key, RichText};
use modbussim_core::register::{decode_value, DataType, Endian};

use crate::theme::Flavor;

const ENDIANS: [(Endian, &str); 4] = [
    (Endian::Big, "AB CD"),
    (Endian::Little, "CD AB"),
    (Endian::MidBig, "BA DC"),
    (Endian::MidLittle, "DC BA"),
];

/// 64-bit byte order naming follows the Modbus convention of spelling all 8
/// bytes. Four common combinations cover most industry devices.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum F64Order {
    Abcdefgh, // full big-endian
    Hgfedcba, // full little-endian
    Badcfehg, // byte-swap within each 16-bit word
    Ghefcdab, // word-swap (reverse word order, bytes big-endian within)
}

const F64_ORDERS: [(F64Order, &str); 4] = [
    (F64Order::Abcdefgh, "ABCDEFGH"),
    (F64Order::Hgfedcba, "HGFEDCBA"),
    (F64Order::Badcfehg, "BADCFEHG"),
    (F64Order::Ghefcdab, "GHEFCDAB"),
];

/// Decode 4 u16 registers as an f64 under the given 8-byte order.
pub fn decode_f64(ws: &[u16], order: F64Order) -> f64 {
    let w0 = ws.first().copied().unwrap_or(0);
    let w1 = ws.get(1).copied().unwrap_or(0);
    let w2 = ws.get(2).copied().unwrap_or(0);
    let w3 = ws.get(3).copied().unwrap_or(0);
    let b0 = w0.to_be_bytes();
    let b1 = w1.to_be_bytes();
    let b2 = w2.to_be_bytes();
    let b3 = w3.to_be_bytes();
    let bytes: [u8; 8] = match order {
        F64Order::Abcdefgh => [b0[0], b0[1], b1[0], b1[1], b2[0], b2[1], b3[0], b3[1]],
        F64Order::Hgfedcba => [b3[1], b3[0], b2[1], b2[0], b1[1], b1[0], b0[1], b0[0]],
        F64Order::Badcfehg => [b0[1], b0[0], b1[1], b1[0], b2[1], b2[0], b3[1], b3[0]],
        F64Order::Ghefcdab => [b3[0], b3[1], b2[0], b2[1], b1[0], b1[1], b0[0], b0[1]],
    };
    f64::from_be_bytes(bytes)
}

/// Encode an f64 back into 4 u16 registers under the given 8-byte order.
pub fn encode_f64(value: f64, order: F64Order) -> [u16; 4] {
    let b = value.to_be_bytes(); // [A, B, C, D, E, F, G, H]
    let out: [u8; 8] = match order {
        F64Order::Abcdefgh => [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]],
        F64Order::Hgfedcba => [b[7], b[6], b[5], b[4], b[3], b[2], b[1], b[0]],
        F64Order::Badcfehg => [b[1], b[0], b[3], b[2], b[5], b[4], b[7], b[6]],
        F64Order::Ghefcdab => [b[6], b[7], b[4], b[5], b[2], b[3], b[0], b[1]],
    };
    [
        u16::from_be_bytes([out[0], out[1]]),
        u16::from_be_bytes([out[2], out[3]]),
        u16::from_be_bytes([out[4], out[5]]),
        u16::from_be_bytes([out[6], out[7]]),
    ]
}

/// Render the panel. Returns writes `(addr, value)` if the user committed
/// an edit this frame, else None.
pub fn render(
    ui: &mut egui::Ui,
    flavor: Flavor,
    values: &[u16],
    base_addr: Option<u16>,
) -> Option<Vec<(u16, u16)>> {
    ui.label(RichText::new("值解析").strong().size(13.5));
    match values.len() {
        0 => {
            crate::ui::caption(ui, flavor, "选中 1–4 行寄存器以查看/编辑多格式");
            None
        }
        1 => {
            let base = base_addr.unwrap_or(0);
            crate::ui::caption(ui, flavor, format!("地址 {} · 1 word", base));
            render_single(ui, base, values[0])
        }
        2 => {
            let base = base_addr.unwrap_or(0);
            crate::ui::caption(
                ui,
                flavor,
                format!("地址 {}..{} · 2 words", base, base + 1),
            );
            render_double(ui, base, [values[0], values[1]])
        }
        3 => {
            // Not a standard width — fall back to single on the first row.
            let base = base_addr.unwrap_or(0);
            crate::ui::caption(
                ui,
                flavor,
                format!("地址 {}..{} · 选 2 或 4 行组合", base, base + 2),
            );
            render_single(ui, base, values[0])
        }
        _ => {
            let base = base_addr.unwrap_or(0);
            crate::ui::caption(
                ui,
                flavor,
                format!("地址 {}..{} · 4 words", base, base + 3),
            );
            let w1 = render_double(ui, base, [values[0], values[1]]);
            ui.add_space(4.0);
            ui.separator();
            let w2 = render_quad(ui, base, [values[0], values[1], values[2], values[3]]);
            combine(w1, w2)
        }
    }
}

fn combine(a: Option<Vec<(u16, u16)>>, b: Option<Vec<(u16, u16)>>) -> Option<Vec<(u16, u16)>> {
    match (a, b) {
        (None, None) => None,
        (Some(mut x), None) | (None, Some(mut x)) => Some(std::mem::take(&mut x)),
        (Some(mut x), Some(mut y)) => {
            x.append(&mut y);
            Some(x)
        }
    }
}

/// Buffer for a single editable cell; stored in egui memory keyed by a stable id.
#[derive(Clone, Default)]
struct EditBuf {
    text: String,
    last_source: u128, // hash-ish of (values, addr) to detect stale cache
}

fn addr_hash(addr: u16, words: &[u16]) -> u128 {
    let mut h = (addr as u128) << 80;
    for (i, w) in words.iter().enumerate() {
        h ^= (*w as u128) << (i * 16);
    }
    h
}

/// Render one editable cell that parses on commit via `parse_fn`.
/// `display` is the canonical string for the current value; overwritten into
/// the edit buffer whenever the source value changes and the field is not
/// being edited.
fn edit_cell(
    ui: &mut egui::Ui,
    id: Id,
    display: String,
    source_hash: u128,
    parse_fn: impl Fn(&str) -> Option<Vec<(u16, u16)>>,
) -> Option<Vec<(u16, u16)>> {
    let mut buf: EditBuf = ui
        .ctx()
        .data_mut(|d| d.get_temp::<EditBuf>(id))
        .unwrap_or_default();

    let resp = ui.add(
        egui::TextEdit::singleline(&mut buf.text)
            .desired_width(120.0)
            .font(egui::TextStyle::Monospace),
    );

    let has_focus = resp.has_focus();
    if !has_focus && buf.last_source != source_hash {
        buf.text = display;
        buf.last_source = source_hash;
    }

    let mut result = None;
    let commit = resp.lost_focus()
        && ui.ctx().input(|i| i.key_pressed(Key::Enter) || !i.pointer.any_pressed())
        || (has_focus && ui.ctx().input(|i| i.key_pressed(Key::Enter)));
    if commit && !buf.text.is_empty() {
        if let Some(writes) = parse_fn(buf.text.trim()) {
            result = Some(writes);
            buf.last_source = 0; // force refresh from fresh cache next frame
        }
    }

    ui.ctx().data_mut(|d| d.insert_temp(id, buf));
    result
}

// --- Single-word formats ---

fn render_single(ui: &mut egui::Ui, addr: u16, v: u16) -> Option<Vec<(u16, u16)>> {
    let h = addr_hash(addr, &[v]);
    let mut out: Option<Vec<(u16, u16)>> = None;
    egui::Grid::new("vp_single")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label("Unsigned");
            out = combine(
                out.take(),
                edit_cell(
                    ui,
                    Id::new(("vp_u16", addr)),
                    v.to_string(),
                    h,
                    move |s| {
                        let n: u32 = s.parse().ok()?;
                        if n > u16::MAX as u32 { return None; }
                        Some(vec![(addr, n as u16)])
                    },
                ),
            );
            ui.end_row();

            ui.label("Signed");
            out = combine(
                out.take(),
                edit_cell(
                    ui,
                    Id::new(("vp_i16", addr)),
                    (v as i16).to_string(),
                    h,
                    move |s| {
                        let n: i32 = s.parse().ok()?;
                        if n < i16::MIN as i32 || n > i16::MAX as i32 { return None; }
                        Some(vec![(addr, n as i16 as u16)])
                    },
                ),
            );
            ui.end_row();

            ui.label("Hex");
            out = combine(
                out.take(),
                edit_cell(
                    ui,
                    Id::new(("vp_hex", addr)),
                    format!("0x{:04X}", v),
                    h,
                    move |s| {
                        let s = s.trim_start_matches("0x").trim_start_matches("0X");
                        let n = u16::from_str_radix(s, 16).ok()?;
                        Some(vec![(addr, n)])
                    },
                ),
            );
            ui.end_row();

            ui.label("Binary");
            let b = format!("{:016b}", v);
            let display = format!("{} {} {} {}", &b[0..4], &b[4..8], &b[8..12], &b[12..16]);
            out = combine(
                out.take(),
                edit_cell(
                    ui,
                    Id::new(("vp_bin", addr)),
                    display,
                    h,
                    move |s| {
                        let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
                        let n = u16::from_str_radix(&cleaned, 2).ok()?;
                        Some(vec![(addr, n)])
                    },
                ),
            );
            ui.end_row();
        });
    out
}

// --- Double-word formats (U32 / I32 / F32 × 4 endians) ---

fn render_double(
    ui: &mut egui::Ui,
    base: u16,
    words: [u16; 2],
) -> Option<Vec<(u16, u16)>> {
    let h = addr_hash(base, &words);
    let mut out: Option<Vec<(u16, u16)>> = None;
    egui::Grid::new("vp_double")
        .num_columns(4)
        .spacing([10.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(RichText::new("字节序").strong());
            ui.label(RichText::new("UInt32").strong());
            ui.label(RichText::new("Int32").strong());
            ui.label(RichText::new("Float32").strong());
            ui.end_row();

            for (e, label) in ENDIANS {
                ui.monospace(label);
                // U32
                let display_u32 = format_dt(&words, DataType::UInt32, e);
                out = combine(
                    out.take(),
                    edit_cell(
                        ui,
                        Id::new(("vp_u32", base, label)),
                        display_u32,
                        h,
                        move |s| {
                            let n: u64 = s.parse().ok()?;
                            if n > u32::MAX as u64 { return None; }
                            let pair = encode_u32(n as u32, e);
                            Some(vec![(base, pair[0]), (base + 1, pair[1])])
                        },
                    ),
                );
                // I32
                let display_i32 = format_dt(&words, DataType::Int32, e);
                out = combine(
                    out.take(),
                    edit_cell(
                        ui,
                        Id::new(("vp_i32", base, label)),
                        display_i32,
                        h,
                        move |s| {
                            let n: i32 = s.parse().ok()?;
                            let pair = encode_u32(n as u32, e);
                            Some(vec![(base, pair[0]), (base + 1, pair[1])])
                        },
                    ),
                );
                // F32
                let display_f32 = format_dt(&words, DataType::Float32, e);
                out = combine(
                    out.take(),
                    edit_cell(
                        ui,
                        Id::new(("vp_f32", base, label)),
                        display_f32,
                        h,
                        move |s| {
                            let f: f32 = s.parse().ok()?;
                            let pair = encode_u32(f.to_bits(), e);
                            Some(vec![(base, pair[0]), (base + 1, pair[1])])
                        },
                    ),
                );
                ui.end_row();
            }
        });
    out
}

// --- Quad-word (Float64) ---

fn render_quad(
    ui: &mut egui::Ui,
    base: u16,
    words: [u16; 4],
) -> Option<Vec<(u16, u16)>> {
    let h = addr_hash(base, &words);
    let mut out: Option<Vec<(u16, u16)>> = None;
    ui.label(RichText::new("Double (64-bit)").strong());
    egui::Grid::new("vp_quad")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            for (order, label) in F64_ORDERS {
                ui.monospace(label);
                let v = decode_f64(&words, order);
                let display = if v.is_finite() {
                    format!("{:.9}", v)
                } else {
                    "NaN / Inf".to_string()
                };
                out = combine(
                    out.take(),
                    edit_cell(
                        ui,
                        Id::new(("vp_f64", base, label)),
                        display,
                        h,
                        move |s| {
                            let f: f64 = s.parse().ok()?;
                            let w = encode_f64(f, order);
                            Some(vec![
                                (base, w[0]),
                                (base + 1, w[1]),
                                (base + 2, w[2]),
                                (base + 3, w[3]),
                            ])
                        },
                    ),
                );
                ui.end_row();
            }
        });
    out
}

// --- Decoding helpers ---

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

/// Encode a u32 (treated as 4 big-endian bytes) into a pair of u16 registers
/// under the given endian. Inverse of the decode transformation in
/// `modbussim_core::register::apply_endian_decode`.
fn encode_u32(value: u32, endian: Endian) -> [u16; 2] {
    let b = value.to_be_bytes(); // [a, b, c, d]
    let [a, b, c, d] = b;
    match endian {
        Endian::Big => [u16::from_be_bytes([a, b]), u16::from_be_bytes([c, d])],
        Endian::Little => [u16::from_be_bytes([c, d]), u16::from_be_bytes([a, b])],
        Endian::MidBig => [u16::from_be_bytes([b, a]), u16::from_be_bytes([d, c])],
        Endian::MidLittle => [u16::from_be_bytes([d, c]), u16::from_be_bytes([b, a])],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_abcd_roundtrip() {
        let got = format_dt(&[0x41C8, 0x0000], DataType::Float32, Endian::Big);
        assert!(got.starts_with("25."), "got={got}");
    }

    #[test]
    fn u32_big_order() {
        let got = format_dt(&[0x0001, 0x0002], DataType::UInt32, Endian::Big);
        assert_eq!(got, "65538");
    }

    #[test]
    fn i32_little_is_negative_example() {
        let got = format_dt(&[0x0000, 0xFFFF], DataType::Int32, Endian::Little);
        assert_eq!(got, "-65536");
    }

    #[test]
    fn f64_abcdefgh_decodes_one() {
        let v = decode_f64(&[0x3FF0, 0x0000, 0x0000, 0x0000], F64Order::Abcdefgh);
        assert!((v - 1.0).abs() < 1e-12, "got={v}");
    }

    #[test]
    fn f64_encode_decode_roundtrip() {
        for order in [
            F64Order::Abcdefgh,
            F64Order::Hgfedcba,
            F64Order::Badcfehg,
            F64Order::Ghefcdab,
        ] {
            let w = encode_f64(3.141592653589793, order);
            let back = decode_f64(&w, order);
            assert!((back - 3.141592653589793).abs() < 1e-12);
        }
    }

    #[test]
    fn encode_u32_roundtrip_big() {
        let pair = encode_u32(0x41C80000, Endian::Big);
        assert_eq!(pair, [0x41C8, 0x0000]);
    }

    #[test]
    fn encode_u32_roundtrip_little() {
        // decode(0x0000, 0x41C8, Little) == 25.0 as f32
        // Our encode should produce the reverse.
        let pair = encode_u32(0x41C80000, Endian::Little);
        assert_eq!(pair, [0x0000, 0x41C8]);
        // And decoding back under Little yields 0x41C80000.
        let decoded =
            decode_value(&[pair[0], pair[1]], DataType::Float32, Endian::Little).unwrap();
        assert!((decoded - 25.0).abs() < 1e-6);
    }
}
