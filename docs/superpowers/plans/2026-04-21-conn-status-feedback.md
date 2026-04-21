# 连接状态反馈与按钮分工 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让子站连接的运行/停止状态在 UI 上有明确视觉反馈（圆点 + 整行染色 + 脉动 + tag 染色），并整理「启动/停止/删除」按钮分工 —— 树节点内只放高频启停、footer 只放低频删除（带二次确认）。

**Architecture:** 全部基于已落地的 token 系统（`theme::success / danger / text_muted / accent`），新增 1 个 ui helper（`danger_button_sm`），SlaveApp struct 加 1 个状态字段（`pending_delete`），`render_tree` 与 SidePanel footer / status_bar 三处局部改写。无新依赖、不动 backend、不动 `TreeAction` 枚举。

**Tech Stack:** Rust · egui 0.33.3 · egui-shadcn 0.3 · 现有 `crates/modbussim-ui-shared` 与 `crates/modbussim-egui`

**Spec:** `docs/superpowers/specs/2026-04-21-conn-status-feedback-design.md`

---

## File Structure

| 文件 | 职责 | 改动类型 |
|---|---|---|
| `crates/modbussim-ui-shared/src/ui.rs` | 通用 UI helper | 新增 1 个函数 (`danger_button_sm`) |
| `crates/modbussim-egui/src/app.rs` | 子站主应用 | 加 1 字段 + 4 段渲染改写：tree connection row · tree 节点按钮 · SidePanel footer · status_bar |

不新增文件。所有改动可读、局部、可独立提交。

---

## Task 1: 新增 `danger_button_sm` helper

**Files:**
- Modify: `crates/modbussim-ui-shared/src/ui.rs` (insert near existing `secondary_button_sm`)

- [ ] **Step 1.1: Read 现状**

```bash
grep -n "secondary_button_sm" crates/modbussim-ui-shared/src/ui.rs
```

应该看到一个 `pub fn secondary_button_sm(...)` 函数（之前 commit 加的 Outline + Sm wrapper）。新 helper 紧跟其后。

- [ ] **Step 1.2: 插入 `danger_button_sm`**

在 `secondary_button_sm` 函数体的右花括号之后、`/// Icon-only button` 注释之前，插入：

```rust
/// Compact destructive button: shadcn Destructive variant + Sm size.
/// Use for low-frequency, dangerous actions like "删除连接" where you
/// want red prominence but Md size feels visually overweight.
pub fn danger_button_sm(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response {
    let theme = shadcn_theme(flavor);
    egui_shadcn::button(
        ui,
        &theme,
        text.into(),
        egui_shadcn::tokens::ControlVariant::Destructive,
        egui_shadcn::tokens::ControlSize::Sm,
        true,
    )
}
```

- [ ] **Step 1.3: 编译**

```bash
cargo check -p modbussim-ui-shared
cargo clippy -p modbussim-ui-shared --no-deps -- -D warnings
```

预期 PASS。

- [ ] **Step 1.4: Commit**

```bash
git add crates/modbussim-ui-shared/src/ui.rs
git commit -m "feat(ui-shared): 新增 danger_button_sm (Destructive + Sm)"
```

---

## Task 2: SlaveApp struct 加 `pending_delete` 字段

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs` line 271 (struct) + 372 (new())

- [ ] **Step 2.1: Read 现状**

```bash
sed -n '270,290p' crates/modbussim-egui/src/app.rs
```

确认 struct 头部字段顺序 + `// UI state` 注释段。

- [ ] **Step 2.2: 加字段**

定位 `show_new_tcp_dialog: bool,` 行（约 line 283），紧跟其后插入：

```rust
    /// 删除连接二次确认状态：(conn_id, 首次点击时刻)。
    /// 3 秒内同一连接再次点删除按钮 → 真删；否则按钮 label 自动恢复。
    pending_delete: Option<(String, std::time::Instant)>,
```

- [ ] **Step 2.3: 在 `new()` 里初始化**

在 `pub fn new(...)` 的 struct 实例化块里（grep `show_new_tcp_dialog: false`），紧跟其后加：

```rust
            pending_delete: None,
```

- [ ] **Step 2.4: 编译**

```bash
cargo check -p modbussim-egui
```

预期 PASS（无新引用方未实现报错；`std::time::Instant` 已经在 app.rs 内多处使用，应已 imported）。

