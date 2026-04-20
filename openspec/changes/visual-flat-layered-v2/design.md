## 上下文

当前 egui 双端 UI（`modbussim-egui` + `modbusmaster-egui`）的视觉结构基于"每个功能区包一个 `uikit::card` + 1 px stroke"的做法，叠加 `SidePanel` 自带边界、`ui.separator()` 横线，最终呈现为"处处硬划线分割"的视觉。所有区域共用单一 `#3c3f41` 背景，只靠线区分板块。

同时 Slave 的 `Selection::RegisterGroup` 视图面对 20001 行寄存器表，却无任何搜索 / 跳转 / 过滤功能——用户修改地址 12345 只能手动滚动。

Tauri 老前端已经有：
- `frontend/src/components/RegisterTable.vue:200-226` 的 `filteredRegisters` 搜索逻辑（地址 + 名称双轨）
- `frontend/src/components/RegisterTable.vue:588-641` 的虚拟滚动 + 地址定位

本次变更把"视觉扁平化"与"搜索交互补齐"一起做，避免两次触动同一批文件。

## 目标 / 非目标

**目标：**
- 建立 egui 双端的统一视觉规范（三级背景、按钮、Bool 切换）
- 解锁 RegisterGroup 视图的地址跳转 + 名称过滤 + Cmd+F 聚焦
- 删除 99% 的 `uikit::card` 调用点，替换为 `ui::region`（仍保留 `card` 入口供极少数需要"强调边框"的场景）
- Slave 4 个 Selection 分支、Master 3 个 Tab、共享日志面板 **一次性**改完

**非目标：**
- 浅色 Flavor（Latte）的色板适配 —— 本次深色优先，浅色保证"可用不崩溃"即可
- 动画 / 过渡效果（egui 桌面态动画收益低）
- 表头 sticky（egui_extras 0.29 不原生支持，方案降级见下）
- 换字体、换主题切换器 UX
- `.modbusproj` 序列化改动（本次纯 UI）

## 决策

### D1 · 三级背景色而不是阴影

**选择**：用 `#1e1f22 / #2b2d30 / #313338` 三层 bg 区分区域。
**备选 A**：`egui::Shadow` 软阴影（"浮起卡片"感）。否决：egui 软阴影边缘糊、性能一般，且此前尝试过用户反馈"像装饰"（见 plan 文件 Checkpoint 2026-04-20）。
**备选 B**：保留 card 但 stroke 改为发光色。否决：仍是"处处画线"，治标不治本。

三级分层的 RGB 差 ≥ 6 单位，肉眼可辨但不刺眼，参考 JetBrains Darcula 窗口系统（tool window #3c3f41 vs editor #2b2b2b 差 17 单位，我们这里略收紧）。

### D2 · `region` helper 取代 card 大多数调用

新函数签名：
```rust
pub enum Layer { L0, L1, L2 }

pub fn region<R>(
    ui: &mut Ui,
    flavor: Flavor,
    layer: Layer,
    margin: egui::Margin,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    egui::Frame::none()
        .fill(theme::bg_of(flavor, layer))
        .inner_margin(margin)
        .show(ui, add)
        .inner
}
```

保留 `card` / `accent_card` 入口避免 breaking，但 Slave / Master / log_panel 全部渲染路径改调 `region`。

**Rejected**：继续沿用 `Frame::group()`（egui 内置） — 它会画 noninteractive.bg_stroke，正是我们要去掉的线。

### D3 · Bool 切换改用自绘圆点

之前 c8c3ffc / 6fec337 等 commit 已经把 egui 原生 checkbox 改为"0/1 文字按钮"，但在 Darcula 下仍偏"Office 塑料"。这次改用 `painter.circle_filled(r=4, color=success|subtext)` + 跟随文字的 `ON/OFF`。

**要点**：
- 命中测试用 `ui.allocate_exact_size(row_h × col_w, Sense::click())`
- 整行级 click，不是只点圆点
- 写回逻辑复用现有 `writes.push((addr, v))` 路径，语义零变

### D4 · 搜索框状态外置到 SlaveApp

搜索框文本**必须**持久化到 `SlaveApp` 级 HashMap，key `(conn_id, slave_id, reg_type)`，以便：
- 切到其他 Device 再回来时文本保留
- 多 RegisterGroup Tab 式使用体验

```rust
pub struct SlaveApp {
    ...
    search_buf: HashMap<(String, u8, RegisterType), String>,
}
```

若放进 `RegViewCache`，切 cache 时会丢——明确不放那里。

### D5 · 地址跳转与过滤二选一

纯数字合法 u16 → 地址跳转 + 高亮，**不**过滤其它行；  
非纯数字 / 超范围 → 模糊过滤。

**备选**：同时过滤 + 跳转到第一个匹配。否决：用户意图上地址跳转和模糊过滤诉求不同，混合会让"我就想滚到 addr=10"变成"只显示 addr=10" 错觉。Tauri 版也是二选一。

