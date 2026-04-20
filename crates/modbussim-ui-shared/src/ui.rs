//! Small reusable UI building blocks: card, primary_button, status_pill.
//!
//! Visual defaults follow VS Code: compact padding, subtle rounding, solid
//! primary button (#0e639c fill + white text), transparent secondary.

use egui::{Color32, Response, RichText, Ui};

use crate::theme::Flavor;

fn card_colors(flavor: Flavor) -> (Color32, Color32) {
    // Cool steel-blue dark panels — clearly distinct from the near-black base
    // so cards visibly float, and with a cold-neutral hue that reads as
    // "industrial instrumentation" rather than toy-like purple.
    if flavor.is_dark() {
        (
            Color32::from_rgb(30, 40, 56),   // card fill  (#1e2838)
            Color32::from_rgb(72, 88, 112),  // stroke     (#485870)
        )
    } else {
        (
            Color32::from_rgb(252, 253, 255),
            Color32::from_rgb(210, 215, 224),
        )
    }
}

/// Rounded "card" frame that visually lifts its content above the panel
/// background. Hardcoded fill + visible stroke + soft shadow + 8px corners.
pub fn card<R>(ui: &mut Ui, flavor: Flavor, add: impl FnOnce(&mut Ui) -> R) -> R {
    let (fill, stroke_color) = card_colors(flavor);
    let shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 2.0),
        blur: 8.0,
        spread: 0.0,
        color: Color32::from_black_alpha(90),
    };
    egui::Frame::none()
        .fill(fill)
        .rounding(8.0)
        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
        .stroke(egui::Stroke::new(1.0, stroke_color))
        .shadow(shadow)
        .show(ui, add)
        .inner
}

/// Card variant with a 3px accent stripe along the left edge.
pub fn accent_card<R>(
    ui: &mut Ui,
    flavor: Flavor,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let accent = crate::theme::accent(flavor);
    let (fill, stroke_color) = card_colors(flavor);
    let shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 2.0),
        blur: 8.0,
        spread: 0.0,
        color: Color32::from_black_alpha(90),
    };
    let resp = egui::Frame::none()
        .fill(fill)
        .rounding(8.0)
        .inner_margin(egui::Margin {
            left: 16.0,
            right: 14.0,
            top: 12.0,
            bottom: 12.0,
        })
        .stroke(egui::Stroke::new(1.0, stroke_color))
        .shadow(shadow)
        .show(ui, add);
    let rect = resp.response.rect;
    let stripe = egui::Rect::from_min_max(
        rect.left_top(),
        egui::pos2(rect.left() + 3.0, rect.bottom()),
    );
    ui.painter().rect_filled(stripe, 2.0, accent);
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