- [ ] **Step 2.5: Commit**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "feat(slave-app): SlaveApp 加 pending_delete 状态（删除二次确认）"
```

---

## Task 3: render_tree · 圆点 + 整行染色 + tag 染色

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs` line 1665-1701 (connection row 渲染段 in `render_tree`)

- [ ] **Step 3.1: Read 现状**

```bash
sed -n '1645,1725p' crates/modbussim-egui/src/app.rs
```

确认 `paint_active_row` 闭包、conn_label 拼接、connection row 整段。

- [ ] **Step 3.2: 替换 state_tag + conn_label 段**

把现有这段（约 line 1668-1672）：

```rust
            let state_tag = match snap.state {
                ConnectionState::Running => "运行中",
                ConnectionState::Stopped => "已停止",
            };
            let conn_label = format!("{} [{}]", snap.label, state_tag);
```

替换为：

```rust
            let (state_text, state_color) = match snap.state {
                ConnectionState::Running => ("运行中", theme::success(flavor)),
                ConnectionState::Stopped => ("已停止", theme::text_muted(flavor)),
            };
            let is_running = matches!(snap.state, ConnectionState::Running);
```

注意：`conn_label` 旧拼接被废弃；下面 painter.text 改为分两次绘制 label + tag。

- [ ] **Step 3.3: 在 `paint_active_row` 闭包旁追加 `paint_running_row`**

定位 `let paint_active_row = |ui: &egui::Ui, rect: egui::Rect| { ... };`（约 line 1657-1663），其后追加：

```rust
        // 整行 8% alpha success 染色，仅用于运行中且未选中的 connection row。
        let paint_running_row = |ui: &egui::Ui, rect: egui::Rect| {
            let s = theme::success(flavor);
            ui.painter().rect_filled(
                rect,
                0.0,
                Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), 0x14),
            );
        };
```

如果 `Color32` 没在 `render_tree` 上下文 import，需要在文件顶部 use（grep `use egui::Color32`，多半已有）。

- [ ] **Step 3.4: 改写 connection row 渲染段（line 1675-1701）**

定位整段：

```rust
            // Connection row
            ui.horizontal(|ui| {
                let arrow = if snap.expanded { "▼" } else { "▶" };
                if ui.small_button(arrow).clicked() {
                    action = Some(TreeAction::ToggleConn(snap.id.clone()));
                }
                let row_resp = ui.allocate_response(
                    egui::vec2(ui.available_width(), 22.0),
                    egui::Sense::click(),
                );
                if conn_is_selected {
                    paint_active_row(ui, row_resp.rect);
                } else if row_resp.hovered() {
                    ui.painter()
                        .rect_filled(row_resp.rect, 0.0, theme::bg_hover(flavor));
                }
                let label_color = if conn_is_selected { acc_fg } else { text_color };
                ui.painter().text(
                    row_resp.rect.left_center() + egui::vec2(4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &conn_label,
                    egui::FontId::proportional(12.5),
                    label_color,
                );
                if row_resp.clicked() {
                    action = Some(TreeAction::SelectConn(snap.id.clone()));
                }
            });
```

整段替换为：

```rust
            // Connection row
            ui.horizontal(|ui| {
                let arrow = if snap.expanded { "▼" } else { "▶" };
                if ui.small_button(arrow).clicked() {
                    action = Some(TreeAction::ToggleConn(snap.id.clone()));
                }
                let row_resp = ui.allocate_response(
                    egui::vec2(ui.available_width(), 22.0),
                    egui::Sense::click(),
                );

                // 优先级：selected > running 染色 > hover；三者互斥
                if conn_is_selected {
                    paint_active_row(ui, row_resp.rect);
                } else if is_running {
                    paint_running_row(ui, row_resp.rect);
                } else if row_resp.hovered() {
                    ui.painter()
                        .rect_filled(row_resp.rect, 0.0, theme::bg_hover(flavor));
                }

                // 状态圆点（左侧 8px 偏移、半径 3.5）
                let dot_center = row_resp.rect.left_center() + egui::vec2(8.0, 0.0);
                if is_running {
                    let phase = (ui.input(|i| i.time)
                        * (2.0 * std::f64::consts::PI / 1.5))
                        .sin() * 0.5 + 0.5;
                    let alpha = (180.0 + 75.0 * phase) as u8;
                    let s = theme::success(flavor);
                    let c = Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), alpha);
                    ui.painter().circle_filled(dot_center, 3.5, c);
                } else {
                    ui.painter().circle_stroke(
                        dot_center,
                        3.5,
                        egui::Stroke::new(1.0, theme::text_muted(flavor)),
                    );
                }

                // label + tag 分两次绘制
                let label_color = if conn_is_selected { acc_fg } else { text_color };
                let label_pos =
                    row_resp.rect.left_center() + egui::vec2(20.0, 0.0); // 圆点之后 12px
                let label_galley = ui.painter().layout_no_wrap(
                    snap.label.clone(),
                    egui::FontId::proportional(12.5),
                    label_color,
                );
                let label_w = label_galley.size().x;
                ui.painter().galley(label_pos - egui::vec2(0.0, label_galley.size().y / 2.0),
                    label_galley, label_color);
                let tag_pos = label_pos + egui::vec2(label_w + 6.0, 0.0);
                ui.painter().text(
                    tag_pos,
                    egui::Align2::LEFT_CENTER,
                    state_text,
                    egui::FontId::proportional(11.0),
                    state_color,
                );

                if row_resp.clicked() {
                    action = Some(TreeAction::SelectConn(snap.id.clone()));
                }
            });
```

