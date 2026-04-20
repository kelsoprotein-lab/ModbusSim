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
        // Industrial apps in the Modbus space typically ship light by default;
        // redisant MSE, Modscan32, KEPServerEX all use white bases.
        Flavor::Latte
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
pub const VSCODE_DARK: catppuccin_egui::Theme = catppuccin_egui::Theme {
    // Accents (VS Code semantic tokens)
    rosewater: rgb(220, 170, 124),
    flamingo: rgb(206, 145, 120),
    pink: rgb(197, 134, 192),
    mauve: rgb(155, 121, 186),
    red: rgb(244, 135, 113),
    maroon: rgb(161, 38, 13),
    peach: rgb(206, 145, 120),
    yellow: rgb(220, 220, 170),
    green: rgb(78, 201, 176),
    teal: rgb(78, 201, 176),
    sky: rgb(86, 156, 214),
    sapphire: rgb(78, 166, 217),
    blue: rgb(59, 154, 232),        // #3b9ae8 — same industrial blue as light
    lavender: rgb(197, 134, 192),
    // Foreground
    text: rgb(220, 220, 220),      // #dcdcdc
    subtext1: rgb(170, 170, 170),
    subtext0: rgb(133, 133, 133),   // #858585
    // Borders / strokes (slightly cooler)
    overlay2: rgb(74, 78, 88),
    overlay1: rgb(56, 60, 70),
    overlay0: rgb(42, 46, 54),       // #2a2e36 — card stroke
    // Surfaces — cards sit one step above base
    surface2: rgb(55, 59, 70),
    surface1: rgb(45, 49, 60),
    surface0: rgb(37, 41, 50),       // #252932 — card fill
    // Backgrounds — VS Code standard, neutral
    base: rgb(30, 30, 30),           // #1e1e1e (central panel)
    mantle: rgb(37, 37, 38),         // #252526 (side panels)
    crust: rgb(51, 51, 51),          // #333333 (darkest)
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
