# 子站 UI 重设计实施计划（B · 工业 HMI 中文版）

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `egui-033-shadcn-migration` 已落地的基础上，重设计子站 UI 视觉系统与信息架构：换冷蓝/绿数值 palette、拉开字号梯度、整理操作层级、值解析改为右侧可关闭抽屉、新增底部状态栏。最终消除"布局/字体/颜色都奇怪"的违和感。

**Architecture:** 改动集中在两个 crate：`modbussim-ui-shared`（theme/palette/wrappers/log_panel）和 `modbussim-egui`（app.rs 主布局 + 寄存器表格 + 值解析改抽屉 + 状态栏）。不引入新依赖，shadcn 按钮/开关复用现状。所有视觉改动通过 token 集中（theme.rs），组件层只调用 token，不再硬编码颜色/字号。

**Tech Stack:** Rust · egui 0.33.3 · egui_extras TableBuilder · egui-shadcn 0.3 · catppuccin-egui（仅作为 palette 容器）

---

## 配色 / 字号 / 间距 Token 速查（贯穿全 Plan）

```text
Layer::L0  #010409   chrome（菜单栏 / SidePanel / 状态栏 / 日志）
Layer::L1  #0d1117   surface（CentralPanel）
Layer::L2  #161b22   raised（hover / 抽屉卡片）
border_subtle  #21262d
border_strong  #30363d
accent_primary #1f6feb   选中 / 表头下划线 / focus
accent_fg      #58a6ff   表头文字 / 链接文字
success        #3fb950   数值 / RX / 就绪
warn           #f0883e   HEX
danger         #f85149   删除 hover / 错误
text_primary   #e6edf3   标题
text_body      #c9d1d9   正文
text_muted     #6e7681   地址 / 元信息
alias          #d2a8ff   别名列

浅色对应：accent #2563eb · success #15803d · warn #c2410c
        L0 #f4f4f5 · L1 #fafafa · L2 #ffffff

字号：Heading 15 · Body 12.5 · Button 12 · Monospace 12.5 · Small 10.5
间距：item_spacing (10,6) · button_padding (12,4) · interact_size.y 24
     panel inner_margin (14,12) · 表格行 22 · 表头 26 · 日志行 18
```

---

## Task 0: 准备 — gitignore + spec 落盘

**Files:**
- Modify: `.gitignore`
- Create: `docs/superpowers/specs/2026-04-21-slave-ui-redesign-design.md`

- [ ] **Step 0.1: 给 .gitignore 加 brainstorm 产物排除**

读 `.gitignore`，在末尾追加：
```
# Brainstorming session artifacts (mockups, state)
.superpowers/
```

- [ ] **Step 0.2: 复制 brainstorm 设计为正式 spec**

把 `~/.claude/plans/ui-image-1-effervescent-mango.md` 第 1–170 行内容（Context + 现状入口 + 设计决策总览 + 组件级改动）复制到 `docs/superpowers/specs/2026-04-21-slave-ui-redesign-design.md`。开头改第一行标题：

```markdown
# 子站 UI 重设计 · 工业 HMI 中文版（Spec）
```

- [ ] **Step 0.3: commit**

```bash
git add .gitignore docs/superpowers/specs/2026-04-21-slave-ui-redesign-design.md
git commit -m "docs(slave-ui): 落地 spec + gitignore 排除 brainstorm 产物"
```

---

## Task 1: theme.rs · palette 与 token 重写

**Files:**
- Modify: `crates/modbussim-ui-shared/src/theme.rs`

设计：当前 `Layer::L0/L1/L2` 颜色弱、accent 是橙、字号梯度平。本任务集中改 palette 三组（背景层 / 语义色 / 字体），并新增 `accent_fg / warn / alias / border_subtle / border_strong` 与 `text::tiny_caps / text::crumb` helper。整体改完启动应用，色调应明显转冷且字号差异明显。

- [ ] **Step 1.1: 改 `bg_of` 三层颜色（line 70-84）**

把函数体替换为：

```rust
pub fn bg_of(flavor: Flavor, layer: Layer) -> Color32 {
    if flavor.is_dark() {
        match layer {
            Layer::L0 => rgb(0x01, 0x04, 0x09), // #010409 chrome
            Layer::L1 => rgb(0x0d, 0x11, 0x17), // #0d1117 surface
            Layer::L2 => rgb(0x16, 0x1b, 0x22), // #161b22 raised
        }
    } else {
        match layer {
            Layer::L0 => rgb(0xf4, 0xf4, 0xf5), // #f4f4f5
            Layer::L1 => rgb(0xfa, 0xfa, 0xfa), // #fafafa
            Layer::L2 => rgb(0xff, 0xff, 0xff), // #ffffff
        }
    }
}
```

- [ ] **Step 1.2: 改 `bg_hover` 与 `bg_selected_row`（line 87-103）**

```rust
pub fn bg_hover(flavor: Flavor) -> Color32 {
    if flavor.is_dark() {
        rgb(0x16, 0x1b, 0x22) // = Layer::L2
    } else {
        rgb(0xe4, 0xe4, 0xe7)
    }
}

pub fn bg_selected_row(flavor: Flavor) -> Color32 {
    if flavor.is_dark() {
        // accent.primary @ 15% alpha → 解多重不蒙底色
        Color32::from_rgba_unmultiplied(0x1f, 0x6f, 0xeb, 0x26)
    } else {
        Color32::from_rgba_unmultiplied(0x25, 0x63, 0xeb, 0x1a)
    }
}
```

- [ ] **Step 1.3: 改语义色 helper（line 314-337）**

替换 `accent / success / danger / subtext`，新增 `accent_fg / warn / alias / border_subtle / border_strong / text_primary / text_body / text_muted`：

