## 上下文

`visual-flat-layered-v2` + `bool-view-and-button-polish` 两轮自研视觉迭代后，用户仍反馈按钮"丑"。这是 self-driven CSS 式调整无法跨越的 aesthetic gap：一套**完整的** design system（token 体系 + 变体体系 + interaction 动画 + accessibility）不是一次调整能做出来的。

egui 生态里 `egui-shadcn` 把 shadcn/ui 完整 port 到 egui。但它要求 egui 0.33 (我们锁 0.29)。同时：
- `egui-phosphor` 0.11 兼容 0.33，0.12 绑 0.34 (跳过 0.33 版本)
- `catppuccin-egui` 5.7 有 `egui33` feature flag
- `egui-modal` + `egui-toast` 是 Cargo.toml 里声明但代码未用的死依赖 —— `grep` 证实
- `egui_extras` 有 0.33.3 官方版

一次升级打包三件事：**视觉 design system 升级 + 死依赖清理 + egui 生态对齐**。

## 目标 / 非目标

**目标：**
- 升 egui 生态到 **0.33.3**（非 0.34，那会踩 shadcn 兼容坑）
- 引入 `egui-shadcn` 0.3+ 作为 Button + Switch 的底层实现
- 保持 `modbussim-ui-shared::ui` 所有 helper 的对外签名**完全不变**，上层零改动
- 清理 `egui-modal` / `egui-toast` 死依赖

**非目标：**
- egui 0.34（当前 shadcn 不支持；等 shadcn 跟进再做下一轮）
- egui-material3（shadcn 够用，Material 在工业 simulator 场景偏消费化）
- 换表格渲染（`egui_extras::TableBuilder` 继续用）
- 彻底重写 theme 色板（继续用 Darcula 三级分层，只把橙 + 绿作为 shadcn 的 primary / success token）

## 决策

### D1 · 锁 egui 0.33 而非 0.34

egui-shadcn 0.3.1 显式声明 `egui = "0.33"`。0.34 需要 shadcn 出新版本或 fork。最小风险路径是**锁 0.33**。一年后 shadcn 和 phosphor 都支持 0.34 时再升。

### D2 · Wrapper API 稳定

`ui.rs` 里所有 helper 函数**签名不变**：

```rust
// Before (自研)
pub fn primary_button(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response;

// After (shadcn 底层)
pub fn primary_button(ui: &mut Ui, flavor: Flavor, text: impl Into<String>) -> Response {
    egui_shadcn::button(ui, text).variant(Variant::Default).size(Size::Md).show(ui)
}
```

上层 `modbussim-egui` / `modbusmaster-egui` / `log_panel.rs` 的所有调用点一行都不改。这是升级风险控制的关键。

### D3 · 死依赖清理时机

`egui-modal` / `egui-toast` 在 `grep -rn "egui_modal\|egui_toast"` 时只在 Cargo.toml 里声明，代码零引用。这是历史包袱。**在本变更里一并删除**而不是留给下一次——升级 + 清理同一个 commit 粒度合理。

### D4 · Phosphor 0.11 兼容性验证

phosphor 0.12 绑 0.34。0.11 的 egui 依赖待验证（搜索结果未明示）。落地时：
1. 先试 `egui-phosphor = "0.11"`
2. 若 Cargo 报 egui 版本冲突，降 0.10
3. 若 0.10 也不行，fork 一份改 Cargo.toml

图标字体嵌入/glyph 常量（`icons::CPU` / `icons::PLUS_CIRCLE` 等十几个调用点）在 0.10-0.11 之间应无变化（phosphor 字面量稳定）。

### D5 · egui 0.29 → 0.33 API 迁移

已排查：

| 0.29 API | 0.33 状态 | 我们是否用 |
|---|---|---|
| `screen_rect` | 废弃，改 `viewport_rect`/`content_rect` | ❌ 未用 |
| `on_begin_pass` / `on_end_pass` | 仍可用，但推荐迁到新 Plugin trait | ❌ 未用 |
| `TextEdit::load_state` + `CCursorRange::two` | ⚠ 待验证 API 是否有 rename | ✅ T6 搜索框使用 |
| `TableBuilder` 所有方法 | 稳定，应无破坏 | ✅ 大量使用 |
| `egui::KeyboardShortcut` / `input_mut(...).consume_shortcut` | 稳定 | ✅ T6 Cmd+F |
| `ctx.style_mut(...)` | 稳定 | ✅ theme::apply |
| font backend | 换 skrifa；`install_cjk_fonts` 可能要改 `Font::from_bytes` → 新 API | ✅ 需改 |

font backend 的具体 API 名字升级时看 compiler error 定点修，不做前期 spike。

### D6 · shadcn theme token 与我们的 color 系统同步

