# Slave 端空状态 Hero · 三色 Dancing Strings 动画

> 规划日期：2026-04-22 · 分支：`feat/slave-tls-ui-2026-04`
> 2026-04-22 修订：按代码事实校准（搬迁而非新建、LogCollector 在 core、LogEntry 无错误级）。

## Context

ModbusSim Slave 端 UI (`crates/modbussim-egui`) 的 `Selection::None` 空状态已经有一版 Hero 动画：`app.rs:2260` 调用本地私有函数 `paint_dancing_strings`（定义在 `app.rs:3814-3856`）——三色 accent/success/warn × mode∈{2,3,5} × 120 点，直接照搬 egui demo，**振幅固定、无节流、无数据驱动**。

本轮改造目标：
1. **搬迁**：把 `paint_dancing_strings` 从 `app.rs` 私有函数提取到 `modbussim-ui-shared`，以便未来 Master 端 (`modbusmaster-egui`) 复用。
2. **接入节流**：当前 `ctx.request_repaint()` 全速重绘吃 CPU，改为按活跃度自适应 16ms/50ms/100ms。
3. **心跳驱动振幅**：聚合所有 connection 的 `LogCollector` 最近 1s TX/RX 总数 → 归一化振幅乘子，让空闲时弦轻起伏、通信繁忙时弦大幅跃动。
4. **预留错误红化钩子**：`HeroPulseFeed.has_error` 字段定义但本轮恒 false（`LogEntry` 当前无错误级字段，Modbus exception 响应 fc|0x80 也不在枚举里 —— 没有干净信号源，不做启发式）。

目标是视觉强化 + 性能修正，不引入新业务通道、不动 `theme.rs`、不接 Master。

## 最终方案

### 1. 模块边界

新建 `crates/modbussim-ui-shared/src/hero_anim.rs`，对外暴露：

```rust
pub struct HeroPulseFeed {
    /// 振幅乘子，0.0..=1.0；调用方归一化后传入
    pub amp: f32,
    /// 预留错误态（本轮恒 false），true 时三弦会 lerp 到 danger
    pub has_error: bool,
    /// true 时直接渲染纯文本欢迎屏，跳过画布
    pub disabled: bool,
}

pub fn show_welcome_hero(
    ui: &mut egui::Ui,
    flavor: crate::theme::Flavor,
    icon_prefix: &str,       // 例如 icons::CPU
    title: &str,             // 例如 "ModbusSlave"
    caption: &str,           // 例如 "从左侧创建或选中一个连接 / 设备 / 寄存器组。"
    feed: HeroPulseFeed,
);
```

- 模块不持有业务状态；`HeroPulseFeed` 由调用方每帧算好传入。
- 内部负责：heading / caption / canvas / 节流 `request_repaint_after` 决策。
- 调用点：`app.rs:2250-2261` 整段 `vertical_centered { heading + caption + paint_dancing_strings }` 替换为单次 `show_welcome_hero(ui, self.flavor, icons::CPU, "ModbusSlave", "...", self.hero_pulse.feed())`。
- 老的 `fn paint_dancing_strings` 删除。

### 2. 心跳采样

**2.1 `LogCollector` 扩展**（`crates/modbussim-core/src/log_collector.rs`）

现有 `LogCollector` 是 `Arc<RwLock<Vec<LogEntry>>>`，无稳定 id。增加一个非阻塞按时间窗口计数方法：

```rust
/// Non-blocking count of entries whose timestamp >= `since`.
/// Returns None if the lock is held by a writer.
pub fn try_count_since(&self, since: chrono::DateTime<chrono::Utc>) -> Option<usize> {
    self.entries
        .try_read()
        .ok()
        .map(|g| g.iter().rev().take_while(|e| e.timestamp >= since).count())
}
```

利用"entries 按写入时间天然有序"的性质，从尾部 `rev()` 扫到第一条早于 `since` 为止——O(k) 其中 k = 窗口内条数，上限 10000 里通常只扫几十条。

**2.2 `SlaveApp.hero_pulse` 字段**（`crates/modbussim-egui/src/app.rs`）