> 注：`painter().galley(...)` 需要左上角坐标，`label_galley.size().y / 2.0` 是把 galley 中心对齐到 row left_center。如 egui 0.33 API 略不同（galley 函数签名变化），按编译错误调整：可以退化用 `painter().text(label_pos, Align2::LEFT_CENTER, &snap.label, FontId::proportional(12.5), label_color)` 代替 galley 组合，然后用 `painter().fonts(|f| f.layout_no_wrap(...).size().x)` 估算宽度后定位 tag_pos。

- [ ] **Step 3.5: 编译**

```bash
cargo check -p modbussim-egui
cargo clippy -p modbussim-egui --no-deps -- -D warnings
```

如果 `painter().galley` 签名报错 → 改用 fallback 方案（详见 Step 3.4 注释）。

- [ ] **Step 3.6: Commit**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "feat(slave-app): 连接状态视觉化 — 圆点 + 整行染色 + tag 染色"
```

---

## Task 4: render_tree · 树节点按钮简化

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs` line 1703-1721 (Per-connection start/stop/delete buttons)

- [ ] **Step 4.1: Read 现状**

```bash
sed -n '1700,1725p' crates/modbussim-egui/src/app.rs
```

应看到 `// Per-connection start/stop/delete buttons` + `ui.horizontal(...)` 三按钮 small_button 段。

- [ ] **Step 4.2: 替换为单按钮 + outline + 颜色**

把这段（line 1703-1721）：

```rust
            // Per-connection start/stop/delete buttons
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                match snap.state {
                    ConnectionState::Stopped => {
                        if ui.small_button("启动").clicked() {
                            action = Some(TreeAction::StartConn(snap.id.clone()));
                        }
                    }
                    ConnectionState::Running => {
                        if ui.small_button("停止").clicked() {
                            action = Some(TreeAction::StopConn(snap.id.clone()));
                        }
                    }
                }
                if ui.small_button("删除").clicked() {
                    action = Some(TreeAction::RemoveConn(snap.id.clone()));
                }
            });
```

整段替换为：

```rust
            // Per-connection: 单个状态相关按钮（启动/停止），删除挪到 footer
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                let (label, color, act): (&str, Color32, TreeAction) = match snap.state {
                    ConnectionState::Stopped => (
                        "▶ 启动",
                        theme::success(flavor),
                        TreeAction::StartConn(snap.id.clone()),
                    ),
                    ConnectionState::Running => (
                        "■ 停止",
                        theme::warn(flavor),
                        TreeAction::StopConn(snap.id.clone()),
                    ),
                };
                let resp = uikit::secondary_button_sm(
                    ui,
                    flavor,
                    egui::RichText::new(label).color(color).size(11.5),
                );
                if resp.clicked() {
                    action = Some(act);
                }
            });
```

