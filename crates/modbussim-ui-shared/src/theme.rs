//! Visual theme inspired by VS Code Dark+ and Light+.
//!
//! Design goals: neutral grays (no blue-tinted base), restrained accent,
//! compact spacing, readable-at-desk typography. Uses the
//! `catppuccin_egui::Theme` struct purely as a color-container for reuse of
//! its styling code — the actual palette is hand-built.

use egui::Color32;
use serde::{Deserialize, Serialize};

/// Theme flavor. Serde-compat with older "mocha"/"latte"/... values so a
/// storage-persisted flavor from an earlier build still deserializes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Flavor {
    /// Dark+ — default dark theme (bg #1e1e1e)
    #[default]
    Mocha,
    /// Dark+ alias (kept for serde compat with older save files)
    Macchiato,
    /// Dark+ alias
    Frappe,
    /// Light+ — light mode (bg #f3f3f3)
    Latte,
}

impl Flavor {
    pub fn label(self) -> &'static str {
        match self {
            Flavor::Mocha | Flavor::Macchiato | Flavor::Frappe => "Dark+",
            Flavor::Latte => "Light+",
        }
    }

    pub fn is_dark(self) -> bool {
        !matches!(self, Flavor::Latte)
    }

    pub fn palette(self) -> catppuccin_egui::Theme {
        if self.is_dark() { VSCODE_DARK } else { VSCODE_LIGHT }
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Three-level background layer for flat-layered visual style.
/// Diff between neighbors ≥ 6 RGB units so regions are visually distinct
/// without painting explicit stroke borders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Chrome (SidePanel, bottom log panel) — deepest.
    L0,
    /// Main content area (CentralPanel).
    L1,
    /// Data container (tables, TextEdit, slider track).
    L2,
}

/// Background color for the given layer under the given flavor.
pub fn bg_of(flavor: Flavor, layer: Layer) -> Color32 {
    if flavor.is_dark() {
        match layer {
            Layer::L0 => rgb(0x01, 0x04, 0x09), // #010409 chrome
            Layer::L1 => rgb(0x0d, 0x11, 0x17), // #0d1117 surface
            Layer::L2 => rgb(0x16, 0x1b, 0x22), // #161b22 raised
        }
    } else {
        match layer {
            Layer::L0 => rgb(0xf4, 0xf4, 0xf5), // #f4f4f5
            Layer::L1 => rgb(0xfa, 0xfa, 0xfa), // #fafafa
            Layer::L2 => rgb(0xff, 0xff, 0xff), // #ffffff
        }
    }
}

/// Hover fill used by non-primary buttons and list rows.
pub fn bg_hover(flavor: Flavor) -> Color32 {
    if flavor.is_dark() {
        rgb(0x16, 0x1b, 0x22) // = Layer::L2
    } else {
        rgb(0xe4, 0xe4, 0xe7)
    }
}

/// Selected row fill (applied full-row in register tables / scan-group list).
pub fn bg_selected_row(flavor: Flavor) -> Color32 {
    if flavor.is_dark() {
        // accent.primary @ 15% alpha → 解多重不蒙底色
        Color32::from_rgba_unmultiplied(0x1f, 0x6f, 0xeb, 0x26)
    } else {
        Color32::from_rgba_unmultiplied(0x25, 0x63, 0xeb, 0x1a)
    }
}

