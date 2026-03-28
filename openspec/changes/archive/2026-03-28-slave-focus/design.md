## 上下文

ModbusSim 是一个基于 Tauri (Rust + Vue) 的 Modbus TCP 模拟器。当前程序同时包含从站（Slave）和主站（Master）两种模式，共享同一个 AppState、同一个前端窗口。后端 core 库中 `slave.rs` 和 `master.rs` 已完全独立，但 app 层（`commands.rs`、`state.rs`、`main.rs`）和前端（`App.vue`、`Toolbar.vue`、`MasterView.vue`）仍然耦合在一起。

创建从站设备时，所有寄存器值初始化为 0，不利于快速验证通信和数据解析。

## 目标 / 非目标

**目标：**
- 从 app 层和前端中彻底移除主站相关代码，使程序成为纯从站模拟器
- 简化日志命令接口，移除 `is_master` 参数
- 新增创建时可选的随机值初始化功能
- 保持 `modbussim-core` 中 `master.rs` 不受影响，供未来独立主站程序使用

**非目标：**
- 不创建独立的主站程序（留待后续）
- 不修改 core 库中 master.rs 的任何代码
- 不增加运行时随机填充功能（仅创建时选项）
- 不增加自定义随机范围配置

## 决策

### 1. 软分离而非物理拆分 workspace

**选择**：从 `modbussim-app` 和前端中删除主站代码，保留 `modbussim-core` 中 `master.rs`。

**替代方案**：将 workspace 拆分为 `modbussim-slave` 和 `modbussim-master` 两个独立 Tauri app。

**理由**：物理拆分涉及 Tauri 项目结构重组（两套 `tauri.conf.json`、两套前端构建），工作量大但当前只需要做从站。先删除 app 层主站代码即可达到目标，未来做主站时再从 core 库上建新 app。

### 2. 日志命令移除 is_master 参数

**选择**：`get_communication_logs`、`clear_communication_logs`、`export_logs_csv` 三个命令直接移除 `is_master` 参数，固定查从站连接日志。

**替代方案**：保留参数但忽略它。

**理由**：保留无用参数增加前后端复杂度且容易混淆。直接移除更干净，前端调用也更简单。这是 breaking change 但仅影响内部 API。

### 3. 随机值实现方式

**选择**：在 `SlaveDevice` 上新增 `with_random_registers()` 构造方法，与现有 `with_default_registers()` 并列。使用 `rand` crate 的 `thread_rng()`。

**替代方案 A**：在 `with_default_registers()` 上增加参数控制初始值模式。
**替代方案 B**：使用 `RegisterMap` 层面的 `fill_random()` 方法。

**理由**：独立的构造方法语义清晰，不修改现有方法签名，调用方通过 `init_mode` 选择。方案 A 会让已有调用点都需要改；方案 B 增加了 RegisterMap 的职责。

### 4. init_mode 参数设计

**选择**：在 `CreateSlaveRequest` 和 `AddSlaveDeviceRequest` 中增加 `init_mode: Option<String>`（`"zero"` | `"random"`），默认 `"zero"`。

**理由**：用 Option + 默认值保持向后兼容，不影响现有创建逻辑。字符串类型方便前端传参和未来扩展。

### 5. 前端新建从站对话框

**选择**：将当前简单的 `showPrompt` 对话框替换为自定义模态框，包含从站 ID、名称、地址范围、初始值模式等字段。

**替代方案**：继续用 prompt 对话框，通过多次 prompt 收集信息。

**理由**：多次 prompt 体验差。自定义模态框可以一次性收集所有参数，且项目已有 `AppDialog.vue` / `RegisterModal.vue` 等模态框组件可参考。

## 风险 / 权衡

- **[API Breaking Change]** 日志命令移除 `is_master` 参数 → 前后端同步修改即可，无外部消费者
- **[新依赖]** `rand` crate → 成熟的 Rust 生态标准库，体积影响极小
- **[未来兼容]** core 库保留 `master.rs` 但 app 层不引用 → `master.rs` 可能因长期不编译而 bit rot → 通过 core 库的单元测试保持编译正确性