> 注：`secondary_button_sm` 签名是 `text: impl Into<String>`，`RichText` 不能直接 Into<String>。需要：
> - 选项 A：把 helper 改为接受 `impl Into<egui::WidgetText>`（更灵活但破坏 API）
> - 选项 B：本任务内调用 `egui_shadcn::button(ui, &uikit::shadcn_theme(flavor), label.to_string(), ControlVariant::Outline, ControlSize::Sm, true)` —— 但 `shadcn_theme` 是 ui.rs 内的私有 fn，不可外用
> - 选项 C（**采用**）：`secondary_button_sm` 仍用纯文本（不带 icon 颜色），按钮**外**画一个 colored richtext label 替代 icon，如：
>
>   ```rust
>   ui.horizontal(|ui| {
>       ui.add_space(18.0);
>       ui.label(egui::RichText::new(label_icon).color(color).size(13.0));  // ▶ / ■ icon 部分
>       let resp = uikit::secondary_button_sm(ui, flavor, label_text);     // "启动" / "停止" 纯文本
>       if resp.clicked() { action = Some(act); }
>   });
>   ```
>
> 把 `(label_icon, label_text)` 拆成两个 tuple element。最终代码：
>
> ```rust
> ui.horizontal(|ui| {
>     ui.add_space(18.0);
>     let (icon, label_text, color, act): (&str, &str, Color32, TreeAction) =
>         match snap.state {
>             ConnectionState::Stopped => (
>                 "▶", "启动",
>                 theme::success(flavor),
>                 TreeAction::StartConn(snap.id.clone()),
>             ),
>             ConnectionState::Running => (
>                 "■", "停止",
>                 theme::warn(flavor),
>                 TreeAction::StopConn(snap.id.clone()),
>             ),
>         };
>     ui.label(egui::RichText::new(icon).color(color).size(12.0));
>     if uikit::secondary_button_sm(ui, flavor, label_text).clicked() {
>         action = Some(act);
>     }
> });
> ```
>
> 这是 plan 的最终采用版本。

- [ ] **Step 4.3: 编译**

```bash
cargo check -p modbussim-egui
cargo clippy -p modbussim-egui --no-deps -- -D warnings
```

- [ ] **Step 4.4: Commit**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "feat(slave-app): 树节点按钮简化 — 单按钮 + 状态色 icon · 删按钮挪 footer"
```

---

## Task 5: SidePanel footer · 删除连接 + 二次确认

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs` line 3409-3443 (footer 内的按钮 horizontal 段)

- [ ] **Step 5.1: Read 现状**

```bash
sed -n '3395,3445p' crates/modbussim-egui/src/app.rs
```

应看到 `egui::Frame::new()` footer + `ui.horizontal(|ui| { stop_label / 删除连接 })` 两个 link_action 段。

- [ ] **Step 5.2: 替换 footer 内按钮逻辑**

把 `if let Some(snap) = active_conn { ... }` 整段（line 3415-3442）替换为：

```rust
                                    if let Some(snap) = active_conn {
                                        let conn_id = snap.id.clone();
                                        let conn_label_short = snap.label.clone();
                                        let now = std::time::Instant::now();
                                        let confirming = self
                                            .pending_delete
                                            .as_ref()
                                            .filter(|(id, t)| {
                                                id == &conn_id
                                                    && now.duration_since(*t).as_secs_f32() < 3.0
                                            })
                                            .is_some();
                                        let label: String = if confirming {
                                            "× 再点一次确认".to_string()
                                        } else {
                                            format!("× 删除连接 {}", conn_label_short)
                                        };
                                        ui.horizontal(|ui| {
                                            if uikit::danger_button_sm(ui, self.flavor, label)
                                                .clicked()
                                            {
                                                if confirming {
                                                    tree_action =
                                                        Some(TreeAction::RemoveConn(conn_id));
                                                    self.pending_delete = None;
                                                } else {
                                                    self.pending_delete =
                                                        Some((conn_id, now));
                                                    ctx.request_repaint_after(
                                                        std::time::Duration::from_millis(3100),
                                                    );
                                                }
                                            }
                                        });
                                    }
```

- [ ] **Step 5.3: 编译**

```bash
cargo check -p modbussim-egui
cargo clippy -p modbussim-egui --no-deps -- -D warnings
```

如果 `ctx` 在 footer 闭包里不在作用域 → 在 SidePanel `.show(ctx, |ui| { ... })` 之前 `let ctx_clone = ctx.clone();` 然后 footer 内用 `ctx_clone.request_repaint_after(...)`。

- [ ] **Step 5.4: Commit**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "feat(slave-app): footer 删除连接 — danger_button_sm + 3 秒二次确认"
```

---

## Task 6: status_bar · 脉动 ● 与新文案

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs` line 3454-3500 (status_bar BottomPanel)

- [ ] **Step 6.1: Read 现状**

