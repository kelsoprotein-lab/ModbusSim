# Slave 端空状态 Hero · 三色 Dancing Strings 动画 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `app.rs` 里的本地 `paint_dancing_strings` 搬进 `modbussim-ui-shared/hero_anim`，加节流重绘，并用所有 connection `LogCollector` 最近 1s TX/RX 条数驱动振幅，让空状态 Hero 从"固定正弦"升级为"通信繁忙时跃动"。

**Architecture:** 在 `modbussim-core::LogCollector` 加非阻塞按时间窗计数 API；在 `modbussim-ui-shared/hero_anim` 新建 `show_welcome_hero` + `HeroPulseFeed`；在 `SlaveApp` 加 `HeroPulseState` 每帧聚合所有连接的计数生成 `feed`。视觉上沿用现有 mode={2,3,5} × 三色 × 120 点公式，乘入 `gain = 0.15 + 0.85 * amp`。

**Tech Stack:** Rust 2021 · egui · chrono · tokio::sync::RwLock（try_read）· crossbeam-channel

---

## Task 1: `LogCollector::try_count_within` 非阻塞时间窗计数

**Files:**
- Modify: `crates/modbussim-core/src/log_collector.rs`
- Test: `crates/modbussim-core/src/log_collector.rs`（同文件内 `#[cfg(test)]`）

- [ ] **Step 1.1: 在测试模块里追加一个会失败的单测**

打开 `crates/modbussim-core/src/log_collector.rs`，在末尾 `mod tests { ... }` 块内部（`fn test_get_recent` 之后）追加：

```rust
    #[tokio::test]
    async fn test_try_count_within_recent_only() {
        let collector = LogCollector::new();
        // 10s 前的老记录
        let mut old = LogEntry::new(Direction::Rx, FunctionCode::ReadHoldingRegisters, "old");
        old.timestamp = chrono::Utc::now() - chrono::Duration::seconds(10);
        collector.add(old).await;
        // 3 条新记录
        for i in 0..3 {
            collector
                .add(LogEntry::new(
                    Direction::Rx,
                    FunctionCode::ReadHoldingRegisters,
                    format!("new {}", i),
                ))
                .await;
        }
        let count = collector.try_count_within(std::time::Duration::from_secs(1));
        assert_eq!(count, Some(3));
    }

    #[tokio::test]
    async fn test_try_count_within_empty() {
        let collector = LogCollector::new();
        assert_eq!(
            collector.try_count_within(std::time::Duration::from_secs(1)),
            Some(0),
        );
    }
```

- [ ] **Step 1.2: 运行测试确认 FAIL**

Run: `cargo test -p modbussim-core log_collector::tests::test_try_count_within -- --nocapture`
Expected: 两个 test 都因 `no method named 'try_count_within'` 编译失败。

- [ ] **Step 1.3: 实现 `try_count_within`**

在 `impl LogCollector` 的 `try_get_all` 方法之后（文件内约 L73 之后）插入：

```rust
    /// Non-blocking count of entries whose timestamp is within the last `window`.
    /// Returns `None` if a writer holds the lock.
    ///
    /// Uses the fact that entries are pushed in time order: scans from the tail
    /// and stops at the first entry older than the cutoff. O(k) where k is the
    /// number of recent entries (typically small even if the buffer is large).
    pub fn try_count_within(&self, window: std::time::Duration) -> Option<usize> {
        let chrono_window = chrono::Duration::from_std(window).ok()?;
        let cutoff = chrono::Utc::now() - chrono_window;
        self.entries.try_read().ok().map(|guard| {
            guard
                .iter()
                .rev()
                .take_while(|e| e.timestamp >= cutoff)
                .count()
        })
    }
```

- [ ] **Step 1.4: 运行测试确认 PASS**

Run: `cargo test -p modbussim-core log_collector::tests::test_try_count_within -- --nocapture`
Expected: 两个 test 都 PASS。

- [ ] **Step 1.5: 全量回归**

Run: `cargo test -p modbussim-core`
Expected: 所有 test 通过（原有测试不受影响）。

- [ ] **Step 1.6: Commit**