```rust
struct HeroPulseState {
    /// 每帧从所有 conn.log_collector 累加出的最近 1s 条数
    recent_count: u32,
    last_sample: Option<Instant>,
}

impl HeroPulseState {
    /// 每帧调用；至少每 100ms 采样一次，中间用缓存值
    fn sample(&mut self, conns: &[ConnSnapshotWithCollector]) -> f32 {
        const WINDOW_MS: i64 = 1000;
        const SAMPLE_EVERY_MS: u128 = 100;
        const SATURATION: u32 = 40;
        if self.last_sample.map_or(true, |t| t.elapsed().as_millis() >= SAMPLE_EVERY_MS) {
            let since = chrono::Utc::now() - chrono::Duration::milliseconds(WINDOW_MS);
            let total: usize = conns
                .iter()
                .filter_map(|c| c.log_collector.try_count_since(since))
                .sum();
            self.recent_count = total.min(u32::MAX as usize) as u32;
            self.last_sample = Some(Instant::now());
        }
        (self.recent_count as f32 / SATURATION as f32).clamp(0.0, 1.0)
    }

    fn feed(&mut self, conns: &[_]) -> HeroPulseFeed {
        HeroPulseFeed { amp: self.sample(conns), has_error: false, disabled: false }
    }
}
```

- **多连接聚合**：每个 connection 各持 `Arc<LogCollector>`，遍历累加。
- **SATURATION = 40**：即 ~40 条/秒满振（100ms bucket 里 4 条）。经验值，可后续调。
- **采样节流**：100ms 采一次，之间复用 `recent_count` 缓存，避免每帧扫 10000 × N 次。
- **不做**：不按 id 增量采样（Vec remove(0) 偏移不可靠）；不开单独线程（try_read 够轻）。

`SlaveApp` 的 `connections: Arc<RwLock<Vec<ConnectionEntry>>>` 已有，`sample` 时临时 try_read 拿快照即可。

### 3. 绘制与布局

`show_welcome_hero` 内部：

```rust
pub fn show_welcome_hero(
    ui: &mut egui::Ui,
    flavor: crate::theme::Flavor,
    icon_prefix: &str,
    title: &str,
    caption: &str,
    feed: HeroPulseFeed,
) {
    use egui::epaint::PathStroke;
    use egui::{emath, epaint, pos2, vec2, Rect};

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

            let gain = 0.15 + 0.85 * feed.amp;   // 底噪 15%，满载 100%
            let modes = [
                (2u32, crate::theme::accent(flavor)),
                (3u32, crate::theme::success(flavor)),
                (5u32, crate::theme::warn(flavor)),
            ];
            let mut shapes = Vec::with_capacity(modes.len());
            for &(mode, color) in &modes {
                let modef = mode as f64;
                let n = 120;
                let speed = 1.5_f64;
                let points: Vec<egui::Pos2> = (0..=n)
                    .map(|i| {
                        let t = i as f64 / n as f64;
                        let base = (time * speed * modef).sin() / modef;
                        let y = gain as f64 * base * (t * std::f64::consts::TAU / 2.0 * modef).sin();
                        to_screen * pos2(t as f32, y as f32)
                    })
                    .collect();
                let thickness = 8.0 / mode as f32;
                let final_color = if feed.has_error {
                    lerp_color(color, crate::theme::danger(flavor), 0.6)
                } else {
                    color
                };
                shapes.push(epaint::Shape::line(points, PathStroke::new(thickness, final_color)));
            }
            ui.painter().extend(shapes);
        });

        // 节流重绘
        let interval = std::time::Duration::from_millis(
            if !ui.ctx().input(|i| i.focused) { 100 }
            else if feed.amp >= 0.3 { 16 }
            else { 50 },
        );
        ui.ctx().request_repaint_after(interval);
    });
}

fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 { /* 逐通道 lerp */ }
```