```bash
sed -n '3450,3515p' crates/modbussim-egui/src/app.rs
```

确认现有 `if let Some(err) ... else if let Some(msg) ... else { ●就绪 }` 三分支。

- [ ] **Step 6.2: 在 status_bar 渲染前计算 any_running**

定位 `let conn_count = self.conn_snapshot.len();` 行（约 line 3451），紧跟其后插入：

```rust
        let any_running = self
            .conn_snapshot
            .iter()
            .any(|s| matches!(s.state, ConnectionState::Running));
        let zero_conns = conn_count == 0;
```

- [ ] **Step 6.3: 重写 else 分支（替代 `● 就绪`）**

把 `} else { ... ●就绪 ... }` 这段（line 3493-3500）：

```rust
                    } else {
                        ui.add(egui::Label::new(
                            egui::RichText::new("●")
                                .color(theme::success(flavor))
                                .size(11.0),
                        ));
                        theme::text::crumb(ui, flavor, "就绪");
                    }
```

替换为：

```rust
                    } else {
                        let (dot_color, dot_alpha, status_text, text_color) = if zero_conns {
                            (
                                theme::text_muted(flavor),
                                255u8,
                                "未连接",
                                theme::text_muted(flavor),
                            )
                        } else if any_running {
                            let phase = (ui.input(|i| i.time)
                                * (2.0 * std::f64::consts::PI / 1.5))
                                .sin() * 0.5 + 0.5;
                            let alpha = (180.0 + 75.0 * phase) as u8;
                            (theme::success(flavor), alpha, "运行中", theme::success(flavor))
                        } else {
                            (
                                theme::text_muted(flavor),
                                255u8,
                                "已停止",
                                theme::text_muted(flavor),
                            )
                        };
                        let dot = if zero_conns || !any_running { "○" } else { "●" };
                        let dot_color_with_alpha = Color32::from_rgba_unmultiplied(
                            dot_color.r(),
                            dot_color.g(),
                            dot_color.b(),
                            dot_alpha,
                        );
                        ui.add(egui::Label::new(
                            egui::RichText::new(dot)
                                .color(dot_color_with_alpha)
                                .size(11.0),
                        ));
                        ui.add(egui::Label::new(
                            egui::RichText::new(status_text)
                                .color(text_color)
                                .size(11.0),
                        ));
                    }
```

- [ ] **Step 6.4: 全局脉动 repaint 触发**

在 `egui::TopBottomPanel::bottom("status_bar")` 之前（line 3454 之前），插入：

```rust
        if any_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
```

这一处 repaint 同时驱动树节点的圆点脉动 + 状态栏 ● 脉动；停止时不请求重绘，UI 静止。

- [ ] **Step 6.5: 编译 + clippy**

```bash
cargo check -p modbussim-egui
cargo clippy -p modbussim-egui --no-deps -- -D warnings
```