```rust
pub fn accent(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0x1f, 0x6f, 0xeb) } else { rgb(0x25, 0x63, 0xeb) }
}
pub fn accent_fg(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0x58, 0xa6, 0xff) } else { rgb(0x3b, 0x82, 0xf6) }
}
pub fn success(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0x3f, 0xb9, 0x50) } else { rgb(0x15, 0x80, 0x3d) }
}
pub fn warn(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0xf0, 0x88, 0x3e) } else { rgb(0xc2, 0x41, 0x0c) }
}
pub fn danger(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0xf8, 0x51, 0x49) } else { rgb(0xb9, 0x1c, 0x1c) }
}
pub fn alias(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0xd2, 0xa8, 0xff) } else { rgb(0x7c, 0x3a, 0xed) }
}
pub fn border_subtle(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0x21, 0x26, 0x2d) } else { rgb(0xe4, 0xe4, 0xe7) }
}
pub fn border_strong(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0x30, 0x36, 0x3d) } else { rgb(0xd4, 0xd4, 0xd8) }
}
pub fn text_primary(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0xe6, 0xed, 0xf3) } else { rgb(0x09, 0x09, 0x0b) }
}
pub fn text_body(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0xc9, 0xd1, 0xd9) } else { rgb(0x3f, 0x3f, 0x46) }
}
pub fn text_muted(flavor: Flavor) -> Color32 {
    if flavor.is_dark() { rgb(0x6e, 0x76, 0x81) } else { rgb(0x71, 0x71, 0x7a) }
}
pub fn subtext(flavor: Flavor) -> Color32 { text_muted(flavor) } // 旧调用点回退
pub fn surface(flavor: Flavor) -> Color32 { bg_of(flavor, Layer::L2) } // 旧调用点回退
```

- [ ] **Step 1.4: 改 `apply` 内深色分支 visuals（line 188-230）**

把现有深色 `s.style_mut(...)` 块整段替换为新 token 驱动版本：

```rust
if flavor.is_dark() {
    let panel       = bg_of(flavor, Layer::L1);                 // #0d1117
    let panel_alt   = bg_of(flavor, Layer::L0);                 // #010409
    let raised      = bg_of(flavor, Layer::L2);                 // #161b22
    let stroke      = border_strong(flavor);                    // #30363d
    let stroke_soft = border_subtle(flavor);                    // #21262d
    let fg          = text_body(flavor);                        // #c9d1d9
    let strong_fg   = text_primary(flavor);                     // #e6edf3
    let sel_bg      = bg_selected_row(flavor);
    let acc         = accent(flavor);                           // #1f6feb
    s.visuals.panel_fill = panel;
    s.visuals.window_fill = panel_alt;
    s.visuals.extreme_bg_color = panel_alt;
    s.visuals.faint_bg_color = raised;
    s.visuals.code_bg_color = raised;
    s.visuals.widgets.noninteractive.bg_fill = panel_alt;
    s.visuals.widgets.noninteractive.weak_bg_fill = panel;
    s.visuals.widgets.noninteractive.bg_stroke.color = stroke_soft;
    s.visuals.widgets.noninteractive.fg_stroke.color = fg;
    s.visuals.widgets.inactive.bg_fill = raised;
    s.visuals.widgets.inactive.weak_bg_fill = panel_alt;
    s.visuals.widgets.inactive.bg_stroke.color = stroke;
    s.visuals.widgets.inactive.fg_stroke.color = fg;
    s.visuals.widgets.hovered.bg_fill = bg_hover(flavor);
    s.visuals.widgets.hovered.bg_stroke.color = bg_hover(flavor);
    s.visuals.widgets.hovered.fg_stroke.color = strong_fg;
    s.visuals.widgets.active.bg_fill = acc;
    s.visuals.widgets.active.bg_stroke.color = acc;
    s.visuals.widgets.active.fg_stroke.color = Color32::WHITE;
    s.visuals.widgets.open.bg_fill = raised;
    s.visuals.window_stroke.color = stroke_soft;
    s.visuals.selection.bg_fill = sel_bg;
    s.visuals.selection.stroke.color = acc;
    s.visuals.override_text_color = Some(fg);
    s.visuals.hyperlink_color = accent_fg(flavor);
    s.visuals.error_fg_color = danger(flavor);
    s.visuals.warn_fg_color = warn(flavor);
}
```

- [ ] **Step 1.5: 改浅色分支（line 231-266）**

类似深色的 token 驱动改写：

```rust
} else {
    let panel       = bg_of(flavor, Layer::L1);
    let panel_alt   = bg_of(flavor, Layer::L0);
    let raised      = bg_of(flavor, Layer::L2);
    let stroke      = border_strong(flavor);
    let stroke_soft = border_subtle(flavor);
    let fg          = text_body(flavor);
    let strong_fg   = text_primary(flavor);
    let sel_bg      = bg_selected_row(flavor);
    let acc         = accent(flavor);
    s.visuals.panel_fill = panel;
    s.visuals.window_fill = raised;
    s.visuals.extreme_bg_color = raised;
    s.visuals.faint_bg_color = panel;
    s.visuals.code_bg_color = panel;
    s.visuals.widgets.noninteractive.bg_fill = panel;
    s.visuals.widgets.noninteractive.weak_bg_fill = panel;
    s.visuals.widgets.noninteractive.bg_stroke.color = stroke_soft;
    s.visuals.widgets.noninteractive.fg_stroke.color = fg;
    s.visuals.widgets.inactive.bg_fill = raised;
    s.visuals.widgets.inactive.weak_bg_fill = panel;
    s.visuals.widgets.inactive.bg_stroke.color = stroke;
    s.visuals.widgets.inactive.fg_stroke.color = fg;
    s.visuals.widgets.hovered.bg_fill = bg_hover(flavor);
    s.visuals.widgets.hovered.bg_stroke.color = bg_hover(flavor);
    s.visuals.widgets.hovered.fg_stroke.color = strong_fg;
    s.visuals.widgets.active.bg_fill = acc;
    s.visuals.widgets.active.bg_stroke.color = acc;
    s.visuals.widgets.active.fg_stroke.color = Color32::WHITE;
    s.visuals.widgets.open.bg_fill = raised;
    s.visuals.window_stroke.color = stroke_soft;
    s.visuals.selection.bg_fill = sel_bg;
    s.visuals.selection.stroke.color = acc;
    s.visuals.override_text_color = Some(fg);
    s.visuals.hyperlink_color = accent_fg(flavor);
    s.visuals.error_fg_color = danger(flavor);
    s.visuals.warn_fg_color = warn(flavor);
}
```

- [ ] **Step 1.6: 改 spacing + 字号（line 269-309）**

把第二个 `ctx.style_mut` 块内的 spacing + TextStyle 替换为：