```bash
git add crates/modbussim-core/src/log_collector.rs
git commit -m "$(cat <<'EOF'
feat(core): LogCollector::try_count_within 非阻塞时间窗计数

为了让 UI Hero 动画按通信活跃度驱动振幅，新增一个 O(k) 非阻塞
API，利用 entries 按写入时间有序的事实从尾部反扫到 cutoff。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: 新建 `ui-shared::hero_anim` 模块（含 `HeroPulseFeed`, `show_welcome_hero`, `lerp_color`）

**Files:**
- Create: `crates/modbussim-ui-shared/src/hero_anim.rs`
- Modify: `crates/modbussim-ui-shared/src/lib.rs:13`

- [ ] **Step 2.1: 创建 `hero_anim.rs` 带完整实现**

写入 `crates/modbussim-ui-shared/src/hero_anim.rs`：

```rust
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
            let to_screen = emath::RectTransform::from_to(
                Rect::from_x_y_ranges(0.0..=1.0, -1.0..=1.0),
                rect,
            );

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
                        let y = gain as f64
                            * base
                            * (t * std::f64::consts::TAU / 2.0 * modef).sin();
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
        assert_eq!(lerp_color(a, b, 0.0), Color32::from_rgba_unmultiplied(10, 20, 30, 255));
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
```

- [ ] **Step 2.2: 在 `lib.rs` 注册模块**

打开 `crates/modbussim-ui-shared/src/lib.rs`，把：

```rust
pub mod fonts;
pub mod format;
pub mod log_panel;
pub mod project;
pub mod theme;
pub mod ui;
pub mod value_panel;
```

改为：

```rust
pub mod fonts;
pub mod format;
pub mod hero_anim;
pub mod log_panel;
pub mod project;
pub mod theme;
pub mod ui;
pub mod value_panel;
```

- [ ] **Step 2.3: 编译确认模块加载成功**

Run: `cargo build -p modbussim-ui-shared`
Expected: 无错误。

- [ ] **Step 2.4: 运行新单测确认通过**

Run: `cargo test -p modbussim-ui-shared hero_anim::tests`
Expected: 3 个 test 都 PASS。

- [ ] **Step 2.5: Commit**

```bash
git add crates/modbussim-ui-shared/src/hero_anim.rs crates/modbussim-ui-shared/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(ui-shared): 新增 hero_anim::show_welcome_hero 三色弦欢迎屏

把原本 app.rs 本地的 paint_dancing_strings 逻辑搬到共享模块，加入
HeroPulseFeed 驱动振幅 + 错误态 lerp + 节流重绘（失焦 100ms/活跃
60fps/空闲 20fps）。调用方下一步接线。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: 在 `SlaveApp` 接入 `HeroPulseState`，替换旧 `paint_dancing_strings`

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs`
  - 删除：`fn paint_dancing_strings` (`app.rs:3812-3856`) 及其调用 (`app.rs:2260`)
  - 新增：模块开头的 `HeroPulseState` struct + impl
  - 修改：`SlaveApp` 结构（`app.rs:272-354`）加 `hero_pulse` 字段
  - 修改：`SlaveApp::new*` 初始化函数（`app.rs:540-546` 等多处）补字段
  - 修改：`render_main` 中 `Selection::None` 分支（`app.rs:2250-2261`）换成 `show_welcome_hero`

- [ ] **Step 3.1: 在 `app.rs` import 区补上需要的模块**

找到 `app.rs:22` 的 `use modbussim_ui_shared::ui as uikit;`，在其下方添加：

```rust
use modbussim_ui_shared::hero_anim::{show_welcome_hero, HeroPulseFeed};
```

- [ ] **Step 3.2: 在 `app.rs` 的常量/类型区加 `HeroPulseState`**

在 `app.rs` 的 `pub struct SlaveApp {` (L272) **之前** 插入：

```rust
/// 空状态 Hero 动画的心跳采样器。每 100ms 从所有连接的 LogCollector
/// 读取最近 1s 的 TX/RX 条数，归一化到 0..=1 作为振幅乘子。
struct HeroPulseState {
    /// 最近一次采样得到的总条数（所有连接聚合）。
    recent_count: u32,
    /// 上次采样时刻；None 表示从未采样。
    last_sample: Option<std::time::Instant>,
}

impl HeroPulseState {
    const WINDOW: std::time::Duration = std::time::Duration::from_secs(1);
    const SAMPLE_EVERY: std::time::Duration = std::time::Duration::from_millis(100);
    const SATURATION: u32 = 40;

    fn new() -> Self {
        Self {
            recent_count: 0,
            last_sample: None,
        }
    }

    /// 若距上次采样已 >= 100ms，则重新遍历所有 connection 的 log_collector
    /// 累加最近 1s 的条数，返回归一化后的振幅（未经 gain 混合）。
    fn sample(&mut self, connections: &SharedConnections) -> f32 {
        let due = self
            .last_sample
            .map_or(true, |t| t.elapsed() >= Self::SAMPLE_EVERY);
        if due {
            if let Ok(entries) = connections.try_read() {
                let total: usize = entries
                    .iter()
                    .filter_map(|e| e.log_collector.try_count_within(Self::WINDOW))
                    .sum();
                self.recent_count = total.min(u32::MAX as usize) as u32;
                self.last_sample = Some(std::time::Instant::now());
            }
            // 写锁冲突 / 初次读失败：沿用上次 recent_count
        }
        amp_from_counts(self.recent_count)
    }

    fn feed(&mut self, connections: &SharedConnections) -> HeroPulseFeed {
        HeroPulseFeed {
            amp: self.sample(connections),
            has_error: false,
            disabled: false,
        }
    }
}

/// 把"最近 1s 总条数"归一化到 0..=1。SATURATION=40 即约 40 条/秒满振。
fn amp_from_counts(total: u32) -> f32 {
    (total as f32 / HeroPulseState::SATURATION as f32).clamp(0.0, 1.0)
}
```

- [ ] **Step 3.3: `SlaveApp` 加字段**

找到 `app.rs:349` 这行：

```rust
    log_last_refresh: Option<Instant>,
```

**紧接其下**插入：

```rust

    // Welcome-screen hero animation pulse sampler.
    hero_pulse: HeroPulseState,
```

- [ ] **Step 3.4: 在 `SlaveApp::new` / 初始化路径补字段**

找到 `app.rs:540-546` 的初始化尾部：

```rust
            log_state: LogPanelState::new(),
            log_cache: Vec::new(),
            log_cache_conn_id: None,
            log_last_refresh: None,
            value_parse_open: true,
        }
    }
