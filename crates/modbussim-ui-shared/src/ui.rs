//! Small reusable UI building blocks: card, primary_button, status_pill.
//!
//! Visual defaults follow VS Code: compact padding, subtle rounding, solid
//! primary button (#0e639c fill + white text), transparent secondary.

use egui::{Color32, Response, RichText, Ui};

use crate::theme::Flavor;

/// Render content inside a rounded "card" frame using the flavor's surface0
/// color. Thin 1px border, 10px padding, 4px radius.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let frame = egui::Frame::none()
        .fill(ui.visuals().extreme_bg_color) // already surface0-ish on dark themes
        .rounding(4.0)
        .inner_margin(egui::Margin::symmetric(10.0, 8.0))
        .stroke(egui::Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ));
    frame.show(ui, add).inner
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