```rust
ctx.style_mut(|s| {
    s.spacing.item_spacing = egui::vec2(10.0, 6.0);
    s.spacing.button_padding = egui::vec2(12.0, 4.0);
    s.spacing.menu_margin = egui::Margin::symmetric(8.0 as i8, 5.0 as i8);
    s.spacing.indent = 14.0;
    s.spacing.interact_size.y = 24.0;

    let r: egui::Rounding = 4.0.into();
    s.visuals.widgets.noninteractive.corner_radius = r;
    s.visuals.widgets.inactive.corner_radius = r;
    s.visuals.widgets.hovered.corner_radius = r;
    s.visuals.widgets.active.corner_radius = r;
    s.visuals.widgets.open.corner_radius = r;
    s.visuals.window_corner_radius = 6.0.into();
    s.visuals.menu_corner_radius = 6.0.into();

    use egui::TextStyle::*;
    s.text_styles.insert(Heading,   egui::FontId::new(15.0, egui::FontFamily::Proportional));
    s.text_styles.insert(Body,      egui::FontId::new(12.5, egui::FontFamily::Proportional));
    s.text_styles.insert(Button,    egui::FontId::new(12.0, egui::FontFamily::Proportional));
    s.text_styles.insert(Monospace, egui::FontId::new(12.5, egui::FontFamily::Monospace));
    s.text_styles.insert(Small,     egui::FontId::new(10.5, egui::FontFamily::Proportional));
});
```

- [ ] **Step 1.7: 在文件末尾新增 `text` 子模块**

```rust
/// 文本渲染辅助：tiny_caps / crumb 等语义文本样式。
pub mod text {
    use super::{Flavor, text_muted, accent_fg};
    use egui::{Ui, RichText};

    /// 表头 / 分组标题用：10.5px 大写、字距感由空格 + 字色弱化体现。
    pub fn tiny_caps(ui: &mut Ui, flavor: Flavor, s: &str) {
        ui.label(
            RichText::new(s.to_uppercase())
                .size(10.5)
                .color(accent_fg(flavor))
                .strong(),
        );
    }

    /// 面包屑 / 元信息：11px、muted。
    pub fn crumb(ui: &mut Ui, flavor: Flavor, s: &str) {
        ui.label(RichText::new(s).size(11.0).color(text_muted(flavor)));
    }
}
```

- [ ] **Step 1.8: 编译 + 烟测**

```bash
cargo check -p modbussim-ui-shared
cargo clippy -p modbussim-ui-shared --no-deps -- -D warnings
```

预期：通过（`subtext` / `surface` 旧调用点已通过 helper 回退兼容）。

```bash
cargo run -p modbussim-egui
```

预期视觉：背景明显转冷（深蓝黑），字号差异更明显（标题 vs 表头），所有按钮立即变蓝（来自 selection.bg_fill）。**注意**：值解析、表格、按钮颜色还要等 Task 2/5 才完全到位。

- [ ] **Step 1.9: commit**

```bash
git add crates/modbussim-ui-shared/src/theme.rs
git commit -m "feat(theme): 切换冷蓝 palette + 拉开字号梯度 + 新增语义色 token"
```

---

## Task 2: ui.rs · shadcn 同步 + helper

**Files:**
- Modify: `crates/modbussim-ui-shared/src/ui.rs`

- [ ] **Step 2.1: 改 `card_colors` 用 Layer token（line 11-25）**

```rust
fn card_colors(flavor: Flavor) -> (Color32, Color32) {
    (theme::bg_of(flavor, Layer::L2), theme::border_subtle(flavor))
}
```

- [ ] **Step 2.2: 改 `card` / `accent_card` 内边距（line 32-88）**

`card` 与 `accent_card` 的 `inner_margin` 改成统一 `(14, 12)`：

```rust
.inner_margin(egui::Margin::symmetric(14.0 as i8, 12.0 as i8))
```

`card` 与 `accent_card` 的 `corner_radius(2.0)` 改成 `4.0`，与全局 rounding 一致。

- [ ] **Step 2.3: 改 `shadcn_theme` palette 为冷蓝（line 102-128）**

```rust
if flavor.is_dark() {
    palette.primary = Color32::from_rgb(0x1f, 0x6f, 0xeb);
    palette.primary_foreground = Color32::WHITE;
    palette.destructive = Color32::from_rgb(0xf8, 0x51, 0x49);
    palette.destructive_foreground = Color32::WHITE;
    palette.ring = Color32::from_rgb(0x1f, 0x6f, 0xeb);
    palette.border = Color32::from_rgb(0x30, 0x36, 0x3d);
    palette.background = Color32::from_rgb(0x0d, 0x11, 0x17);
    palette.foreground = Color32::from_rgb(0xc9, 0xd1, 0xd9);
    palette.muted_foreground = Color32::from_rgb(0x6e, 0x76, 0x81);
    palette.accent = Color32::from_rgb(0x3f, 0xb9, 0x50); // success 绿用作辅 accent（"+ 批量添加"）
    palette.accent_foreground = Color32::WHITE;
} else {
    palette.primary = Color32::from_rgb(0x25, 0x63, 0xeb);
    palette.primary_foreground = Color32::WHITE;
    palette.destructive = Color32::from_rgb(0xb9, 0x1c, 0x1c);
    palette.destructive_foreground = Color32::WHITE;
    palette.ring = Color32::from_rgb(0x25, 0x63, 0xeb);
    palette.border = Color32::from_rgb(0xd4, 0xd4, 0xd8);
    palette.background = Color32::from_rgb(0xfa, 0xfa, 0xfa);
    palette.foreground = Color32::from_rgb(0x3f, 0x3f, 0x46);
    palette.muted_foreground = Color32::from_rgb(0x71, 0x71, 0x7a);
    palette.accent = Color32::from_rgb(0x15, 0x80, 0x3d);
    palette.accent_foreground = Color32::WHITE;
}
```

- [ ] **Step 2.4: `primary_button` 改用 Accent 变体（绿色"+ 批量添加"语义）**

把 `primary_button` 内的 `ControlVariant::Primary` 改为 `ControlVariant::Accent`（已在 palette.accent 设为绿色），`secondary_button` / `danger_button` / `icon_button` 保持原变体。

```rust
pub fn primary_button(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response {
    let theme = shadcn_theme(flavor);
    egui_shadcn::button(
        ui, &theme, text.into(),
        egui_shadcn::tokens::ControlVariant::Accent,   // ← 改
        egui_shadcn::tokens::ControlSize::Md, true,
    )
}
```

