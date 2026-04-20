//! Small reusable UI building blocks: card, primary_button, status_pill.
//!
//! Visual defaults follow VS Code: compact padding, subtle rounding, solid
//! primary button (#0e639c fill + white text), transparent secondary.

use egui::{Color32, Response, RichText, Ui};

use crate::theme::Flavor;

fn card_colors(flavor: Flavor) -> (Color32, Color32) {
    // Industrial flat-panel: card fill is the panel base color (white in light
    // mode, near-black in dark), and a crisp 1 px stroke does the dividing —
    // no shadow, no rounding, no "float". Matches the redisant / Modscan look.
    if flavor.is_dark() {
        (
            Color32::from_rgb(37, 37, 38),   // #252526 — slightly above base
            Color32::from_rgb(80, 80, 82),   // #505052
        )
    } else {
        (
            Color32::from_rgb(255, 255, 255), // #ffffff — card is as white as base
            Color32::from_rgb(208, 208, 208), // #d0d0d0 — subtle gray stroke
        )
    }
}

/// Flat bordered panel. No shadow, 2 px corner radius, 10 px padding — mimics
/// the GroupBox / section divider used in desktop industrial tools.
pub fn card<R>(ui: &mut Ui, flavor: Flavor, add: impl FnOnce(&mut Ui) -> R) -> R {
    let (fill, stroke_color) = card_colors(flavor);
    egui::Frame::none()
        .fill(fill)
        .rounding(2.0)
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .stroke(egui::Stroke::new(1.0, stroke_color))
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
    let resp = egui::Frame::none()
        .fill(fill)
        .rounding(2.0)
        .inner_margin(egui::Margin {
            left: 10.0,
            right: 10.0,
            top: 10.0,
            bottom: 8.0,
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

/// Primary action button: solid accent fill, white text, tight padding.
pub fn primary_button(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response {
    let accent = crate::theme::accent(flavor);
    let btn = egui::Button::new(
        RichText::new(text.into())
            .color(Color32::WHITE)
            .size(13.0),
    )
    .fill(accent)
    .rounding(3.0)
    .min_size(egui::vec2(0.0, 24.0));
    ui.add(btn)
}

/// Secondary (default) button: no fill, regular text color, subtle border.
pub fn secondary_button(ui: &mut Ui, _flavor: Flavor, text: impl Into<String>) -> Response {
    let btn = egui::Button::new(RichText::new(text.into()).size(13.0))
        .rounding(3.0)
        .min_size(egui::vec2(0.0, 24.0));
    ui.add(btn)
}

/// Danger / destructive button: slightly muted red fill.
pub fn danger_button(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response {
    let red = crate::theme::danger(flavor);
    let btn = egui::Button::new(
        RichText::new(text.into())
            .color(Color32::WHITE)
            .size(13.0),
    )
    .fill(red.linear_multiply(0.85))
    .rounding(3.0)
    .min_size(egui::vec2(0.0, 24.0));
    ui.add(btn)
}

/// Pill-shaped status badge. Small rounded label with colored text and a very
/// faint tinted background — never loud.
pub fn status_pill(ui: &mut Ui, text: impl Into<String>, color: Color32) {
    let bg = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 36);
    egui::Frame::none()
        .fill(bg)
        .rounding(3.0)
        .inner_margin(egui::Margin::symmetric(6.0, 1.0))
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
