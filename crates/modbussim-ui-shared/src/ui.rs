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

/// Primary action button: solid accent fill, white text, no stroke.
pub fn primary_button(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response {
    let accent = theme::accent(flavor);
    let btn = egui::Button::new(
        RichText::new(text.into())
            .color(Color32::WHITE)
            .size(13.0),
    )
    .fill(accent)
    .stroke(egui::Stroke::NONE)
    .corner_radius(2.0)
    .min_size(egui::vec2(0.0, 24.0));
    ui.add(btn)
}

/// Secondary (default) button: subtle L2 fill (not transparent — a fully
/// transparent button loses visual affordance against the L1 panel bg).
/// egui's global widgets.hovered.bg_fill takes over on hover.
pub fn secondary_button(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response {
    let btn = egui::Button::new(RichText::new(text.into()).size(13.0))
        .fill(theme::bg_of(flavor, Layer::L2))
        .stroke(egui::Stroke::NONE)
        .corner_radius(2.0)
        .min_size(egui::vec2(0.0, 24.0));
    ui.add(btn)
}

/// Danger / destructive button: slightly muted red fill, no stroke.
pub fn danger_button(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response {
    let red = theme::danger(flavor);
    let btn = egui::Button::new(
        RichText::new(text.into())
            .color(Color32::WHITE)
            .size(13.0),
    )
    .fill(red.linear_multiply(0.85))
    .stroke(egui::Stroke::NONE)
    .corner_radius(2.0)
    .min_size(egui::vec2(0.0, 24.0));
    ui.add(btn)
}

/// Icon-only button: 24×24, transparent default, hover uses egui global bg.
pub fn icon_button(ui: &mut Ui, _flavor: Flavor, icon: &str) -> Response {
    let btn = egui::Button::new(RichText::new(icon).size(14.0))
        .fill(Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .corner_radius(2.0)
        .min_size(egui::vec2(24.0, 24.0));
    ui.add(btn)
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

/// iOS-style toggle switch: 40×18 rounded track + 14 px white knob (16 px on hover).
/// Full-rect click handling — users don't have to hit the knob precisely.
/// Returns a `Response` whose `.clicked()` is true on the frame the toggle
/// flipped (value is mutated before returning).
pub fn toggle_switch(ui: &mut Ui, flavor: Flavor, value: &mut bool) -> Response {
    let desired = egui::vec2(40.0, 18.0);
    let (rect, mut resp) = ui.allocate_exact_size(desired, egui::Sense::click());
    if resp.clicked() {
        *value = !*value;
        resp.mark_changed();
    }
    let track_color = if *value {
        theme::success(flavor)
    } else {
        theme::bg_hover(flavor)
    };
    ui.painter().rect_filled(rect, 9.0, track_color);
    let knob_r = if resp.hovered() { 8.0 } else { 7.0 };
    let cx = if *value {
        rect.right() - 9.0
    } else {
        rect.left() + 9.0
    };
    let center = egui::pos2(cx, rect.center().y);
    ui.painter()
        .circle_filled(center, knob_r, Color32::from_rgb(235, 235, 235));
    resp
}