/// VS Code Dark+ palette mapped into catppuccin_egui::Theme slots.
///   base    = editor bg          = #1e1e1e
///   mantle  = side bar / panels  = #252526
///   crust   = activity bar       = #333333 (darker contrast)
///   surface0/1/2 = successively lighter widget bg for layering
///   overlay0/1/2 = borders / strokes
///   text    = main fg            = #cccccc
///   subtext = muted fg           = #858585
///   blue    = accent             = #0e639c (button primary)
/// JetBrains Darcula palette: warm-gray editor with orange accent.
pub const VSCODE_DARK: catppuccin_egui::Theme = catppuccin_egui::Theme {
    // Accents — Darcula semantic tokens
    rosewater: rgb(255, 198, 109),
    flamingo: rgb(204, 120, 50),
    pink: rgb(189, 126, 199),
    mauve: rgb(157, 121, 209),
    red: rgb(255, 100, 100),
    maroon: rgb(169, 46, 34),
    peach: rgb(204, 120, 50),      // #cc7832 — keyword orange (primary accent)
    yellow: rgb(255, 198, 109),    // #ffc66d — class / highlight
    green: rgb(106, 135, 89),      // #6a8759 — string / success
    teal: rgb(119, 159, 165),
    sky: rgb(152, 195, 250),
    sapphire: rgb(106, 135, 175),
    blue: rgb(106, 135, 175),      // #6a87af — secondary blue
    lavender: rgb(157, 121, 209),
    // Foreground
    text: rgb(220, 223, 228),      // #dcdfe4 — brighter than stock Darcula #a9b7c6
    subtext1: rgb(180, 183, 188),
    subtext0: rgb(156, 160, 164),  // #9ca0a4 — still muted but ≥4.5:1 on #2b2b2b
    // Borders / strokes (warm gray)
    overlay2: rgb(98, 101, 104),
    overlay1: rgb(81, 86, 89),     // #515659 — separator
    overlay0: rgb(69, 73, 74),
    // Surfaces — one-step layering
    surface2: rgb(77, 80, 82),
    surface1: rgb(60, 63, 65),     // #3c3f41 — side panels
    surface0: rgb(49, 51, 53),     // #313335
    // Backgrounds — Darcula reference values
    base: rgb(43, 43, 43),         // #2b2b2b — editor bg
    mantle: rgb(60, 63, 65),       // #3c3f41 — tool windows
    crust: rgb(37, 37, 37),        // #252525 — darkest
};

pub const VSCODE_LIGHT: catppuccin_egui::Theme = catppuccin_egui::Theme {
    // redisant-MSE–inspired light palette: near-white base, light gray
    // toolbars, a crisp industrial blue accent.
    rosewater: rgb(142, 95, 0),
    flamingo: rgb(141, 76, 43),
    pink: rgb(175, 0, 219),
    mauve: rgb(94, 68, 172),
    red: rgb(200, 51, 54),
    maroon: rgb(157, 20, 15),
    peach: rgb(175, 82, 0),
    yellow: rgb(145, 120, 0),
    green: rgb(0, 128, 64),
    teal: rgb(0, 128, 128),
    sky: rgb(0, 120, 180),
    sapphire: rgb(0, 90, 180),
    blue: rgb(59, 154, 232),          // #3b9ae8 — redisant industrial blue
    lavender: rgb(94, 68, 172),
    text: rgb(51, 51, 51),            // #333333
    subtext1: rgb(102, 102, 102),
    subtext0: rgb(140, 140, 140),
    overlay2: rgb(168, 172, 180),
    overlay1: rgb(192, 196, 204),
    overlay0: rgb(208, 208, 208),     // #d0d0d0 — card stroke
    surface2: rgb(232, 232, 232),
    surface1: rgb(240, 240, 240),
    surface0: rgb(245, 245, 245),     // #f5f5f5 — toolbar
    base: rgb(255, 255, 255),         // #ffffff — editor
    mantle: rgb(245, 245, 245),       // #f5f5f5 — side panels
    crust: rgb(232, 232, 232),        // #e8e8e8 — deepest light
};