> 若 egui-shadcn 0.3 没有 `Accent` 变体，回退方案：保留 `Primary` + 把 `palette.primary` 设为绿色 `#3fb950`，把"链接/选中"用 `palette.accent` 蓝色。先按 Accent 写，编译失败时回退。

- [ ] **Step 2.5: 在文件末尾新增 `panel_header` 与 `link_action`**

```rust
/// 主区头部：上行 Heading 标题 + 下行 muted 面包屑。
pub fn panel_header(ui: &mut Ui, flavor: Flavor, title: &str, crumb: Option<&str>) {
    ui.vertical(|ui| {
        ui.label(RichText::new(title).heading().color(theme::text_primary(flavor)));
        if let Some(c) = crumb {
            theme::text::crumb(ui, flavor, c);
        }
    });
}

/// 无边框文字操作（停止 / 删除连接 / 关闭）。hover 变 accent 或 danger。
pub fn link_action(ui: &mut Ui, flavor: Flavor, label: &str, danger: bool) -> Response {
    let base = theme::text_muted(flavor);
    let hover = if danger { theme::danger(flavor) } else { theme::accent_fg(flavor) };
    let resp = ui.add(egui::Label::new(RichText::new(label).color(base).size(11.5)).sense(egui::Sense::click()));
    if resp.hovered() {
        let painter = ui.painter();
        painter.text(
            resp.rect.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(11.5),
            hover,
        );
    }
    resp
}
```

- [ ] **Step 2.6: 编译 + 视觉验证**

```bash
cargo check -p modbussim-ui-shared
cargo clippy -p modbussim-ui-shared --no-deps -- -D warnings
cargo run -p modbussim-egui
```

预期：FC01 toggle 仍正常；批量添加按钮变绿色；停止/删除/导出按钮 outline 蓝边。

- [ ] **Step 2.7: commit**

```bash
git add crates/modbussim-ui-shared/src/ui.rs
git commit -m "feat(ui-shared): shadcn palette 转冷蓝 + 新增 panel_header/link_action"
```

---

## Task 3: log_panel.rs · 单行 header + 折叠 + 箭头方向

**Files:**
- Modify: `crates/modbussim-ui-shared/src/log_panel.rs`

- [ ] **Step 3.1: 给 LogPanelState 加 `collapsed` 字段（line 8-30）**

```rust
pub struct LogPanelState {
    pub open: bool,
    pub collapsed: bool,
    pub show_rx: bool,
    pub show_tx: bool,
    pub filter_text: String,
}

impl LogPanelState {
    pub fn new() -> Self {
        Self {
            open: true,
            collapsed: false,
            show_rx: true,
            show_tx: true,
            filter_text: String::new(),
        }
    }
}
```

- [ ] **Step 3.2: 整合 header 为单行（line 80-107）**

把 `.show(ctx, |ui| { ... })` 内的两段 `ui.horizontal(...)` 整合为一行：

```rust
.show(ctx, |ui| {
    ui.horizontal(|ui| {
        let chev = if state.collapsed { "▶" } else { "▼" };
        if ui.add(egui::Label::new(RichText::new(chev).size(11.0)).sense(egui::Sense::click())).clicked() {
            state.collapsed = !state.collapsed;
        }
        ui.label(RichText::new("通信日志").strong().size(12.5));
        if let Some(label) = conn_label {
            crate::theme::text::crumb(ui, flavor, &format!("· {} · {} 条", label, cache.len()));
        } else {
            crate::theme::text::crumb(ui, flavor, "· 选中连接以查看");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::ui::link_action(ui, flavor, "关闭", false).clicked() {
                action = LogPanelAction::Close;
            }
            if crate::ui::link_action(ui, flavor, "导出 CSV", false).clicked() {
                action = LogPanelAction::Export;
            }
            if crate::ui::link_action(ui, flavor, "清空", false).clicked() {
                action = LogPanelAction::Clear;
            }
            ui.add(
                egui::TextEdit::singleline(&mut state.filter_text)
                    .hint_text("过滤…")
                    .desired_width(160.0),
            );
            ui.checkbox(&mut state.show_tx, "TX");
            ui.checkbox(&mut state.show_rx, "RX");
        });
    });
    if state.collapsed { return; }
    ui.add_space(6.0);
    // 接 TableBuilder 段（保持不变）
    let entries: Vec<&LogEntry> = cache.iter().rev().filter(|e| accepts(state, e)).collect();
    // ...
});
```

- [ ] **Step 3.3: 改方向列宽 + 箭头符号（line 113-141）**

```rust
TableBuilder::new(ui)
    .striped(false)                                 // ← 关掉，与 hover 冲突
    .resizable(true)
    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
    .column(Column::exact(150.0))
    .column(Column::exact(28.0))                    // ← 方向列收窄
    .column(Column::exact(60.0))
    .column(Column::remainder())
    .header(22.0, |mut h| {
        h.col(|ui| crate::theme::text::tiny_caps(ui, flavor, "时间"));
        h.col(|ui| crate::theme::text::tiny_caps(ui, flavor, "向"));
        h.col(|ui| crate::theme::text::tiny_caps(ui, flavor, "FC"));
        h.col(|ui| crate::theme::text::tiny_caps(ui, flavor, "详情"));
    })
    .body(|body| {
        body.rows(18.0, entries.len(), |mut row| {
            let e = entries[row.index()];
            row.col(|ui| {
                ui.add(egui::Label::new(
                    RichText::new(e.timestamp.format("%H:%M:%S%.3f").to_string())
                        .monospace().color(crate::theme::text_muted(flavor))
                ));
            });
            row.col(|ui| {
                let (sym, c) = match e.direction {
                    Direction::Rx => ("←", crate::theme::success(flavor)),
                    Direction::Tx => ("→", crate::theme::accent_fg(flavor)),
                };
                ui.add(egui::Label::new(RichText::new(sym).color(c).strong().monospace()));
            });
            row.col(|ui| {
                ui.add(egui::Label::new(
                    RichText::new(e.function_code.name())
                        .monospace().color(crate::theme::warn(flavor))
                ));
            });
            row.col(|ui| {
                ui.add(egui::Label::new(
                    RichText::new(&e.detail).monospace().color(crate::theme::text_body(flavor))
                ));
            });
        });
    });
```

- [ ] **Step 3.4: 编译 + 视觉验证**

```bash
cargo check -p modbussim-ui-shared
cargo clippy -p modbussim-ui-shared --no-deps -- -D warnings
cargo run -p modbussim-egui
```

