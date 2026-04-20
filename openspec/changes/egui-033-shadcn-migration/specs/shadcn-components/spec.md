## 新增需求

### 需求:egui-shadcn 集成作为按钮 / 开关的底层实现

`modbussim-ui-shared::ui` 模块的 `primary_button` / `secondary_button` / `danger_button` / `icon_button` / `toggle_switch` 五个 helper 必须以 `egui-shadcn` crate 提供的控件为底层实现。Wrapper 函数对外签名**保持不变**——上层 `modbussim-egui` / `modbusmaster-egui` / `log_panel.rs` 不得需要修改调用点。

变体映射：
- `primary_button(ui, flavor, text)` → `egui_shadcn::button(ui, text).variant(Default).size(Md)`
- `secondary_button(ui, flavor, text)` → `egui_shadcn::button(ui, text).variant(Outline).size(Md)`
- `danger_button(ui, flavor, text)` → `egui_shadcn::button(ui, text).variant(Destructive).size(Md)`
- `icon_button(ui, flavor, icon)` → `egui_shadcn::button(ui, icon).variant(Ghost).size(Sm)`
- `toggle_switch(ui, flavor, &mut bool)` → `egui_shadcn::switch(ui, value)`（保留 `Response` 返回类型）

具体的 shadcn API 符号以 crate 实际导出为准（该 crate 0.3 版本 API）。如发现 shadcn 不支持某个特定变体（如 Ghost）则用自定义 Frame + shadcn Button 组合，不得回退到自绘。

shadcn 变体的 theme token（`primary color` / `border color` / `ring color` 等）必须在应用启动时通过 shadcn 的 theme API 与 `theme::apply` 同步调用，确保 dark mode 用 Darcula 橙 `#cc7832` 作 primary、`#6a8759` 作 success（switch on 色）。

#### 场景:primary_button 视觉

- **当** 用户查看 Slave `Selection::RegisterGroup` 的"批量添加"按钮
- **那么** 该按钮必须由 shadcn `Default` 变体渲染
- **那么** fill 必须为 accent 橙 `#cc7832`
- **那么** 必须显示 shadcn 的 focus ring（2 px 外描边）当该按钮获得键盘焦点
- **那么** 调用方代码 `uikit::primary_button(ui, flavor, "批量添加")` 必须无签名变更

#### 场景:toggle_switch 使用 shadcn Switch

- **当** 用户点击 FC01 视图某行的 bool 开关
- **那么** 开关视觉必须是 shadcn Switch（带 Radix 滑动动画）
- **那么** on 态轨道色必须为 success `#6a8759`
- **那么** 对外 API `uikit::toggle_switch(ui, flavor, &mut bool) -> Response` 必须保持不变
- **那么** 该函数内部必须**不再**包含任何手绘椭圆 / circle_filled 逻辑

#### 场景:依赖缺失降级

- **当** 未来 egui-shadcn crate 停止维护且 egui 升级到更新版本
- **那么** 我们可以 fork shadcn 至本地 `third_party/egui-shadcn/` 目录
- **那么** wrapper 签名保持不变，维持兼容层
