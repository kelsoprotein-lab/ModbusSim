//! Small reusable UI building blocks: region, card, primary_button, status_pill.
//!
//! Visual defaults: Darcula three-level bg layering, orange accent
//! (#cc7832 primary fill), no default stroke on buttons — hover relies on
//! bg_hover fill instead of borders.

use egui::{Color32, Response, RichText, Ui};

use crate::theme::{self, Flavor, Layer};

fn card_colors(flavor: Flavor) -> (Color32, Color32) {
    // Flat panel. Dark mode = Darcula tool-window fill #3c3f41 on editor
    // #2b2b2b with #515659 stroke (same as IDE "chrome panel" contrast).
    if flavor.is_dark() {
        (
            Color32::from_rgb(60, 63, 65),   // #3c3f41
            Color32::from_rgb(81, 86, 89),   // #515659
        )
    } else {
        (
            Color32::from_rgb(255, 255, 255),
            Color32::from_rgb(208, 208, 208),
        )
    }
}

/// Flat bordered panel. No shadow, 2 px corner radius, 10 px padding — mimics
/// the GroupBox / section divider used in desktop industrial tools.
///
/// Kept for backward compatibility; prefer `region` for new code which uses
/// background-layer differences instead of stroke borders.
pub fn card<R>(ui: &mut Ui, flavor: Flavor, add: impl FnOnce(&mut Ui) -> R) -> R {
    let (fill, stroke_color) = card_colors(flavor);
    egui::Frame::new()
        .fill(fill)
        .corner_radius(2.0)
        .inner_margin(egui::Margin::symmetric(10.0 as i8, 8.0 as i8))
        .stroke(egui::Stroke::new(1.0, stroke_color))
        .show(ui, add)
        .inner
}

/// Flat region: bg fill by layer + inner padding, **no stroke**. Use this
/// to group content without painting a visible border — region boundaries
/// come from the bg-layer delta between neighbors.
pub fn region<R>(
    ui: &mut Ui,
    flavor: Flavor,
    layer: Layer,
    margin: egui::Margin,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    egui::Frame::new()
        .fill(theme::bg_of(flavor, layer))
        .inner_margin(margin)
        .show(ui, add)
        .inner
}

/// Same as `card`, plus a 2 px accent line along the top edge. Used for the
/// current-context header (e.g. "FC04 Input Registers — slave_1").
pub fn accent_card<R>(
    ui: &mut Ui,
    flavor: Flavor,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let accent = crate::theme::accent(flavor);
    let (fill, stroke_color) = card_colors(flavor);
    let resp = egui::Frame::new()
        .fill(fill)
        .corner_radius(2.0)
        .inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 10,
            bottom: 8,
        })
        .stroke(egui::Stroke::new(1.0, stroke_color))
        .show(ui, add);
    // Paint a 2 px accent stripe across the top.
    let rect = resp.response.rect;
    let stripe = egui::Rect::from_min_max(
        rect.left_top(),
        egui::pos2(rect.right(), rect.top() + 2.0),
    );
    ui.painter().rect_filled(stripe, 0.0, accent);
    resp.inner
}

