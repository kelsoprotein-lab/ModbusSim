//! Welcome-screen Hero: 三色 dancing-strings 动画，灵感来自 egui demo
//! `dancing_strings.rs`。调用方把活跃度归一化后通过 `HeroPulseFeed` 传入，
//! 模块负责节流重绘 + 绘制 heading / caption / canvas 整体布局。

use egui::{emath, epaint, epaint::PathStroke, pos2, vec2, Color32, Pos2, Rect};

use crate::theme::{self, Flavor};

/// 调用方每帧构造并传入的轻量入参。
#[derive(Debug, Clone, Copy)]
pub struct HeroPulseFeed {
    /// 振幅乘子，0.0..=1.0。0 只保留底噪，1 接近 demo 原版幅度。
    pub amp: f32,
    /// 预留字段（本轮恒 false）：true 时三弦 lerp 到 danger。
    pub has_error: bool,
    /// true 时跳过画布，仅渲染 heading + caption。
    pub disabled: bool,
}

impl Default for HeroPulseFeed {
    fn default() -> Self {
        Self {
            amp: 0.0,
            has_error: false,
            disabled: false,
        }
    }
}

/// 空状态欢迎屏：大标题 + 说明文字 + 三色弦画布。
///
/// `icon_prefix` 直接 format 进 heading，方便调用方塞 Phosphor 图标字符。
pub fn show_welcome_hero(
    ui: &mut egui::Ui,
    flavor: Flavor,
    icon_prefix: &str,
    title: &str,
    caption: &str,
    feed: HeroPulseFeed,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.heading(format!("{}  {}", icon_prefix, title));
        crate::ui::caption(ui, flavor, caption);
        ui.add_space(28.0);

        if feed.disabled {
            return;
        }

        let max_w = ui.available_width().min(560.0);
        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            ui.set_min_width(max_w);
            let time = ui.input(|i| i.time);
            let desired = vec2(max_w, max_w * 0.22);
            let (_id, rect) = ui.allocate_space(desired);
            let to_screen =
                emath::RectTransform::from_to(Rect::from_x_y_ranges(0.0..=1.0, -1.0..=1.0), rect);

            let gain = 0.15 + 0.85 * feed.amp; // 底噪 15%，满载 100%
            let modes: [(u32, Color32); 3] = [
                (2, theme::accent(flavor)),
                (3, theme::success(flavor)),
                (5, theme::warn(flavor)),
            ];
            let mut shapes = Vec::with_capacity(modes.len());
            for &(mode, color) in &modes {
                let modef = mode as f64;
                let n: usize = 120;
                let speed: f64 = 1.5;
                let points: Vec<Pos2> = (0..=n)
                    .map(|i| {
                        let t = i as f64 / n as f64;
                        let base = (time * speed * modef).sin() / modef;
                        let y =
                            gain as f64 * base * (t * std::f64::consts::TAU / 2.0 * modef).sin();
                        to_screen * pos2(t as f32, y as f32)
                    })
                    .collect();
                let thickness = 8.0 / mode as f32;
                let final_color = if feed.has_error {
                    lerp_color(color, theme::danger(flavor), 0.6)
                } else {
                    color
                };
                shapes.push(epaint::Shape::line(
                    points,
                    PathStroke::new(thickness, final_color),
                ));
            }
            ui.painter().extend(shapes);
        });

        // 节流重绘：失焦 100ms、活跃 60fps、空闲 20fps。
        let focused = ui.ctx().input(|i| i.focused);
        let interval_ms = if !focused {
            100
        } else if feed.amp >= 0.3 {
            16
        } else {
            50
        };
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(interval_ms));
    });
}

/// 逐通道线性插值，`t=0` 返回 `a`，`t=1` 返回 `b`。
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
        lerp(a.a(), b.a()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_color_endpoints() {
        let a = Color32::from_rgb(10, 20, 30);
        let b = Color32::from_rgb(200, 210, 220);
        assert_eq!(
            lerp_color(a, b, 0.0),
            Color32::from_rgba_unmultiplied(10, 20, 30, 255)
        );
        assert_eq!(
            lerp_color(a, b, 1.0),
            Color32::from_rgba_unmultiplied(200, 210, 220, 255)
        );
    }

    #[test]
    fn lerp_color_midpoint() {
        let a = Color32::from_rgb(0, 0, 0);
        let b = Color32::from_rgb(200, 200, 200);
        let mid = lerp_color(a, b, 0.5);
        assert_eq!(mid.r(), 100);
        assert_eq!(mid.g(), 100);
        assert_eq!(mid.b(), 100);
    }

    #[test]
    fn feed_default_is_idle() {
        let f = HeroPulseFeed::default();
        assert_eq!(f.amp, 0.0);
        assert!(!f.has_error);
        assert!(!f.disabled);
    }
}
