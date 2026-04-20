## 修改需求

### 需求:按钮样式规范

所有按钮必须由 `modbussim-ui-shared::ui` 的 wrapper 函数（`primary_button` / `secondary_button` / `danger_button` / `icon_button`）渲染。Wrapper 内部**必须**使用 `egui-shadcn` crate 提供的 Button 变体作为底层渲染实现，不得自绘或使用 `egui::Button` 直接渲染。

四种 wrapper 的 shadcn 变体映射：
- `primary_button` → shadcn Default 变体；fill = Darcula 橙 `accent` `#cc7832`（通过 shadcn theme token 设置），white text
- `secondary_button` → shadcn Outline 变体；transparent fill + 1 px subtle border + hover bg
- `danger_button` → shadcn Destructive 变体；fill = `danger` `#bc3f3c`，white text
- `icon_button` → shadcn Ghost 变体 + icon content；默认 transparent，hover 填

Padding / rounding / focus ring 由 shadcn Default Size = Md (高度 36 px 左右) 托管，不再由我们手写。如需更紧凑（Slave 表格里按钮太高）可切换到 Sm 变体，但不得关闭 focus ring。

#### 场景:Slave 批量添加按钮视觉

- **当** 用户在 Slave `Selection::RegisterGroup` 看到"批量添加"按钮
- **那么** 该按钮必须使用 shadcn Default 变体（通过 `primary_button` wrapper）
- **那么** 背景必须为 `accent` 主橙 `#cc7832`
- **那么** 键盘 Tab 聚焦时必须显示 shadcn focus ring

#### 场景:次级按钮 Outline 变体

- **当** 用户看到 Slave 中任意次级按钮（"停止"、"删除"、"清空"、"导出 CSV"、"关闭"、"创建"等）
- **那么** 该按钮必须使用 shadcn Outline 变体
- **那么** 静止态必须有 1 px subtle border，肉眼可辨是按钮
- **那么** hover 时必须显示 `bg_hover` fill + 加深 border 的反馈
- **那么** 不得为透明无框

### 需求:Bool 寄存器切换按钮形变

`Selection::RegisterGroup` 中 FC01 线圈 / FC02 离散输入的 Bool 列必须使用 **shadcn Switch 控件**作为底层实现，通过 `modbussim-ui-shared::ui::toggle_switch` wrapper 暴露。

视觉 / 交互来自 shadcn：
- 尺寸: shadcn 默认（约 36×20 px，与 40×18 相近）
- on 态轨道色: `theme::success(flavor)` (深色 `#6a8759`)
- off 态轨道色: shadcn default (低对比灰)
- 滑块 + 动画: Radix 风格滑动过渡（非瞬时跳变）
- focus ring: shadcn 键盘聚焦有可见 ring
- 全 switch 矩形必须 clickable

wrapper 内部不得再有 `painter.circle_filled` / `painter.rect_filled` / 自绘椭圆代码。

#### 场景:FC02 离散输入行切换

- **当** 用户打开 FC02 视图并点击 addr=5 的 Bool 单元格
- **那么** 该单元格视觉从"灰色 shadcn Switch off 态"变为"绿色 shadcn Switch on 态"
- **那么** 切换必须有 shadcn 滑动动画（非瞬时跳变）
- **那么** 行其他列（名称、注释）内容保持不变

#### 场景:键盘 Tab 聚焦

- **当** 用户按 Tab 键聚焦到某行 switch
- **那么** 必须显示 shadcn focus ring
- **那么** 按空格键必须翻转 switch 值
- **那么** wrapper Response 必须正确反馈 `clicked()` = true