/// Lazily constructed shadcn Theme (dark / light variant derived from Flavor).
/// Theme creation is not free (computes palette tables), so cache it per frame.
/// Overrides shadcn's Neutral base palette with our Darcula orange / industrial
/// blue tokens so buttons and switches pick up our accent color instead of
/// rendering as washed-out gray.
fn shadcn_theme(flavor: Flavor) -> egui_shadcn::Theme {
    use egui_shadcn::tokens::{ColorPalette, ShadcnBaseColor};
    let mut palette = if flavor.is_dark() {
        ColorPalette::shadcn_dark(ShadcnBaseColor::Neutral)
    } else {
        ColorPalette::shadcn_light(ShadcnBaseColor::Neutral)
    };
    if flavor.is_dark() {
        // Darcula orange accent + Layer::L1/L2 background alignment
        palette.primary = Color32::from_rgb(0xcc, 0x78, 0x32);
        palette.primary_foreground = Color32::from_rgb(0x1e, 0x1e, 0x1e);
        palette.destructive = Color32::from_rgb(0xbc, 0x3f, 0x3c);
        palette.destructive_foreground = Color32::WHITE;
        palette.ring = Color32::from_rgb(0xcc, 0x78, 0x32);
        palette.border = Color32::from_rgb(0x51, 0x56, 0x59);
        palette.background = Color32::from_rgb(0x2b, 0x2d, 0x30);
        palette.foreground = Color32::from_rgb(0xd4, 0xd7, 0xdb);
        palette.muted_foreground = Color32::from_rgb(0x9c, 0xa0, 0xa4);
        palette.accent = palette.primary;
        palette.accent_foreground = palette.primary_foreground;
    } else {
        // redisant industrial blue accent
        palette.primary = Color32::from_rgb(0x3b, 0x9a, 0xe8);
        palette.primary_foreground = Color32::WHITE;
        palette.destructive = Color32::from_rgb(0xc8, 0x33, 0x36);
        palette.destructive_foreground = Color32::WHITE;
        palette.ring = Color32::from_rgb(0x3b, 0x9a, 0xe8);
        palette.border = Color32::from_rgb(0xd0, 0xd0, 0xd0);
        palette.background = Color32::from_rgb(0xf5, 0xf5, 0xf5);
        palette.foreground = Color32::from_rgb(0x33, 0x33, 0x33);
        palette.muted_foreground = Color32::from_rgb(0x66, 0x66, 0x66);
        palette.accent = palette.primary;
        palette.accent_foreground = palette.primary_foreground;
    }
    egui_shadcn::Theme::new(palette)
}

/// Primary action button: shadcn Default (Primary) variant.
pub fn primary_button(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response {
    let theme = shadcn_theme(flavor);
    egui_shadcn::button(
        ui,
        &theme,
        text.into(),
        egui_shadcn::tokens::ControlVariant::Primary,
        egui_shadcn::tokens::ControlSize::Md,
        true,
    )
}

/// Secondary (default) button: shadcn Outline variant.
pub fn secondary_button(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response {
    let theme = shadcn_theme(flavor);
    egui_shadcn::button(
        ui,
        &theme,
        text.into(),
        egui_shadcn::tokens::ControlVariant::Outline,
        egui_shadcn::tokens::ControlSize::Md,
        true,
    )
}

/// Danger / destructive button: shadcn Destructive variant.
pub fn danger_button(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response {
    let theme = shadcn_theme(flavor);
    egui_shadcn::button(
        ui,
        &theme,
        text.into(),
        egui_shadcn::tokens::ControlVariant::Destructive,
        egui_shadcn::tokens::ControlSize::Md,
        true,
    )
}

/// Icon-only button: shadcn Ghost variant + small size.
pub fn icon_button(ui: &mut Ui, flavor: Flavor, icon: &str) -> Response {
    let theme = shadcn_theme(flavor);
    egui_shadcn::button(
        ui,
        &theme,
        icon.to_string(),
        egui_shadcn::tokens::ControlVariant::Ghost,
        egui_shadcn::tokens::ControlSize::Sm,
        true,
    )
}

/// Pill-shaped status badge. Small rounded label with colored text and a very
/// faint tinted background — never loud.
pub fn status_pill(ui: &mut Ui, text: impl Into<String>, color: Color32) {
    let bg = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 36);
    egui::Frame::new()
        .fill(bg)
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(6.0 as i8, 1.0 as i8))
        .show(ui, |ui| {
            ui.label(RichText::new(text.into()).color(color).size(11.5));
        });
}

/// Section heading with optional leading icon. Uses Heading text style.
pub fn section_heading(ui: &mut Ui, icon: &str, title: &str) {
    let text = if icon.is_empty() {
        title.to_string()
    } else {
        format!("{}  {}", icon, title)
    };
    ui.label(RichText::new(text).heading());
}

/// Small subtext / caption color label (11px, muted).
pub fn caption(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) {
    ui.label(
        RichText::new(text.into())
            .color(crate::theme::subtext(flavor))
            .size(11.0),
    );
}

/// Radix-style shadcn Switch. Replaces the earlier self-drawn 40×18 toggle;
/// the shadcn widget owns track sizing, slide animation, hover/focus ring.
/// Returns `Response` — caller inspects `.clicked()` to detect a flip.
pub fn toggle_switch(ui: &mut Ui, flavor: Flavor, value: &mut bool) -> Response {
    let theme = shadcn_theme(flavor);
    egui_shadcn::switch(
        ui,
        &theme,
        value,
        "",
        egui_shadcn::tokens::ControlVariant::Primary,
        egui_shadcn::tokens::ControlSize::Md,
        true,
    )
}