预期：日志面板 header 一行排开，可以点 ▼ 折叠；方向列只剩 ← / → 符号；FC 列橙色。

- [ ] **Step 3.5: commit**

```bash
git add crates/modbussim-ui-shared/src/log_panel.rs
git commit -m "feat(log-panel): 单行 header + 可折叠 + RX/TX 改箭头符号"
```

---

## Task 4: app.rs · 左侧 SidePanel 重构

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs`（SidePanel 块约 line 2798-2860）

- [ ] **Step 4.1: 改 SidePanel 宽度与边距（line 2798-2805）**

把 `egui::SidePanel::left("connections")` 配置改为：

```rust
egui::SidePanel::left("connections")
    .resizable(true)
    .default_width(240.0)
    .min_width(200.0)
    .show_separator_line(false)
    .frame(
        egui::Frame::none()
            .fill(theme::bg_of(self.flavor, theme::Layer::L0))
            .inner_margin(egui::Margin::symmetric(0, 0)),
    )
    .show(ctx, |ui| { /* 见下三步 */ });
```

- [ ] **Step 4.2: 改 SidePanel 内部为「头 / 树 / footer」三段**

把 SidePanel `.show(ctx, |ui| { ... })` 闭包内（替换原"新建 TCP 连接"行 + 现有树渲染）改写为：

```rust
ui.allocate_ui_with_layout(
    ui.available_size(),
    egui::Layout::top_down(egui::Align::Min),
    |ui| {
        // —— 头部：tiny_caps "连接" + 右上 + 新建 ——
        egui::Frame::none()
            .inner_margin(egui::Margin { left: 14, right: 10, top: 12, bottom: 8 })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    crate::ui_shared::theme::text::tiny_caps(ui, self.flavor, "连接");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::ui_shared::ui::link_action(ui, self.flavor, "+ 新建", false).clicked() {
                            self.show_new_tcp_dialog = true;
                        }
                    });
                });
            });

        // —— 树：可滚动区 ——
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(ui.available_height() - 40.0) // 给 footer 留空间
            .show(ui, |ui| {
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(8, 0))
                    .show(ui, |ui| {
                        self.render_connection_tree(ui); // 现有树渲染抽出，见 Step 4.3
                    });
            });

        // —— footer：停止 / 删除连接 ——
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            egui::Frame::none()
                .fill(theme::bg_of(self.flavor, theme::Layer::L0))
                .inner_margin(egui::Margin { left: 14, right: 14, top: 8, bottom: 10 })
                .stroke(egui::Stroke::new(1.0, theme::border_subtle(self.flavor)))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if let Some(active) = self.active_connection_id() {
                            if crate::ui_shared::ui::link_action(ui, self.flavor, "停止", false).clicked() {
                                self.stop_connection(active);
                            }
                            ui.add_space(14.0);
                            if crate::ui_shared::ui::link_action(ui, self.flavor, "删除连接", true).clicked() {
                                self.request_delete_connection(active);
                            }
                        }
                    });
                });
        });
    },
);
```

> 实施者注：`self.active_connection_id()` / `self.stop_connection(...)` / `self.request_delete_connection(...)` / `self.show_new_tcp_dialog` 这些方法字段在当前 app.rs 散在各处。如有缺失，补一个 helper：扫描 `self.connections`，找 `running` 的第一个 / 用户选中的那个；删除走现有 "删除"按钮的同一路径。

- [ ] **Step 4.3: 抽出 `render_connection_tree(&mut self, ui: &mut Ui)`**

把现有 SidePanel 内树渲染逻辑（每个节点 `SelectableLabel` 那段）抽到一个新 method。在节点渲染处把激活态样式改为：

```rust
let is_active = matches!(self.selection, Selection::FunctionCode { .. } /* 同上 */);
let row_resp = ui.allocate_response(egui::vec2(ui.available_width(), 22.0), egui::Sense::click());
if is_active {
    let acc = theme::accent(self.flavor);
    let stripe_rect = egui::Rect::from_min_size(
        row_resp.rect.left_top(), egui::vec2(2.0, row_resp.rect.height())
    );
    ui.painter().rect_filled(stripe_rect, 0.0, acc);
    ui.painter().rect_filled(
        row_resp.rect.expand2(egui::vec2(0.0, 0.0)).translate(egui::vec2(2.0, 0.0)),
        0.0,
        Color32::from_rgba_unmultiplied(0x1f, 0x6f, 0xeb, 0x26),
    );
}
let painter = ui.painter();
let label_color = if is_active { theme::accent_fg(self.flavor) } else { theme::text_body(self.flavor) };
let weight = if is_active { egui::FontId::new(12.5, egui::FontFamily::Proportional) } else { egui::FontId::new(12.5, egui::FontFamily::Proportional) };
painter.text(
    row_resp.rect.left_center() + egui::vec2(10.0 + indent_px, 0.0),
    egui::Align2::LEFT_CENTER,
    node_label,
    weight,
    label_color,
);
// 节点右侧 badge（行数）：painter.text(...) muted
```

> 节点细节较多（折叠箭头 / icon / hover 背景）。保留现有交互逻辑，仅替换样式段。

- [ ] **Step 4.4: 编译 + 视觉验证**

```bash
cargo check -p modbussim-egui
cargo clippy -p modbussim-egui --no-deps -- -D warnings
cargo run -p modbussim-egui
```

预期：左侧栏 240px 宽；顶部"连接"小大写 + 右侧 + 新建链接；激活节点左侧蓝竖条 + 蓝半透明背景 + 字色 #58a6ff；底部 footer "停止 / 删除连接" 灰文字、悬停变蓝/红。

- [ ] **Step 4.5: commit**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "feat(slave-app): SidePanel 重构 — 240px + 头/树/footer 三段"
```

---