shadcn 用自己的 theme token（`--primary` / `--primary-foreground` / `--destructive` 等）。egui-shadcn 通过它自己的 theme API 暴露这些 token。实施时：

1. 在 `theme::apply` 里，调完我们自己的 Visuals 覆写后，额外调用 shadcn 的 theme setter（具体 API 待 crate 文档确认），传入：
   - `primary` = accent 橙 `#cc7832`
   - `destructive` = danger 红 `#bc3f3c`
   - `background` = `bg_of(L1)` `#2b2d30`
   - `foreground` = 文本色 `#d4d7db`
   - `ring` = accent 橙

2. Flavor 切换时重放，以保证 shadcn 组件跟着我们的 Flavor 走。

### D7 · 回退策略

分支 `refactor/egui-skeleton` 当前 HEAD 打 tag `pre-egui-033` 作为回退基线。如果升级中任何阶段（egui bump / shadcn 集成 / API 迁移）卡死，`git reset --hard pre-egui-033` 即可回到已发布的 `visual-flat-layered-v2 + bool-view-and-button-polish` 状态。

## 风险 / 权衡

| 风险 | 影响 | 缓解 |
|---|---|---|
| phosphor 0.11 实际不支持 egui 0.33 | 图标字体无法加载，10+ 调用点出空框 | 降级 0.10；再不行 fork |
| egui-shadcn 0.3.1 API 文档稀薄 | 实施时 trial-error | Phase 2 先做 widget_gallery 试跑，确认 API 再集成 |
| font backend 换 skrifa 后中文字体测量/渲染行为微变 | 表格对齐 / 字形略改 | 升级后目视 + mbpoll 冒烟都通过才过关 |
| TextEdit::load_state API 换了 | T6 搜索框全选功能失灵 | 按 compiler error 修；最坏情况去掉全选功能 |
| TableBuilder scroll_to_row 签名变化 | T6 地址跳转失灵 | 同上 |
| 生态 crate 某个不配合 | 阻塞整个升级 | Phase 1 先把 egui/eframe/extras 三件升到 0.33 验证编译；如三件干净,再做后续 |
| CI 三平台 release build 失败（Linux 新 wgpu / skrifa 依赖） | PR 无法合并 | ci-egui.yml 本地 act 或预 push 到 feature branch 先跑一遍 |

## 迁移计划

分 4 个 phase，每个 phase 一个 commit 可独立回退：

**Phase 1 · 纯 bump egui 到 0.33**（核心三件 + phosphor + catppuccin + egui_extras）
- workspace Cargo.toml 改版本号
- `cargo check --workspace` 报错按 API diff 修
- 关键点：install_cjk_fonts、TextEdit::load_state、TableBuilder API
- 不引入 shadcn，不删死依赖（范围控制）
- 验证: `cargo test --workspace` 绿 + `cargo build --release` 绿 + `mbpoll` 读写仍通

**Phase 2 · 引入 egui-shadcn**（试跑 widget gallery）
- workspace deps 加 `egui-shadcn = "0.3"`
- 在 `modbussim-ui-shared` 下新建 `src/shadcn_preview.rs` 跑一个 widget_gallery，验证 Button / Switch 能渲染、theme token 能配
- 不改现有 ui.rs wrapper
- 验证: `cargo run -p modbussim-egui --bin shadcn_preview` 目视

**Phase 3 · ui.rs wrapper 切到 shadcn**
- primary / secondary / danger / icon_button 内部换 shadcn Button
- toggle_switch 换 shadcn Switch
- theme::apply 扩展 shadcn theme token 同步
- 签名不变，上层零改动
- 验证: Slave / Master / log_panel 全视图目视 OK；FC01 toggle 能翻转

**Phase 4 · 清理死依赖 + shadcn_preview bin 删除 + CI 验证**
- 删 egui-modal / egui-toast（workspace + 三 crate Cargo.toml）
- 删 Phase 2 的预览 bin
- push，CI 三平台跑过
- smoke: mbpoll 读写 + 地址搜索 + 抖动开关 + 批量添加 一条龙

## 未决问题

1. egui-shadcn v0.3.1 的具体 API 符号（`Variant::Default` 还是 `ButtonVariant::Default`）？Phase 2 试跑时确认。
2. Theme token 同步 API 如何调用？shadcn 文档需 RTFS（read the f* source）。Phase 2 时读。
3. phosphor 0.10 vs 0.11 哪个兼容 0.33？Phase 1 先试 0.11。
4. eframe 0.33 的 `NativeOptions.viewport` 是否还叫这个名？main.rs 可能要跟。
5. CI 的 ubuntu-22.04 libraries（libgtk-3-dev / libxcb）对新 skrifa font backend 是否需要新增包？先跑 CI 看报错。