```

改为：

```rust
            log_state: LogPanelState::new(),
            log_cache: Vec::new(),
            log_cache_conn_id: None,
            log_last_refresh: None,
            hero_pulse: HeroPulseState::new(),
            value_parse_open: true,
        }
    }
```

**检查其他初始化点**：`grep` 找 `value_parse_open: true` 出现位置（构造函数可能不止一个）：

Run: `grep -n "value_parse_open: true" crates/modbussim-egui/src/app.rs`

Expected: 若只输出一行（`app.rs:545` 左右），已处理完；若有多行，对每一行**紧挨 `value_parse_open:` 之前**同样加 `hero_pulse: HeroPulseState::new(),`。

- [ ] **Step 3.5: 替换 `Selection::None` 分支**

找到 `app.rs:2250-2261`：

```rust
            Selection::None => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.heading(format!("{}  ModbusSlave", icons::CPU));
                    uikit::caption(
                        ui,
                        self.flavor,
                        "从左侧创建或选中一个连接 / 设备 / 寄存器组。",
                    );
                    ui.add_space(28.0);
                    paint_dancing_strings(ui, self.flavor);
                });
            }
```

替换为：

```rust
            Selection::None => {
                let feed = self.hero_pulse.feed(&self.connections);
                show_welcome_hero(
                    ui,
                    self.flavor,
                    icons::CPU,
                    "ModbusSlave",
                    "从左侧创建或选中一个连接 / 设备 / 寄存器组。",
                    feed,
                );
            }
```

- [ ] **Step 3.6: 删除旧 `paint_dancing_strings`**

Run: `grep -n "fn paint_dancing_strings\|^/// 空状态装饰" crates/modbussim-egui/src/app.rs`

按输出定位到注释起始行与函数体结束行（从 `/// 空状态装饰：致敬 egui demo` 注释那一行起，一直到 `fn paint_dancing_strings` 函数最外层 `}` 结束），**整段删除**。

删除后检查：
- `grep -n "paint_dancing_strings" crates/modbussim-egui/src/app.rs` 应无任何输出（若 Step 3.5 漏替换会在这暴露）
- 文件末尾无残留孤立空行或多余大括号

- [ ] **Step 3.7: 编译确认**

Run: `cargo build -p modbussim-egui`
Expected: 无错误。如报 "cannot find function `paint_dancing_strings`"，说明 Step 3.5 的替换没生效；如报 "no field `hero_pulse`"，说明 Step 3.3/3.4 漏了初始化点。

- [ ] **Step 3.8: 手动视觉验证（冒烟）**

Run: `cargo run -p modbussim-egui`