- **尺寸**：沿用现有 `max_w = min(avail, 560) × 0.22` 比例（现有代码已这么做）。
- **颜色**：保持三色，`lerp_color` 辅助函数留着但本轮 `has_error = false` 走不到。
- **heading/caption** 进入模块内；调用方只传文案。

### 4. 重绘节流

- 窗口失焦 → 100ms（约 10fps）
- `feed.amp >= 0.3` → 16ms（~60fps，有人在忙，画流畅）
- `feed.amp < 0.3` → 50ms（~20fps，底噪期足够）
- `feed.disabled == true` → 不调画布也不 `request_repaint_after`

移除现有的无条件 `ui.ctx().request_repaint()`。

### 5. 需修改/新增文件

| 文件 | 改动 |
|---|---|
| `crates/modbussim-core/src/log_collector.rs` | 新增 `try_count_since(DateTime<Utc>) -> Option<usize>` + 单测 |
| `crates/modbussim-ui-shared/src/hero_anim.rs` | 新建：`HeroPulseFeed`、`show_welcome_hero`、`lerp_color` |
| `crates/modbussim-ui-shared/src/lib.rs` | 加 `pub mod hero_anim;` |
| `crates/modbussim-egui/src/app.rs` | 删 `paint_dancing_strings`；`SlaveApp` 加 `hero_pulse: HeroPulseState`；`render_main` 的 `Selection::None` 分支改调 `show_welcome_hero` |

**不改**：`theme.rs`、`log_panel.rs`（UI 层）、`value_panel.rs`、任何 Master 端 crate。

### 6. 测试 · 验证

**单测**：

1. `LogCollector::try_count_since`（core crate）：
   - 空 collector → `Some(0)`
   - 3 条最近 + 2 条远于窗口 → `Some(3)`
   - 全部早于窗口 → `Some(0)`
2. `HeroPulseState::sample`：可以用 `mock` 的 `Vec<&LogCollector>` 注入已预填充的 collector，验证满振/静默的返回值。

**手动视觉验证**（`cargo run -p modbussim-egui`）：

- 启动后 `Selection::None`：三弦轻起伏（gain ≈ 0.15），肉眼 CPU 应比之前明显降低（Activity Monitor 比较）
- 连接一台 Master，往里打 FC03 高频查询 → 弦振幅明显上拉（gain → 1.0）
- 停止 Master，~1s 后振幅回落
- 窗口失焦 → 重绘降到 10fps（观察 CPU 曲线）

**回归**：`cargo fmt --all && cargo clippy --workspace -- -D warnings && cargo test --workspace`。

### 7. 风险 / 不做清单

已识别：
- **SATURATION=40 过高/过低**：经验初值，按视觉反馈再调。用常量暴露以便调整。
- **try_read 争用**：`LogCollector` 写锁持有时 `try_count_since` 返回 `None`，那一帧沿用上次 `recent_count` 缓存——已被节流逻辑覆盖。
- **chrono 已是依赖**：`modbussim-core` 的 `Cargo.toml` 已经因 `LogEntry.timestamp` 引入 chrono，本次无需新增依赖。

YAGNI（明确不做）：
- Error 级启发式（要么等引入 Level 字段再做，要么别做）
- 动画偏好 UI 开关
- Master 端接入（API 已预留但不调）
- 寄存器值 → 振幅映射

### 8. 关键文件路径（索引）

- `crates/modbussim-egui/src/app.rs:2250-2261` — `Selection::None` 当前空状态（待替换）
- `crates/modbussim-egui/src/app.rs:3814-3856` — 现有 `paint_dancing_strings`（待删除）
- `crates/modbussim-egui/src/app.rs:575` / `:593` — `entry.log_collector.try_get_all() / .clear()` 参考用法
- `crates/modbussim-core/src/log_collector.rs` — 待扩展 `try_count_since`
- `crates/modbussim-ui-shared/src/theme.rs` — `accent/success/warn/danger(flavor)` 签名参考
- `crates/modbussim-ui-shared/src/ui.rs` — `caption(ui, flavor, text)` 参考
- 参考：egui `crates/egui_demo_lib/src/demo/dancing_strings.rs`
