## 为什么

刚归档的 `visual-flat-layered-v2` 变更把 Bool 列从填色按钮改成了 `○ OFF / ● ON` 圆点+文字形态。用户在实际使用中反馈两个问题：

1. **Bool 视图空间浪费**：FC01/FC02 表格沿用 FC03/FC04 的五列布局（地址/值/Hex/Binary/空），但对 bool 而言 Hex 和 Binary 列毫无意义，实际渲染为两个 `—`，剩余 ~70% 水平空间完全闲置。20001 行在屏幕上每行只有 110 px 的"布尔"列里显示一个 ● 和两个字母，信息密度和交互密度都很低。
2. **`○/●` 不像可点按钮**：圆点+文字看起来像纯 label，缺少"按钮 affordance"——没有边框、没有静止态背景、没有 hover 反馈。工程师在 20K 行列表里反复切换 bool 时，看不出这是可交互元件。
3. **所有次级按钮过度扁平化**：`secondary_button` 当前 `transparent fill + Stroke::NONE`，静止态几乎隐形。用户反馈"所有页面的按钮都不好看"——不是指 primary 橙色按钮，而是"停止 / 删除 / 清空 / 导出 CSV / 关闭 / 创建" 这些全部肉眼难辨的次级按钮。

时机合适：Bool 视图改造、值控件改造、按钮静止态调整都只影响 `ui.rs`（helper）+ `app.rs`（单个 Selection 分支），代码局部性强，一次做完一致性好。

## 变更内容

- **Bool 表格列重分配**（`modbussim-egui::Selection::RegisterGroup` 的 `is_bool` 分支）：从 `地址(80) / 布尔(110) / —(100) / —(140) / 空(remainder)` 改为 `地址(80) / 值(170) / 名称(200) / 注释(remainder)`。名称 / 注释列从当前 Device 对应的 `register_defs[addr]` 的 `name` / `comment` 字段取值（多数为空字符串 → 列空；未来项目文件导入命名后立即有效）。
- **新 `toggle_switch` 原语**（`modbussim-ui-shared/src/ui.rs`）：自绘 40×18 px 椭圆轨道 + 14 px 圆形滑块。`on` 态绿色轨道（success），`off` 态灰色轨道（bg_hover）。滑块 hover 时放大到 16 px 给 affordance 反馈。返回 `(Response, new_value)` 方便上层判断是否翻转写回。
- **Bool 值列使用 `toggle_switch`** 替代当前自绘的 ○/● 点 + 文字。位置在列正中。
- **`secondary_button` 静止态加淡 fill**：从 `Color32::TRANSPARENT` 改成 `theme::bg_of(flavor, Layer::L2)`（深色 `#313338`、浅色 `#ffffff`），相对 panel 层（L1 `#2b2d30`）视觉上略浮起，但 2 个 RGB 单位差依然很微妙不喧宾夺主。hover 时 egui 全局 `widgets.hovered.bg_fill = bg_hover` 接管——affordance 清晰。primary 和 danger 保持不变（它们原本就有强 fill）。
- **BREAKING**（仅仅对 visual spec 而言）：`egui-visual-style` 里"bool 切换必须用圆点+文字"的需求被替换为"必须用 iOS 风格 toggle 开关"。

## 功能 (Capabilities)

### 新增功能
（无 — 本轮补丁不引入新 capability）

### 修改功能
- `egui-visual-style`: 调整"Bool 寄存器切换按钮形变"需求（改为 iOS toggle），微调"按钮样式规范"需求（secondary 加淡 fill 描述），新增"Bool 寄存器表格列布局"需求

## 影响

**受影响的 crate**：
- `modbussim-ui-shared` — `ui.rs` 加 `toggle_switch` 函数；`secondary_button` 改 fill
- `modbussim-egui` — `app.rs` `Selection::RegisterGroup` 的 `is_bool` 表格分支：header + body cell 重分配，`is_bool` 的 row.col 调用次数从 4 次（值 + 3 个空列）改为 4 次（值 / 名称 / 注释 / 空），用 toggle_switch 替换自绘 dot

**风险**：
- 工程文件 `.modbusproj` 里的 `register_defs` 通常有 20000+ 条空 name/comment 记录，渲染 4 列文本会降低滚动帧率；需确认 egui `TableBuilder::body.rows` 的懒渲染对空字符串不敏感（预期仍 60 fps）
- `toggle_switch` 的命中测试要给整个 40×18 区域，不能只是滑块圆点——否则 20K 行点击精度过差
- `secondary_button` 的 fill 在浅色 Flavor（Latte）下效果要再确认；本变更主要针对深色