/// Apply palette + tight VS Code-ish layout/type defaults.
pub fn apply(ctx: &egui::Context, flavor: Flavor) {
    catppuccin_egui::set_theme(ctx, flavor.palette());

    // catppuccin_egui::set_theme does not always map `panel_fill` / `window_fill`
    // / widget backgrounds correctly in light mode, so we force the critical
    // fields ourselves to match the target industrial palette.
    ctx.style_mut(|s| {
        if flavor.is_dark() {
            let panel       = bg_of(flavor, Layer::L1);                 // #0d1117
            let panel_alt   = bg_of(flavor, Layer::L0);                 // #010409
            let raised      = bg_of(flavor, Layer::L2);                 // #161b22
            let stroke      = border_strong(flavor);                    // #30363d
            let stroke_soft = border_subtle(flavor);                    // #21262d
            let fg          = text_body(flavor);                        // #c9d1d9
            let strong_fg   = text_primary(flavor);                     // #e6edf3
            let sel_bg      = bg_selected_row(flavor);
            let acc         = accent(flavor);                           // #1f6feb
            s.visuals.panel_fill = panel;
            s.visuals.window_fill = panel_alt;
            s.visuals.extreme_bg_color = panel_alt;
            s.visuals.faint_bg_color = raised;
            s.visuals.code_bg_color = raised;
            s.visuals.widgets.noninteractive.bg_fill = panel_alt;
            s.visuals.widgets.noninteractive.weak_bg_fill = panel;
            s.visuals.widgets.noninteractive.bg_stroke.color = stroke_soft;
            s.visuals.widgets.noninteractive.fg_stroke.color = fg;
            s.visuals.widgets.inactive.bg_fill = raised;
            s.visuals.widgets.inactive.weak_bg_fill = panel_alt;
            s.visuals.widgets.inactive.bg_stroke.color = stroke;
            s.visuals.widgets.inactive.fg_stroke.color = fg;
            s.visuals.widgets.hovered.bg_fill = bg_hover(flavor);
            s.visuals.widgets.hovered.bg_stroke.color = bg_hover(flavor);
            s.visuals.widgets.hovered.fg_stroke.color = strong_fg;
            s.visuals.widgets.active.bg_fill = acc;
            s.visuals.widgets.active.bg_stroke.color = acc;
            s.visuals.widgets.active.fg_stroke.color = Color32::WHITE;
            s.visuals.widgets.open.bg_fill = raised;
            s.visuals.window_stroke.color = stroke_soft;
            s.visuals.selection.bg_fill = sel_bg;
            s.visuals.selection.stroke.color = acc;
            s.visuals.override_text_color = Some(fg);
            s.visuals.hyperlink_color = accent_fg(flavor);
            s.visuals.error_fg_color = danger(flavor);
            s.visuals.warn_fg_color = warn(flavor);
        } else {
            let panel       = bg_of(flavor, Layer::L1);
            let _panel_alt  = bg_of(flavor, Layer::L0);
            let raised      = bg_of(flavor, Layer::L2);
            let stroke      = border_strong(flavor);
            let stroke_soft = border_subtle(flavor);
            let fg          = text_body(flavor);
            let strong_fg   = text_primary(flavor);
            let sel_bg      = bg_selected_row(flavor);
            let acc         = accent(flavor);
            s.visuals.panel_fill = panel;
            s.visuals.window_fill = raised;
            s.visuals.extreme_bg_color = raised;
            s.visuals.faint_bg_color = panel;
            s.visuals.code_bg_color = panel;
            s.visuals.widgets.noninteractive.bg_fill = panel;
            s.visuals.widgets.noninteractive.weak_bg_fill = panel;
            s.visuals.widgets.noninteractive.bg_stroke.color = stroke_soft;
            s.visuals.widgets.noninteractive.fg_stroke.color = fg;
            s.visuals.widgets.inactive.bg_fill = raised;
            s.visuals.widgets.inactive.weak_bg_fill = panel;
            s.visuals.widgets.inactive.bg_stroke.color = stroke;
            s.visuals.widgets.inactive.fg_stroke.color = fg;
            s.visuals.widgets.hovered.bg_fill = bg_hover(flavor);
            s.visuals.widgets.hovered.bg_stroke.color = bg_hover(flavor);
            s.visuals.widgets.hovered.fg_stroke.color = strong_fg;
            s.visuals.widgets.active.bg_fill = acc;
            s.visuals.widgets.active.bg_stroke.color = acc;
            s.visuals.widgets.active.fg_stroke.color = Color32::WHITE;
            s.visuals.widgets.open.bg_fill = raised;
            s.visuals.window_stroke.color = stroke_soft;
            s.visuals.selection.bg_fill = sel_bg;
            s.visuals.selection.stroke.color = acc;
            s.visuals.override_text_color = Some(fg);
            s.visuals.hyperlink_color = accent_fg(flavor);
            s.visuals.error_fg_color = danger(flavor);
            s.visuals.warn_fg_color = warn(flavor);
        }
    });

    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(10.0, 6.0);
        s.spacing.button_padding = egui::vec2(12.0, 4.0);
        s.spacing.menu_margin = egui::Margin::symmetric(8.0 as i8, 5.0 as i8);
        s.spacing.indent = 14.0;
        s.spacing.interact_size.y = 24.0;

        let r: egui::CornerRadius = 4.0.into();
        s.visuals.widgets.noninteractive.corner_radius = r;
        s.visuals.widgets.inactive.corner_radius = r;
        s.visuals.widgets.hovered.corner_radius = r;
        s.visuals.widgets.active.corner_radius = r;
        s.visuals.widgets.open.corner_radius = r;
        s.visuals.window_corner_radius = 6.0.into();
        s.visuals.menu_corner_radius = 6.0.into();

        use egui::TextStyle::*;
        s.text_styles.insert(Heading,   egui::FontId::new(15.0, egui::FontFamily::Proportional));
        s.text_styles.insert(Body,      egui::FontId::new(12.5, egui::FontFamily::Proportional));
        s.text_styles.insert(Button,    egui::FontId::new(12.0, egui::FontFamily::Proportional));
        s.text_styles.insert(Monospace, egui::FontId::new(12.5, egui::FontFamily::Monospace));
        s.text_styles.insert(Small,     egui::FontId::new(10.5, egui::FontFamily::Proportional));
    });
}