- [ ] **Step 6.6: Commit**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "feat(slave-app): 状态栏脉动 ● + 三态文案（运行中/已停止/未连接）"
```

---

## Task 7: 视觉烟测 + fmt + 最终 push

**Files:**
- 不改源码，仅运行验证

- [ ] **Step 7.1: cargo fmt**

```bash
cargo fmt --all
cargo fmt --all -- --check
```

预期：第二条无输出。

如果第二条有输出 → 第一条已 in-place 修复，再 git add/commit：

```bash
git add -u
git commit -m "style: cargo fmt --all"
```

- [ ] **Step 7.2: 全 workspace check**

```bash
cargo check --workspace
cargo clippy -p modbussim-egui -p modbussim-ui-shared --no-deps -- -D warnings
```

预期：focus 两个 crate `-D warnings` 干净；workspace `cargo check` 通过（modbussim-core 4 个 pre-existing warnings 不阻塞）。

- [ ] **Step 7.3: 视觉冒烟 — controller 启动 GUI（人工验证）**

```bash
cargo run -p modbussim-egui --release
```

按以下顺序逐项核对（每项 PASS 才算 OK）：

1. 默认无连接 → 状态栏 `○ 未连接 · 0 连接 · 0 从站`，灰色静态
2. 新建一个 TCP 5502 连接 → 树出现一行：`○ TCP 0.0.0.0:5502  已停止`（左空圆 + tag 灰），无染色
3. 节点下方按钮：`▶（绿）启动`（icon 绿、文字 outline button）
4. 点「启动」：
   - 整行立即 success @ 8% 浅绿背景
   - 圆点变实心绿，1.5 秒周期 alpha 脉动
   - tag 变 `运行中` 绿字
   - 按钮文字变 `■（黄）停止`
   - 状态栏 `● 运行中 · 1 连接 · 1 从站`，● 与树同步脉动
5. 点「停止」反向回退所有视觉
6. 点 footer `× 删除连接 TCP 0.0.0.0:5502`：
   - 按钮文字变 `× 再点一次确认`
   - 等 4 秒不点 → 自动恢复为 `× 删除连接 ...`
   - 在 3 秒内再点 → 真删，连接消失
7. 同时建 2 个连接（再起一个 5503），一开一停 → 树内一行染绿一行不染；状态栏脉动只要任一在跑
8. 视图菜单切浅色模式 → 浅绿染色在白底可辨；圆点与按钮颜色仍语义清晰

- [ ] **Step 7.4: CPU 占用粗测（可选，5 秒钟）**

打开 macOS `Activity Monitor` 或 `top -pid <pid>`：
- 全部连接停止时：`modbussim-egui` CPU < 1% (空闲)
- 一个连接运行：CPU < 5% (脉动重绘)

如果 ≥ 10%，回 Task 6.4 检查 `request_repaint_after` 是否只在 any_running 才调用。

- [ ] **Step 7.5: push**

```bash
git push
```

预期：分支已存在 origin，增量推送 commits（约 6 个 feat + 可能 1 个 style）。

- [ ] **Step 7.6: PR 留言**

PR #2 已开。这次的改动会自动出现在 PR diff 里。如果要追加 description，编辑 PR body 加一段「Connection status feedback」小结，否则直接靠 commit message 即可。

```bash
gh pr view 2 --json url,additions,deletions
```

---

## Self-Review

**1. Spec coverage:**

| Spec 段 | 覆盖 task |
|---|---|
| 1 信息架构（按钮分工） | Task 4（树节点单按钮）+ Task 5（footer 删除）|
| 2 状态指示（圆点 / 整行染色 / tag 染色） | Task 3（render_tree 完整改写）|
| 3 瞬时反馈（4 通道同步） | Task 3 + Task 4（按钮文字立刻变 + 行染色立刻出 + 圆点立刻脉动 + tag 立刻染色，全靠 state flip 自动联动）|
| 4 删除二次确认 | Task 2（state 字段）+ Task 5（confirming 逻辑）|
| 5 性能（仅 any_running 时 repaint） | Task 6.4 |
| 主要修改文件清单 | Task 1（ui.rs `danger_button_sm`）+ Tasks 2-6（app.rs）|
| 验证步骤 1-9 | Task 7.3 8 项 + Task 7.4 CPU + Task 7.2 cargo check/clippy |

**所有 spec 要求都有对应 task。无遗漏。**

**2. Placeholder scan:** 无 TBD / TODO / "implement later" / "add appropriate error handling" / "Similar to Task N" / 引用未定义 type/fn 的步骤。Task 3.4 / Task 4.2 内含一段"如 API 略不同则 fallback"的 inline 说明（针对 egui 0.33 painter().galley 签名变化），是合理的明确兜底，不算 placeholder。

**3. Type consistency:**

- `pending_delete: Option<(String, std::time::Instant)>` 在 Task 2.2 定义，Task 5.2 解构 `(id, t)` 一致
- `danger_button_sm(ui, flavor, label)` 在 Task 1.2 签名 `text: impl Into<String>`，Task 5.2 传 `String` 一致
- `secondary_button_sm` 已存在签名 `text: impl Into<String>`，Task 4.2 传 `&str` 一致
- `paint_running_row` 在 Task 3.3 定义，Task 3.4 调用一致
- `is_running` / `state_text` / `state_color` 在 Task 3.2 定义，Task 3.4 全部使用一致
- `any_running` / `zero_conns` 在 Task 6.2 定义，Task 6.3 + 6.4 全部使用一致
- `TreeAction::StartConn / StopConn / RemoveConn` 已存在，Task 4.2 + Task 5.2 调用一致

**全部 type / 字段名一致。**

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-21-conn-status-feedback.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — 我每个 task 派一个新 subagent 执行，task 间我做 review，适合多 task 串行。

**2. Inline Execution** — 在当前会话按 task 顺序执行，每 2 task 一次 checkpoint review。

**Which approach?**