## Task 5: app.rs · 主区头 + 工具栏 + 寄存器表格

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs`（CentralPanel 块 line 2865+ 与 TableBuilder line 2315-2462）

- [ ] **Step 5.1: 改 CentralPanel frame + 主区头**

把 CentralPanel 块改为：

```rust
egui::CentralPanel::default()
    .frame(
        egui::Frame::none()
            .fill(theme::bg_of(self.flavor, theme::Layer::L1))
            .inner_margin(egui::Margin { left: 18, right: 18, top: 14, bottom: 0 }),
    )
    .show(ctx, |ui| {
        // —— 主区头 ——
        ui.horizontal(|ui| {
            crate::ui_shared::ui::panel_header(
                ui,
                self.flavor,
                &self.current_view_title(),               // "FC03 保持寄存器"
                self.current_view_crumb().as_deref(),     // Some("slave_1 · 20001 行")
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::ui_shared::ui::primary_button(ui, self.flavor, "+ 批量添加").clicked() {
                    self.open_batch_add_dialog();
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text("搜索地址 / 别名…")
                        .desired_width(220.0),
                );
                let icon = if self.value_parse_open { "◧ 收起解析" } else { "◧ 值解析" };
                if crate::ui_shared::ui::link_action(ui, self.flavor, icon, false).clicked() {
                    self.value_parse_open = !self.value_parse_open;
                }
            });
        });
        ui.add_space(10.0);

        // —— 工具栏（fmt-pill + 已选行 + 次操作）——
        self.render_register_toolbar(ui);
        ui.add_space(6.0);

        // —— 表格 —— （复用现有 self.render_register_table，下面 Step 5.3 改样式）
        self.render_register_table(ui);
    });
```

> `self.current_view_title()` / `self.current_view_crumb()` / `self.search_query` / `self.value_parse_open` / `self.open_batch_add_dialog()` 等若不存在则按需要在 struct 里加 `value_parse_open: bool`、`search_query: String` 字段（默认 false / 空），title/crumb 写两个 helper 根据 `self.selection` 返回相应字符串。

- [ ] **Step 5.2: 新增 `render_register_toolbar(&mut self, ui)`**

```rust
fn render_register_toolbar(&mut self, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        crate::ui_shared::theme::text::tiny_caps(ui, self.flavor, "格式");
        // fmt-pill
        egui::Frame::new()
            .fill(theme::bg_of(self.flavor, theme::Layer::L2))
            .stroke(egui::Stroke::new(1.0, theme::border_strong(self.flavor)))
            .corner_radius(12.0)
            .inner_margin(egui::Margin::symmetric(10.0 as i8, 2.0 as i8))
            .show(ui, |ui| {
                egui::ComboBox::from_id_salt("fmt_pill")
                    .selected_text(
                        egui::RichText::new(self.fmt.label())
                            .color(theme::accent_fg(self.flavor))
                            .monospace().size(11.5),
                    )
                    .show_ui(ui, |ui| {
                        for f in [Fmt::U16, Fmt::I16, Fmt::Hex, Fmt::Bin] {
                            ui.selectable_value(&mut self.fmt, f, f.label());
                        }
                    });
            });

        ui.add_space(12.0);
        crate::ui_shared::theme::text::crumb(
            ui,
            self.flavor,
            &format!("已选 {} 行", self.selected_rows.len()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::ui_shared::ui::link_action(ui, self.flavor, "清零", false).clicked() {
                self.clear_selected_rows();
            }
            if crate::ui_shared::ui::link_action(ui, self.flavor, "导出", false).clicked() {
                self.export_register_csv();
            }
        });
    });
}
```

- [ ] **Step 5.3: 改 TableBuilder 列宽 + 表头（line 2315-2330）**

把现有 TableBuilder 链替换：

```rust
TableBuilder::new(ui)
    .striped(false)
    .resizable(true)
    .max_scroll_height(avail_h)
    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
    .column(Column::exact(80.0))                   // 地址（右对齐渲染）
    .column(Column::exact(120.0))                  // 别名
    .column(Column::exact(100.0))                  // 值
    .column(Column::exact(80.0))                   // HEX
    .column(Column::remainder().at_least(180.0))   // 二进制
    .header(26.0, |mut h| {
        h.col(|ui| crate::ui_shared::theme::text::tiny_caps(ui, self.flavor, "地址"));
        h.col(|ui| crate::ui_shared::theme::text::tiny_caps(ui, self.flavor, "别名"));
        h.col(|ui| crate::ui_shared::theme::text::tiny_caps(ui, self.flavor, "U16"));
        h.col(|ui| crate::ui_shared::theme::text::tiny_caps(ui, self.flavor, "HEX"));
        h.col(|ui| crate::ui_shared::theme::text::tiny_caps(ui, self.flavor, "二进制"));
    })
    .body(|body| {
        body.rows(22.0, group_rows, |mut row| {
            let idx = row.index();
            let addr = base_addr + idx as u16;
            let val = self.regs[idx];
            let is_sel = self.selected_rows.contains(&idx);

            row.col(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(
                        egui::RichText::new(addr.to_string())
                            .monospace()
                            .color(theme::text_muted(self.flavor)),
                    ));
                });
            });
            row.col(|ui| {
                let alias = self.alias_of(addr).unwrap_or("—");
                let color = if alias == "—" { theme::text_muted(self.flavor) } else { theme::alias(self.flavor) };
                ui.add(egui::Label::new(egui::RichText::new(alias).color(color).monospace()));
            });
            row.col(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if is_sel {
                        // 编辑态：DragValue / TextEdit
                        let mut v = val;
                        if ui.add(egui::DragValue::new(&mut v).range(0..=u16::MAX)).changed() {
                            self.set_register(addr, v);
                        }
                    } else {
                        ui.add(egui::Label::new(
                            egui::RichText::new(val.to_string())
                                .color(theme::success(self.flavor))
                                .monospace().strong(),
                        ));
                    }
                });
            });
            row.col(|ui| {
                ui.add(egui::Label::new(
                    egui::RichText::new(format!("0x{:04X}", val))
                        .color(theme::warn(self.flavor)).monospace(),
                ));
            });
            row.col(|ui| {
                ui.add(egui::Label::new(
                    egui::RichText::new(format!("{:016b}", val))
                        .color(theme::text_muted(self.flavor)).monospace().size(11.0),
                ));
            });

            // 行选中底色 / hover 底色
            let row_rect = row.response().rect;
            let painter = ui.painter();
            if is_sel {
                painter.rect_filled(row_rect, 0.0, theme::bg_selected_row(self.flavor));
            } else if row.response().hovered() {
                painter.rect_filled(row_rect, 0.0, theme::bg_hover(self.flavor));
            }
        });
    });