### D6 · 高亮用 ui.painter 画 rect 而不是改 row bg

Row 级 2 秒淡出高亮：
- `last_highlight: Option<(u16, Instant)>` 存在 `RegViewCache` 或 `SlaveApp`
- TableBuilder body_rows 回调里检查 `row.index() as u16 == highlighted && elapsed < 2.0s`
- 命中时 `painter.rect_filled(row_rect, 0.0, accent.linear_multiply(fade_alpha))`
- `fade_alpha = 0.6 * (1.0 - elapsed / 2.0)`
- 每帧 `ctx.request_repaint()` 驱动淡出

**备选**：改 `TableBuilder.striped` 的反色。否决：TableBuilder striped 不可按条件行开关。

### D7 · Cmd+F 绑定

```rust
let shortcut = egui::KeyboardShortcut::new(
    egui::Modifiers::COMMAND,
    egui::Key::F,
);
if ctx.input_mut(|i| i.consume_shortcut(&shortcut)) {
    if matches!(self.selection, Selection::RegisterGroup { .. }) {
        self.want_focus_search = true;
    }
}
```

渲染时：
```rust
let resp = ui.add(egui::TextEdit::singleline(&mut buf));
if self.want_focus_search {
    resp.request_focus();
    // 全选：egui 0.29 用 egui::TextBuffer::clear + reinsert，或 TextEditOutput.state.set_ccursor_range
    self.want_focus_search = false;
}
```

**备选**：监听 raw `Key::F` + modifier 手写。否决：`consume_shortcut` 已是 egui 官方 API，跨平台 COMMAND = macOS ⌘ / 其他 Ctrl。

### D8 · Sticky header 的降级

`egui_extras::TableBuilder` 0.29 不支持 sticky header。方案：
1. 表头 bg 改用 `bg_layer_1`（主区同色），滚动时表头会随 ScrollArea 一起滚走——接受这个降级
2. 或者把 header 拆出 `TableBuilder`，手动画一个固定行然后表格从第二行开始——复杂度大、工作量不值
3. **接受方案 1**，若用户反馈需要再升级（用户本轮已经说"布局还行"，sticky 不是核心痛点）

## 风险 / 权衡

| 风险 | 影响 | 缓解 |
|---|---|---|
| `Frame::none().fill()` 在不同平台下 alpha blend 结果略有差异 | 视觉不一致 | 只用 opaque Color32，不用半透明 fill |
| Cmd+F 被 macOS WindowServer 抢走 | 快捷键失灵 | `consume_shortcut` 在 egui 帧开始即调用（consume 优先） |
| 6 处视图一起改，commit 回退难 | 出 bug 难溯源 | 每个视图一个 commit（Slave None / Connection / Device / RegisterGroup / Master / log_panel 共 6 commit） |
| `region` 去掉 stroke 后某些小 UI 元素（如 Grid 单元格）失去边界 | 视觉"粘"在一起 | 表格 / TextEdit 等数据容器保留 1 px 内部 stroke，只去"外层 card" stroke |
| Bool 自绘点在窗口高 DPI 下可能糊 | 点偏小/锯齿 | 用 `ui.painter().circle_filled` 自动适配 pixels_per_point |
| 20K 行表的高亮计算每帧跑 | 渲染成本 | 只在 `last_highlight.is_some() && elapsed < 2s` 窗口内 request_repaint |
| 搜索 HashMap key 用 String 多次 clone | 内存碎片 | 切 RegisterGroup 时直接 `entry().or_default()` 获取可变引用，避免 clone |

## 迁移计划

本变更为**纯视觉 + 纯 UI 交互**改动，无数据迁移、无 API breaking。回退只需 `git revert`。

实施顺序建议（对应 tasks.md 的 6 commit）：

1. **T1** `theme.rs` + `ui.rs` 基建（新 color 常量、`region`、按钮重写）
2. **T2** `log_panel.rs` 换 `region`（最简单，先验证 region 形态）
3. **T3** Master `app.rs` 3 个 Tab 改造（按钮、card → region）
4. **T4** Slave `app.rs` None / Connection / Device 分支（较小）
5. **T5** Slave `app.rs` RegisterGroup 分支（最重头）
6. **T6** Slave RegisterGroup 添加搜索框 + Cmd+F + 地址跳转 + 过滤

## 未决问题

1. 搜索框的"地址跳转 trigger 时机"：是**边打边跳**（每次字符变化）还是**回车 / debounce 300 ms**？
   - 倾向 debounce 300 ms 避免打到 `1`（addr=1） → `12`（addr=12） → `123`（addr=123）连续跳动
   - T6 实施时确认
2. Bool 圆点在点击瞬间是否要做 150 ms 颜色过渡动画？
   - 倾向**不做**（D3 已决定不加动画）
   - 若用户反馈"切换不明显"再加
3. 日志面板的"关闭"按钮目前是纯文字。是否也改为 icon-only 按钮？
   - 不在本变更范围；待后续 icons 字体嵌入决定后统一处理
