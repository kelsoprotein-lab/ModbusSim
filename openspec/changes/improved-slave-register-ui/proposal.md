## 为什么

当前子站寄存器管理界面缺少添加、编辑、删除寄存器的交互入口。用户只能通过 JSON 文件导入/导出操作寄存器，流程繁琐、效率低下。参考同类工具（ModbusSlave）的交互方式，通过 Modal 对话框和右键菜单可以大幅提升易用性。

## 变更内容

- **新增**：寄存器添加 Modal 对话框，支持填写名称、地址、类型，高级选项（数据类型、字节序）默认折叠
- **新增**：寄存器右键菜单，提供"编辑寄存器"和"删除寄存器"两个选项
- **编辑**：编辑寄存器复用水添加 Modal，预填当前值
- **新增**：地址冲突检测，当地址已存在时提示用户选择覆盖或取消
- **界面**：全部 UI 文案中文化

## 功能 (Capabilities)

### 新增功能

- `slave-register-ui`: 子站寄存器 UI 交互规范 — 定义 Modal 对话框、右键菜单、地址冲突检测的行为和 UI 文本

### 修改功能

- `slave-mode`: 现有的 Modbus 子站引擎已有 add_register 等 Tauri Commands，本次仅扩展前端交互层，不变更后端接口

## 影响

- 前端：`SlaveView.vue` 新增 RegisterModal 组件、右键上下文菜单、地址冲突提示逻辑
- 后端：无需修改，add_register / remove_register 等 Commands 已存在
- 规范：新增 `specs/slave-register-ui/spec.md`
