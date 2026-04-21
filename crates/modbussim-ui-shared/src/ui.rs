//! Small reusable UI building blocks: region, card, primary_button, status_pill,
//! panel_header, link_action.
//!
//! Visual defaults: cold-blue palette (#0d1117 surface), green primary action
//! (#3fb950 "+ 批量添加"), blue accent_fg (#58a6ff links/hover). No hardcoded
//! RGB — all colors delegated to `theme::` token functions.

use egui::{Color32, Response, RichText, Ui};

use crate::theme::{self, Flavor, Layer};

fn card_colors(flavor: Flavor) -> (Color32, Color32) {
    // Industrial HMI: raised L2 background + subtle border. Same look in both
    // flavors — token routing handles dark/light.
    (
        theme::bg_of(flavor, Layer::L2),
        theme::border_subtle(flavor),
    )
}

/// Flat panel with raised bg + subtle border. Used for grouped content.
pub fn card<R>(ui: &mut Ui, flavor: Flavor, add: impl FnOnce(&mut Ui) -> R) -> R {
    let (fill, stroke_color) = card_colors(flavor);
    egui::Frame::new()
        .fill(fill)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(14.0 as i8, 12.0 as i8))
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
pub fn accent_card<R>(ui: &mut Ui, flavor: Flavor, add: impl FnOnce(&mut Ui) -> R) -> R {
    let accent = crate::theme::accent(flavor);
    let (fill, stroke_color) = card_colors(flavor);
    let resp = egui::Frame::new()
        .fill(fill)
        .corner_radius(4.0)
        .inner_margin(egui::Margin {
            left: 14,
            right: 14,
            top: 12,
            bottom: 10,
        })
        .stroke(egui::Stroke::new(1.0, stroke_color))
        .show(ui, add);
    // Paint a 2 px accent stripe across the top.
    let rect = resp.response.rect;
    let stripe =
        egui::Rect::from_min_max(rect.left_top(), egui::pos2(rect.right(), rect.top() + 2.0));
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
        // Industrial HMI: cool blue primary + green action accent + L1/L2 bg
        palette.primary = Color32::from_rgb(0x3f, 0xb9, 0x50); // 主操作绿（"+ 批量添加"）
        palette.primary_foreground = Color32::WHITE;
        palette.destructive = Color32::from_rgb(0xf8, 0x51, 0x49);
        palette.destructive_foreground = Color32::WHITE;
        palette.ring = Color32::from_rgb(0x1f, 0x6f, 0xeb); // focus 蓝
        palette.border = Color32::from_rgb(0x30, 0x36, 0x3d);
        palette.background = Color32::from_rgb(0x0d, 0x11, 0x17);
        palette.foreground = Color32::from_rgb(0xc9, 0xd1, 0xd9);
        palette.muted_foreground = Color32::from_rgb(0x6e, 0x76, 0x81);
        palette.accent = Color32::from_rgb(0x1f, 0x6f, 0xeb); // 蓝 accent（链接/选中）
        palette.accent_foreground = Color32::WHITE;
    } else {
        palette.primary = Color32::from_rgb(0x15, 0x80, 0x3d); // 浅色主操作深绿
        palette.primary_foreground = Color32::WHITE;
        palette.destructive = Color32::from_rgb(0xb9, 0x1c, 0x1c);
        palette.destructive_foreground = Color32::WHITE;
        palette.ring = Color32::from_rgb(0x25, 0x63, 0xeb);
        palette.border = Color32::from_rgb(0xd4, 0xd4, 0xd8);
        palette.background = Color32::from_rgb(0xfa, 0xfa, 0xfa);
        palette.foreground = Color32::from_rgb(0x3f, 0x3f, 0x46);
        palette.muted_foreground = Color32::from_rgb(0x71, 0x71, 0x7a);
        palette.accent = Color32::from_rgb(0x25, 0x63, 0xeb);
        palette.accent_foreground = Color32::WHITE;
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

/// Compact secondary button: shadcn Outline variant + Sm size.
/// Use for tertiary actions where Md feels visually heavy ("+ 新建" /
/// "+ 批量添加" / "导出 CSV"), but a borderless `link_action` lacks visual weight.
pub fn secondary_button_sm(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response {
    let theme = shadcn_theme(flavor);
    egui_shadcn::button(
        ui,
        &theme,
        text.into(),
        egui_shadcn::tokens::ControlVariant::Outline,
        egui_shadcn::tokens::ControlSize::Sm,
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
/// Wrapped in a fixed 48×24 centered sub-Ui so that inside an `exact`/`initial`
/// table column the switch stays at its native ~35×20 track size and doesn't
/// inherit the cell's full width (which would leave it left-aligned with a
/// cavernous click area).
pub fn toggle_switch(ui: &mut Ui, flavor: Flavor, value: &mut bool) -> Response {
    let theme = shadcn_theme(flavor);
    let desired = egui::vec2(48.0, 24.0);
    ui.allocate_ui_with_layout(
        desired,
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
        |ui| {
            egui_shadcn::switch(
                ui,
                &theme,
                value,
                "",
                egui_shadcn::tokens::ControlVariant::Primary,
                egui_shadcn::tokens::ControlSize::Md,
                true,
            )
        },
    )
    .inner
}

/// Panel header: heading title + optional muted breadcrumb on a second line.
/// Used by Slave's CentralPanel header ("FC03 保持寄存器" / "slave_1 · 20001 行").
pub fn panel_header(ui: &mut Ui, flavor: Flavor, title: &str, crumb: Option<&str>) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(title)
                .heading()
                .color(theme::text_primary(flavor)),
        );
        if let Some(c) = crumb {
            theme::text::crumb(ui, flavor, c);
        }
    });
}

/// Borderless text-action: "停止" / "删除连接" / "关闭". Hovers to accent_fg
/// (or `danger(flavor)` when `danger=true`). Returns the click `Response`.
pub fn link_action(ui: &mut Ui, flavor: Flavor, label: &str, danger: bool) -> Response {
    let base = theme::text_muted(flavor);
    let resp = ui.add(
        egui::Label::new(RichText::new(label).color(base).size(11.5)).sense(egui::Sense::click()),
    );
    if resp.hovered() {
        let hover = if danger {
            theme::danger(flavor)
        } else {
            theme::accent_fg(flavor)
        };
        ui.painter().text(
            resp.rect.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(11.5),
            hover,
        );
    }
    resp
}
