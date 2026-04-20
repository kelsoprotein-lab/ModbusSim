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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flavor {
    /// Dark+ — default dark theme (bg #1e1e1e)
    Mocha,
    /// Dark+ alias (kept for serde compat with older save files)
    Macchiato,
    /// Dark+ alias
    Frappe,
    /// Light+ — light mode (bg #f3f3f3)
    Latte,
}

impl Default for Flavor {
    fn default() -> Self {
        // Darcula-style warm gray is the default dark look that Modbus users
        // consistently land on (JetBrains IDEs, Android Studio — decades of
        // industrial-desktop precedent).
        Flavor::Mocha
    }
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
    text: rgb(169, 183, 198),      // #a9b7c6 — Darcula default
    subtext1: rgb(152, 152, 152),
    subtext0: rgb(128, 128, 128),  // #808080 — comment
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
            // Darcula warm-gray + orange accent
            let panel = Color32::from_rgb(43, 43, 43);       // #2b2b2b (editor)
            let panel_alt = Color32::from_rgb(60, 63, 65);   // #3c3f41 (tool window)
            let input_bg = Color32::from_rgb(69, 73, 74);    // #45494a
            let stroke = Color32::from_rgb(81, 86, 89);      // #515659
            let fg = Color32::from_rgb(187, 187, 187);       // #bbbbbb
            let sel_bg = Color32::from_rgb(75, 110, 175);    // #4b6eaf — Darcula selection
            let accent = Color32::from_rgb(204, 120, 50);    // #cc7832 orange
            s.visuals.panel_fill = panel;
            s.visuals.window_fill = panel_alt;
            s.visuals.extreme_bg_color = Color32::from_rgb(37, 37, 37); // #252525 input-ish bg
            s.visuals.faint_bg_color = Color32::from_rgb(49, 51, 53);    // #313335 — striped row
            s.visuals.code_bg_color = Color32::from_rgb(49, 51, 53);
            s.visuals.widgets.noninteractive.bg_fill = panel_alt;
            s.visuals.widgets.noninteractive.weak_bg_fill = panel;
            s.visuals.widgets.noninteractive.bg_stroke.color = stroke;
            s.visuals.widgets.noninteractive.fg_stroke.color = fg;
            s.visuals.widgets.inactive.bg_fill = input_bg;
            s.visuals.widgets.inactive.weak_bg_fill = panel_alt;
            s.visuals.widgets.inactive.bg_stroke.color = stroke;
            s.visuals.widgets.inactive.fg_stroke.color = fg;
            s.visuals.widgets.hovered.bg_fill = Color32::from_rgb(91, 95, 97);
            s.visuals.widgets.hovered.bg_stroke.color = Color32::from_rgb(112, 116, 119);
            s.visuals.widgets.hovered.fg_stroke.color = fg;
            s.visuals.widgets.active.bg_fill = accent;
            s.visuals.widgets.active.bg_stroke.color = accent;
            s.visuals.widgets.active.fg_stroke.color = Color32::from_rgb(30, 30, 30);
            s.visuals.widgets.open.bg_fill = input_bg;
            s.visuals.window_stroke.color = stroke;
            s.visuals.selection.bg_fill = sel_bg;
            s.visuals.selection.stroke.color = accent;
            s.visuals.override_text_color = Some(fg);
            s.visuals.hyperlink_color = Color32::from_rgb(104, 151, 187); // darcula ctor blue
            s.visuals.error_fg_color = Color32::from_rgb(255, 100, 100);
            s.visuals.warn_fg_color = Color32::from_rgb(255, 198, 109);
        } else {
            let panel = Color32::from_rgb(245, 245, 245);    // #f5f5f5
            let white = Color32::from_rgb(255, 255, 255);    // #ffffff
            let stroke = Color32::from_rgb(208, 208, 208);   // #d0d0d0
            let stroke_strong = Color32::from_rgb(190, 190, 190);
            let fg = Color32::from_rgb(51, 51, 51);          // #333333
            let sel_bg = Color32::from_rgb(201, 218, 248);   // #c9daf8 row highlight
            let accent = Color32::from_rgb(59, 154, 232);    // #3b9ae8
            s.visuals.panel_fill = panel;
            s.visuals.window_fill = white;
            s.visuals.extreme_bg_color = white;
            s.visuals.faint_bg_color = Color32::from_rgb(248, 248, 248);
            s.visuals.code_bg_color = Color32::from_rgb(240, 240, 240);
            s.visuals.widgets.noninteractive.bg_fill = panel;
            s.visuals.widgets.noninteractive.weak_bg_fill = panel;
            s.visuals.widgets.noninteractive.bg_stroke.color = stroke;
            s.visuals.widgets.noninteractive.fg_stroke.color = fg;
            s.visuals.widgets.inactive.bg_fill = Color32::from_rgb(240, 240, 240);
            s.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(245, 245, 245);
            s.visuals.widgets.inactive.bg_stroke.color = stroke;
            s.visuals.widgets.inactive.fg_stroke.color = fg;
            s.visuals.widgets.hovered.bg_fill = Color32::from_rgb(230, 230, 230);
            s.visuals.widgets.hovered.bg_stroke.color = stroke_strong;
            s.visuals.widgets.hovered.fg_stroke.color = fg;
            s.visuals.widgets.active.bg_fill = accent;
            s.visuals.widgets.active.bg_stroke.color = accent;
            s.visuals.widgets.active.fg_stroke.color = Color32::WHITE;
            s.visuals.widgets.open.bg_fill = Color32::from_rgb(230, 230, 230);
            s.visuals.window_stroke.color = stroke;
            s.visuals.selection.bg_fill = sel_bg;
            s.visuals.selection.stroke.color = accent;
            s.visuals.override_text_color = Some(fg);
            s.visuals.hyperlink_color = accent;
            s.visuals.error_fg_color = Color32::from_rgb(200, 51, 54);
            s.visuals.warn_fg_color = Color32::from_rgb(175, 82, 0);
        }
    });

    ctx.style_mut(|s| {
        // Tight spacing — VS Code-like density
        s.spacing.item_spacing = egui::vec2(8.0, 4.0);
        s.spacing.button_padding = egui::vec2(9.0, 3.0);
        s.spacing.menu_margin = egui::Margin::symmetric(6.0, 4.0);
        s.spacing.indent = 14.0;
        s.spacing.interact_size.y = 22.0;

        // Slight rounding — VS Code uses mostly 2-4px, not 8+
        let r: egui::Rounding = 3.0.into();
        s.visuals.widgets.noninteractive.rounding = r;
        s.visuals.widgets.inactive.rounding = r;
        s.visuals.widgets.hovered.rounding = r;
        s.visuals.widgets.active.rounding = r;
        s.visuals.widgets.open.rounding = r;
        s.visuals.window_rounding = 4.0.into();
        s.visuals.menu_rounding = 4.0.into();

        // Type scale — smaller than our previous version, closer to VS Code
        use egui::TextStyle::*;
        s.text_styles.insert(
            Heading,
            egui::FontId::new(15.0, egui::FontFamily::Proportional),
        );
        s.text_styles.insert(
            Body,
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
        );
        s.text_styles.insert(
            Button,
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
        );
        s.text_styles.insert(
            Monospace,
            egui::FontId::new(12.5, egui::FontFamily::Monospace),
        );
        s.text_styles.insert(
            Small,
            egui::FontId::new(11.0, egui::FontFamily::Proportional),
        );
    });
}

// --- Semantic color helpers used by app code ---

pub fn accent(flavor: Flavor) -> Color32 {
    flavor.palette().blue
}

pub fn success(flavor: Flavor) -> Color32 {
    flavor.palette().green
}

pub fn danger(flavor: Flavor) -> Color32 {
    flavor.palette().red
}

pub fn subtext(flavor: Flavor) -> Color32 {
    flavor.palette().subtext0
}

pub fn surface(flavor: Flavor) -> Color32 {
    flavor.palette().surface0
}