// --- Semantic color helpers used by app code ---

pub fn accent(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0x1f, 0x6f, 0xeb) } else { rgb(0x25, 0x63, 0xeb) }
}
pub fn accent_fg(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0x58, 0xa6, 0xff) } else { rgb(0x3b, 0x82, 0xf6) }
}
pub fn success(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0x3f, 0xb9, 0x50) } else { rgb(0x15, 0x80, 0x3d) }
}
pub fn warn(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0xf0, 0x88, 0x3e) } else { rgb(0xc2, 0x41, 0x0c) }
}
pub fn danger(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0xf8, 0x51, 0x49) } else { rgb(0xb9, 0x1c, 0x1c) }
}
pub fn alias(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0xd2, 0xa8, 0xff) } else { rgb(0x7c, 0x3a, 0xed) }
}
pub fn border_subtle(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0x21, 0x26, 0x2d) } else { rgb(0xe4, 0xe4, 0xe7) }
}
pub fn border_strong(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0x30, 0x36, 0x3d) } else { rgb(0xd4, 0xd4, 0xd8) }
}
pub fn text_primary(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0xe6, 0xed, 0xf3) } else { rgb(0x09, 0x09, 0x0b) }
}
pub fn text_body(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0xc9, 0xd1, 0xd9) } else { rgb(0x3f, 0x3f, 0x46) }
}
pub fn text_muted(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0x6e, 0x76, 0x81) } else { rgb(0x71, 0x71, 0x7a) }
}
pub fn subtext(flavor: Flavor) -> Color32 { text_muted(flavor) } // 旧调用点回退
pub fn surface(flavor: Flavor) -> Color32 { bg_of(flavor, Layer::L2) } // 旧调用点回退

/// 文本渲染辅助：tiny_caps / crumb 等语义文本样式。
pub mod text {
    use super::{Flavor, text_muted, accent_fg};
    use egui::{Ui, RichText};

    /// 表头 / 分组标题用：10.5px 大写、字距感由空格 + 字色弱化体现。
    pub fn tiny_caps(ui: &mut Ui, flavor: Flavor, s: &str) {
        ui.label(
            RichText::new(s.to_uppercase())
                .size(10.5)
                .color(accent_fg(flavor))
                .strong(),
        );
    }

    /// 面包屑 / 元信息：11px、muted。
    pub fn crumb(ui: &mut Ui, flavor: Flavor, s: &str) {
        ui.label(RichText::new(s).size(11.0).color(text_muted(flavor)));
    }
}