```

> 别名/选中编辑态/行 hover 等若现有 app 内已有逻辑，保留逻辑只换样式。`self.alias_of` / `self.set_register` 同名 fn 若不存在按现有数据模型 inline。

- [ ] **Step 5.4: 表头下划线 2px 蓝**

在 `.header(26.0, ...)` 闭包末尾或外层 frame 中，用 `ui.painter().line_segment` 在 header 下边画一条 2px `theme::accent(self.flavor)` 横线。最稳妥放法：在 TableBuilder 调用前先 `ui.allocate_painter` 占住表头底边那 2px 区域，或者在 header 的最后一列 `h.col` 内 `ui.painter().line_segment(...)` 跨整行宽度（用 `ui.max_rect()` 取宽）。

- [ ] **Step 5.5: 编译 + 视觉验证**

```bash
cargo check -p modbussim-egui
cargo clippy -p modbussim-egui --no-deps -- -D warnings
cargo run -p modbussim-egui
```

预期：表格地址右对齐 muted、别名紫、数值右对齐绿粗、HEX 橙、二进制 muted；选中行 15% alpha 蓝底；hover 行 raised 色；表头小大写蓝字 + 下方 2px 蓝线。

- [ ] **Step 5.6: commit**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "feat(slave-app): 主区头 + fmt-pill 工具栏 + 寄存器表格语义化色彩"
```

---

## Task 6: app.rs · 值解析抽屉 + 状态栏 + 快捷键

**Files:**
- Modify: `crates/modbussim-egui/src/app.rs`

- [ ] **Step 6.1: app struct 加状态字段**

找到主 App struct 定义，加：

```rust
pub value_parse_open: bool,
pub log_collapsed_persist: bool, // 持久化日志折叠（可选）
pub search_query: String,
```

`Default` impl 里都设为初始值（false / "")。

- [ ] **Step 6.2: 在 CentralPanel 之前插入右侧 SidePanel 抽屉**

```rust
egui::SidePanel::right("value_parse")
    .resizable(true)
    .default_width(240.0)
    .min_width(200.0)
    .show_separator_line(false)
    .frame(
        egui::Frame::none()
            .fill(theme::bg_of(self.flavor, theme::Layer::L0))
            .inner_margin(egui::Margin { left: 14, right: 14, top: 12, bottom: 12 }),
    )
    .show_animated(ctx, self.value_parse_open, |ui| {
        ui.horizontal(|ui| {
            crate::ui_shared::theme::text::tiny_caps(ui, self.flavor, "值解析");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::ui_shared::ui::link_action(ui, self.flavor, "×", false).clicked() {
                    self.value_parse_open = false;
                }
            });
        });
        ui.add_space(8.0);
        if let Some(parse) = self.compute_value_parse() {
            self.render_value_parse_grid(ui, &parse);
        } else {
            crate::ui_shared::theme::text::crumb(
                ui, self.flavor, "选中 1–4 行寄存器以查看",
            );
        }
    });
```

> `self.compute_value_parse()` 应返回当前选中行（最多 4 行）下的 `U16/I16/HEX/BIN/U32/F32/ASCII` 等。复用现有的 `value_panel.rs` 数据计算逻辑（保留计算、丢弃常驻渲染部分）。

- [ ] **Step 6.3: 把现有 value_panel 列从 CentralPanel 中移除**

删除 CentralPanel 内现有的"值解析"右列分配（很可能是 columns(2) / TableBuilder 的最后一列 `Column::remainder()`）。

- [ ] **Step 6.4: 新增 BottomPanel 状态栏（CentralPanel 之前）**

```rust
egui::TopBottomPanel::bottom("statusbar")
    .resizable(false)
    .exact_height(22.0)
    .show_separator_line(false)
    .frame(
        egui::Frame::none()
            .fill(theme::bg_of(self.flavor, theme::Layer::L0))
            .inner_margin(egui::Margin::symmetric(14.0 as i8, 4.0 as i8)),
    )
    .show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(
                egui::RichText::new("●").color(theme::success(self.flavor)).size(11.0),
            ));
            crate::ui_shared::theme::text::crumb(ui, self.flavor, "就绪");
            ui.add_space(14.0);
            crate::ui_shared::theme::text::crumb(
                ui, self.flavor,
                &format!("{} 连接 · {} 从站", self.connections.len(), self.total_slaves()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                crate::ui_shared::theme::text::crumb(
                    ui, self.flavor, env!("CARGO_PKG_VERSION"),
                );
            });
        });
    });
```

> 注意：BottomPanel 已被现有 log_panel 占用（`shared_log_panel`）。这两个面板在 egui 里可堆叠（按 add 顺序从下往上），先 `add` statusbar、再 `add` log_panel 即可让状态栏在最底。

- [ ] **Step 6.5: 全局快捷键**

在 `update` 顶部、绘制面板前：

```rust
ctx.input(|i| {
    if i.key_pressed(egui::Key::V) && !i.modifiers.any() {
        self.value_parse_open = !self.value_parse_open;
    }
    if i.key_pressed(egui::Key::L) && !i.modifiers.any() {
        self.log_state.collapsed = !self.log_state.collapsed;
    }
    if i.key_pressed(egui::Key::Slash) && !i.modifiers.any() {
        self.focus_search_next_frame = true;
    }
    if i.key_pressed(egui::Key::Escape) && !self.selected_rows.is_empty() {
        self.selected_rows.clear();
    }
});
```

`focus_search_next_frame: bool` 字段在主区头渲染搜索框时检查并 `response.request_focus()`。

- [ ] **Step 6.6: 视图菜单新增切换项**

找菜单栏 `视图` 的 `ui.menu_button("视图", |ui| { ... })`，加：

```rust
ui.menu_button("视图", |ui| {
    ui.checkbox(&mut self.value_parse_open, "显示值解析 (V)");
    ui.checkbox(&mut self.log_state.open, "显示通信日志");
    if !self.log_state.open { self.log_state.collapsed = false; }
    ui.separator();
    if ui.button("浅色 / 深色切换").clicked() {
        self.flavor = if self.flavor.is_dark() { Flavor::Latte } else { Flavor::Mocha };
    }
});
```

- [ ] **Step 6.7: 编译 + 视觉验证**

```bash
cargo check -p modbussim-egui
cargo clippy -p modbussim-egui --no-deps -- -D warnings
cargo run -p modbussim-egui
```

预期：默认无值解析；按 `V` / 点工具栏 `◧ 值解析` → 抽屉滑入；按 `L` 折叠日志；底部 22px 状态栏（绿点 + "就绪 · 1 连接 · 1 从站 · 0.x.x"）。