Expected：
- 启动后 `Selection::None` 可见大标题 + 提示文字 + 三色弦动画
- 没建任何连接时弦幅度很小（约原版的 15%）
- Activity Monitor 观察：窗口前台且静止时 CPU 约 20fps 重绘水平

- [ ] **Step 3.9: Commit**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "$(cat <<'EOF'
refactor(slave-app): 空状态 Hero 接入共享 show_welcome_hero + 心跳采样

删除本地 paint_dancing_strings，改调 ui-shared 的 show_welcome_hero；
新增 HeroPulseState 字段，每 100ms 聚合所有连接 LogCollector 最近
1s 条数归一化振幅（SATURATION=40）。本轮 has_error 恒 false 预留。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: 给 `amp_from_counts` 加单测（回归保护）

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs`（在文件末尾 `#[cfg(test)] mod tests` 或新增模块）

- [ ] **Step 4.1: 在 `app.rs` 末尾（最后一个 `}` 之后）追加测试模块**

Run: `tail -n 5 crates/modbussim-egui/src/app.rs` 确认文件结尾形态。

在文件最末尾追加：

```rust

#[cfg(test)]
mod hero_pulse_tests {
    use super::amp_from_counts;

    #[test]
    fn amp_zero_when_silent() {
        assert_eq!(amp_from_counts(0), 0.0);
    }

    #[test]
    fn amp_saturates_at_one() {
        assert_eq!(amp_from_counts(40), 1.0);
        assert_eq!(amp_from_counts(100), 1.0);
        assert_eq!(amp_from_counts(u32::MAX), 1.0);
    }

    #[test]
    fn amp_linear_in_between() {
        // 20 条 → 0.5，允许浮点误差
        let v = amp_from_counts(20);
        assert!((v - 0.5).abs() < 1e-6, "got {}", v);
    }
}
```

- [ ] **Step 4.2: 运行确认通过**

Run: `cargo test -p modbussim-egui hero_pulse_tests`
Expected: 3 个 test PASS。

- [ ] **Step 4.3: Commit**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "$(cat <<'EOF'
test(slave-app): amp_from_counts 归一化边界

保护 SATURATION 常量与 clamp 行为，防止未来调整意外回归。

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: 全量回归 + 手动视觉验收

**Files:** 无代码改动。仅运行校验命令并填写视觉验收清单。

- [ ] **Step 5.1: 格式化**

Run: `cargo fmt --all`
Expected: 无改动。若有改动，`git add -u && git commit -m "style: cargo fmt --all"`。

- [ ] **Step 5.2: Clippy**

Run: `cargo clippy --workspace -- -D warnings`
Expected: 0 warning / 0 error。若有，逐一修复后 `git commit`。

- [ ] **Step 5.3: 全量测试**

Run: `cargo test --workspace`
Expected: 所有现有 test + 新增 7 个 test（2 × try_count_within，3 × hero_anim::tests，3 × hero_pulse_tests）全通过。

- [ ] **Step 5.4: 手动视觉验收清单**

Run: `cargo run -p modbussim-egui`

手动核对：

- [ ] 启动后 `Selection::None`：可见 `⌁ ModbusSlave` 大标题 + 中文提示 + 三色弦画布
- [ ] 无连接 / 无流量时：弦幅度约原来 15%，视觉上轻微起伏
- [ ] 新建一个 TCP 连接 + 1 台从站，启动连接（不发流量）：弦幅度仍在底噪范围
- [ ] 用外部 Modbus Master 工具（如 `mbpoll`）向 Slave 高频查询 FC03（例如 50Hz）：弦幅度明显上拉接近原版
- [ ] 停止 Master：~1 秒内弦幅度回落
- [ ] 选中某个连接 / 设备 / 寄存器组：Hero 消失（正常离开空状态）
- [ ] 再切回 `Selection::None`：Hero 再次出现
- [ ] 窗口失焦（点到浏览器）：打开 Activity Monitor / `top -pid <pid>` 观察 CPU 从活跃降到约 10fps 对应水平

- [ ] **Step 5.5: 若有 fmt / clippy 衍生 commit，在此统一 push**

本地不 push；仅报告给用户。

---

## 完成标准

- 所有 Task 的 `- [ ]` 打钩
- `git log --oneline` 看到至少 3 个本次提交（Task 1 / 2 / 3），可能多 1-2 个（Task 4 / 5 衍生）
- `cargo test --workspace`、`cargo clippy --workspace -- -D warnings`、`cargo fmt --all --check` 全绿
- 手动视觉清单 8 项全部打钩
