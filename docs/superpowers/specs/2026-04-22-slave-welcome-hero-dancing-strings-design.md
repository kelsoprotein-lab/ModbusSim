# Slave 端空状态 Hero · 三色 Dancing Strings 动画

> 规划日期：2026-04-22 · 分支：`feat/slave-tls-ui-2026-04`

## Context

ModbusSim Slave 端 UI (`crates/modbussim-egui`) 目前 `Selection::None` 空状态只有一行大标题 + 一段提示文案（`app.rs:2250-2259`）。当用户还没建任何连接、或临时切走选中项时，CentralPanel 大片留白显得单薄。项目已经有脉动状态灯（`app.rs:3660-3688`，commit `37769f8`）作为生命体征的"微动"，但占屏非常小。

参考 egui 官方 demo 的 [`dancing_strings.rs`](https://github.com/emilk/egui/blob/main/crates/egui_demo_lib/src/demo/dancing_strings.rs)，可以在空状态加一块 Hero 画布，既让首屏更有"活着"的质感，又能把 Slave 运行时的 TX/RX 节奏直观视觉化——"没在忙"时轻轻起伏，"在忙"时琴弦跃动，出错瞬间三条变红。目标是纯视觉强化，不引入新的业务数据通道、不动现有 theme/log_panel。

## 最终方案

### 1. 模块边界

新增 `crates/modbussim-ui-shared/src/hero_anim.rs`，对外：

```rust
pub struct HeroPulseFeed {
    pub amp: f32,            // 0.0..=1.0
    pub has_error: bool,
    pub disabled: bool,      // 预留开关，当前恒 false
}

pub fn show_welcome_hero(ui: &mut egui::Ui, feed: HeroPulseFeed);
```

- 模块不持有业务状态，只负责渲染 + 调用 `ctx.request_repaint_after`。
- 放在 `ui-shared` 而非 `modbussim-egui` 本地：未来 Master 端 (`modbusmaster-egui`) 可直接复用。
- `app.rs` `Selection::None` 分支（L2250-2259）整段替换为 `show_welcome_hero(ui, self.hero_pulse.feed())`。

### 2. 心跳采样

在 `SlaveApp` 加字段 `hero_pulse: HeroPulseState`：

```rust
struct HeroPulseState {
    ring:     [u32; 10],   // 1s 窗口 × 10 × 100ms bucket
    err_ring: [u32; 10],
    cursor: usize,
    last_tick: std::time::Instant,
    last_seen_log_id: u64,
}
```

- 每帧 `update()` 开头调用 `hero_pulse.tick(&log_collector)`：
  1. `Instant::now() - last_tick >= 100ms` 时游标前推、旧 bucket 清零。
  2. 从 `LogCollector` 拉取 `last_seen_log_id` 之后的**原始入账**条目（不受 UI 过滤影响）——需在 `LogCollector` 加一个 `total_ingested_since(id: u64) -> impl Iterator<Item = &LogEntry>` 访问器（3-5 行）。
  3. 每条记录累加到当前 bucket；`level == Error` 同步计入 `err_ring`。
- `feed()` 返回：
  - `amp = (sum(ring) as f32 / SATURATION_RATE).clamp(0.0, 1.0)`，`SATURATION_RATE` 取 40（约每 100ms 4 条消息即满振）。
  - `has_error = sum(err_ring) > 0`。

不引入新 channel、不改 `UiEvent`、不计时响应延迟。

### 3. 绘制与布局

`show_welcome_hero` 内部：

```
vertical_centered {
  heading("ModbusSim · Slave")
  add_space(12)
  Frame::canvas {
    allocate_exact_size(vec2(avail_w.min(640), 180))
    draw 3 strings: mode ∈ {2, 3, 5}
  }
  add_space(8)
  muted_label("从左侧创建或选中一个连接 · 或按 ⌘N 新建")
}
```

- 坐标变换：`RectTransform::from_to(Rect{x:0..=1, y:-1..=1}, canvas_rect)`（沿用 demo）。
- 琴弦公式（在 demo 基础上乘入 `gain`）：

  ```rust
  let gain = 0.15 + 0.85 * feed.amp;              // 底噪 15%，满载 100%
  let base = (time * SPEED * mode).sin() / mode;
  let y    = gain * base * (t * TAU/2.0 * mode).sin();
  ```

  `SPEED = 1.5`，`n = 120` 采样点，`thickness = 10.0 / mode`。
- 颜色：默认 `[Theme::accent(), Theme::success(), Theme::warn()]`；`feed.has_error` 时三条各自 `lerp(color, Theme::danger(), 0.6)`。
- **不用** Window（demo 用的是 Window，这里内联 CentralPanel）；**不用** `PathStroke::new_uv` 的渐变（与语义三色冲突）。

### 4. 重绘节流

- `feed.amp >= 0.3` → `request_repaint_after(16ms)`（~60fps）
- `feed.amp <  0.3` → `request_repaint_after(50ms)`（~20fps，与状态栏脉动节奏对齐）
- `ui.ctx().input(|i| !i.focused)` → `request_repaint_after(100ms)`
- `feed.disabled == true` → 直接 `return`，画纯文本欢迎屏

### 5. 需修改/新增文件

| 文件 | 改动 |
|---|---|
| `crates/modbussim-ui-shared/src/hero_anim.rs` | 新建 |
| `crates/modbussim-ui-shared/src/lib.rs` | `pub mod hero_anim;` |
| `crates/modbussim-ui-shared/src/log_panel.rs` | `LogCollector::total_ingested_since()` + `next_log_id()` |
| `crates/modbussim-egui/src/app.rs` | `SlaveApp` 加 `hero_pulse`；`update()` 前置 tick；`Selection::None` 分支改调 `show_welcome_hero` |

**不改**：`theme.rs`、`value_panel.rs`、任何 Master 端 crate。

### 6. 测试 · 验证

1. **单测**（`hero_pulse_state`，不依赖 egui）：
   - 注入 10 条日志 → `amp ≈ 1.0`
   - 静置 1.1s → `amp == 0.0`
   - 注入 Error 级日志 → `has_error == true`；1s 后清零
   - 游标轮转正确性（bucket 越界）
2. **手动视觉验证**：
   - `cargo run -p modbussim-egui` → `Selection::None` 可见三色动画
   - 建连接开始 TX/RX → 振幅明显上拉
   - 人为触发解析错误 → 三弦短暂转红
   - 窗口失焦 → CPU 降档（用 Activity Monitor 观察）
3. **回归**：`cargo fmt --all`、`cargo clippy --workspace -- -D warnings`、`cargo test --workspace`。

### 7. 风险 / 不做清单

已识别：
- **CPU**：三条 120 点 path/frame，60fps，egui 可轻松承受；节流已覆盖。
- **色盲友好度**：蓝/绿/橙 + 错误红在三色色盲下辨识度一般。本次不加开关，后续可做 `accessibility::color_blind_mode`。
- **数据源范围**：只统计 `LogCollector` 入账条目（所有 TX/RX + Log）；不引入独立计数器以免重复维护。

YAGNI（明确不做）：
- 动画开关偏好 UI
- 声音效果
- Master 端接入（API 预留，本次不调）
- 寄存器值→振幅的逐寄存器映射

### 8. 关键文件路径（索引）

- `crates/modbussim-egui/src/app.rs:2250-2259` — `Selection::None` 当前空状态
- `crates/modbussim-egui/src/app.rs:3660-3688` — 既有脉动状态灯（风格参照）
- `crates/modbussim-ui-shared/src/log_panel.rs` — `LogCollector` 持有者
- `crates/modbussim-ui-shared/src/theme.rs:66-385` — `accent/success/warn/danger` 色板
- 参考：egui `crates/egui_demo_lib/src/demo/dancing_strings.rs`