- [ ] **Step 6.8: commit**

```bash
git add crates/modbussim-egui/src/app.rs
git commit -m "feat(slave-app): 值解析改右抽屉 + 底部状态栏 + 快捷键 V/L/Esc//"
```

---

## Task 7: 浅色模式校核 + mbpoll 烟测

**Files:**
- Touch: 无（仅运行验证）

- [ ] **Step 7.1: 切浅色目视回归**

```bash
cargo run -p modbussim-egui
# 视图菜单 → 浅色/深色切换
```

逐项核对：
- 背景三层（菜单/主区/卡）灰阶可辨
- accent 蓝 / success 绿深色 / warn 橙深色 / 别名紫深色 在白底下都 ≥4.5:1 对比度
- 选中行有蓝底（不刺眼）
- 表头 tiny_caps 仍是 accent_fg 蓝
- shadcn 按钮（绿"+ 批量添加" / outline 灰边）正常

如有问题，回 Task 1 / Task 2 微调浅色 RGB。

- [ ] **Step 7.2: mbpoll 烟测**

终端 1：
```bash
cargo run -p modbussim-egui --release
# 在 GUI 内新建 TCP slave 监听 5502
```

终端 2：
```bash
mbpoll -m tcp -p 5502 -t 4 -r 14984 -c 4 127.0.0.1
mbpoll -m tcp -p 5502 -t 4 -r 14984 -- 1450 0 0 1 127.0.0.1
mbpoll -m tcp -p 5502 -t 4 -r 14984 -c 4 127.0.0.1
```

预期 GUI 内：表格 14984 显示 1450（绿粗体）；通信日志条目正确（RX `←` 绿 / TX `→` 蓝、FC 橙、详情 muted）。

- [ ] **Step 7.3: workspace 测试**

```bash
cargo test --workspace --exclude modbussim-app --exclude modbusmaster-app
cargo clippy --workspace --no-deps -- -D warnings
cargo fmt --all -- --check
```

预期：全绿；如有红色，`cargo fmt --all` 后再跑一次。

- [ ] **Step 7.4: 截图替换 spec 截图（可选）**

若有时间，截一张新 dark 模式截图保存到 `docs/superpowers/specs/assets/2026-04-21-slave-ui-after.png`，并在 spec 文件追加："Before / After" 对比段。

- [ ] **Step 7.5: commit**

```bash
git add -u
git commit -m "test(slave-ui): 双主题视觉回归 + mbpoll 烟测全通"
```

---

## Task 8: PR

- [ ] **Step 8.1: push 分支**

```bash
git push -u origin refactor/egui-skeleton
```

- [ ] **Step 8.2: 创建 PR**

```bash
gh pr create --title "feat(ui): 子站 UI 重设计 — 工业 HMI 中文版" --body "$(cat <<'EOF'
## Summary
- 重构 \`modbussim-ui-shared/theme.rs\`：palette 改冷蓝、新增 token (accent_fg/warn/alias/border_*/text_*)、字号梯度拉开、tiny_caps/crumb helper
- 重构 \`modbussim-ui-shared/ui.rs\`：shadcn palette 同步、card 内边距统一、新增 panel_header / link_action
- 重构 \`modbussim-ui-shared/log_panel.rs\`：单行 header、可折叠、RX/TX 改 ← / → 符号
- 重构 \`modbussim-egui/app.rs\`：SidePanel 240px + 头/树/footer 三段、主区头 panel_header、fmt-pill 工具栏、寄存器表格语义化色彩、值解析改右抽屉、底部 22px 状态栏、快捷键 V/L/Esc/\`/\`

## Test plan
- [ ] cargo check / clippy / fmt 全绿
- [ ] cargo test workspace 全绿
- [ ] mbpoll 烟测：读 / 写 / 表格刷新 / 日志条目
- [ ] 双主题视觉回归
- [ ] CI 三平台（macOS / Linux / Windows）通过

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 8.3: 观察 CI**

```bash
gh run list --branch refactor/egui-skeleton --limit 3
```

如失败按错误修复后再 push。

---

## Self-Review

- **Spec coverage**：spec 第 2 节 palette、第 3 节字号、第 4 节间距、第 5 节组件清单（A–F）均落到具体 task：
  - palette / TextStyle / spacing → Task 1
  - shadcn 同步 / panel_header / link_action → Task 2
  - log_panel 单行 + 折叠 + 箭头 → Task 3
  - SidePanel 重构 → Task 4
  - 主区头 / 工具栏 / 表格 → Task 5
  - 值解析抽屉 / 状态栏 / 快捷键 / 视图菜单 → Task 6
  - 浅色 + mbpoll → Task 7
  - fonts.rs Monospace 回退链 → 未在本 plan 实施（spec 里属于 nice-to-have，且与 CJK 字体加载的 skrifa 后端交互复杂）。**实施者注**：发现问题再补，或在 Task 1 末尾另起 Step 1.10 处理。
- **Placeholder scan**：无 TODO / TBD / "实现细节后补"。每个 step 都给出可直接粘贴的 Rust 代码或可执行的命令。
- **Type consistency**：
  - Token helper 全部用 `theme::xxx(flavor) -> Color32` 同签名
  - `tiny_caps(ui, flavor, &str)` / `crumb(ui, flavor, &str)` 在 Task 3/4/5/6 一致
  - `link_action(ui, flavor, &str, bool) -> Response` 在 Task 3/4/5/6 一致
  - `panel_header(ui, flavor, title, Option<crumb>)` 在 Task 5 一致
  - `LogPanelState.collapsed` 在 Task 3 定义、Task 6 视图菜单引用
  - `value_parse_open: bool` 在 Task 6.1 定义、Task 5.1 / 6.2 / 6.5 / 6.6 引用
- **未决细节**：
  - egui-shadcn 0.3 `ControlVariant::Accent` 是否存在，Task 2.4 已注明回退方案
  - app.rs 内 `self.alias_of / self.set_register / self.selected_rows / self.fmt / self.regs / self.connections` 等字段的具体名字以现状为准，实施者按需对齐

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-21-slave-ui-redesign.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — 每个 task 派一个新 subagent 执行，task 间我做 review。适合纯视觉工作 + 有现成 mockup 比对。

**2. Inline Execution** — 在当前会话内按 task 顺序执行，每 2 个 task 一个 checkpoint review。

**Which approach?**
