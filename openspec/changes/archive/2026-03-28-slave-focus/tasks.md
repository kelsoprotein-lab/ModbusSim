## 1. 后端：移除主站代码

- [x] 1.1 从 `state.rs` 中删除 `MasterConnectionState`、`MasterConnectionInfo`、`next_master_id` 字段及相关 use 声明
- [x] 1.2 从 `commands.rs` 中删除所有主站相关命令：`create_master_connection`、`connect_master`、`disconnect_master`、`delete_master_connection`、`list_master_connections`、`master_read`、`master_write_single_coil`、`master_write_single_register`，以及 `CreateMasterRequest`、`ReadRequest`、`WriteSingleCoilRequest`、`WriteSingleRegisterRequest`、`MasterConnectionEvent`、`parse_read_function` 等相关结构体和函数
- [x] 1.3 从 `lib.rs`（app crate）的 `invoke_handler` 中移除所有 `master_*` 命令注册
- [x] 1.4 简化日志命令：`get_communication_logs`、`clear_communication_logs`、`export_logs_csv` 移除 `is_master` 参数，删除主站分支，固定查从站连接
- [x] 1.5 清理 `commands.rs` 顶部不再需要的 `use` 导入（`MasterConfig`、`MasterConnection`、`ReadFunction`、`ReadResult`、`PollConfig` 等）
- [x] 1.6 验证 `cargo build` 编译通过，无 dead_code 警告

## 2. 前端：移除主站 UI

- [x] 2.1 删除 `MasterView.vue` 组件文件
- [x] 2.2 修改 `App.vue`：删除 `appMode` ref 和 `provide('appMode', appMode')`；删除 `v-if="appMode === 'slave'"` / `v-else` 条件分支；固定使用从站三栏布局；删除 MasterView 导入；删除 `:has(.master-mode)` CSS 规则
- [x] 2.3 修改 `Toolbar.vue`：删除 `appMode` inject；删除 `toggleMode` 函数；删除"从站模式/主站模式"切换按钮及相关 CSS
- [x] 2.4 修改前端日志相关调用：移除 `get_communication_logs`、`clear_communication_logs`、`export_logs_csv` 调用中的 `is_master` 参数
- [x] 2.5 验证前端编译通过且无运行时错误

## 3. 后端：新增随机值初始化

- [x] 3.1 在 `modbussim-core` 的 `Cargo.toml` 中添加 `rand` 依赖
- [x] 3.2 在 `slave.rs` 中实现 `SlaveDevice::with_random_registers(slave_id, name, max_address)` 方法：Coil/DI 使用 `rand::random::<bool>()`，Holding/Input Register 使用 `rand::random::<u16>()`
- [x] 3.3 为 `with_random_registers` 添加单元测试：验证生成 404 个 RegisterDef，RegisterMap 四个区域各 101 个地址有值
- [x] 3.4 修改 `commands.rs` 中 `CreateSlaveRequest`，增加 `init_mode: Option<String>` 字段
- [x] 3.5 修改 `create_slave_connection` 命令，根据 `init_mode` 选择 `with_default_registers` 或 `with_random_registers`
- [x] 3.6 修改 `commands.rs` 中 `AddSlaveDeviceRequest`，增加 `init_mode: Option<String>` 字段
- [x] 3.7 修改 `add_slave_device` 命令，当 `init_mode` 有值时使用带预填寄存器的构造方法
- [x] 3.8 验证 `cargo test` 全部通过

## 4. 前端：新建从站/连接对话框改造

- [x] 4.1 修改 `Toolbar.vue` 中 `newConnection` 函数：将 `showPrompt` 改为自定义模态框，包含端口号输入和初始值模式选项（全零/随机），调用 `create_slave_connection` 时传入 `init_mode`
- [x] 4.2 修改 `Toolbar.vue` 中 `newSlave` 函数：将 `showPrompt` 改为自定义模态框，包含从站 ID 输入和初始值模式选项（全零/随机），调用 `add_slave_device` 时传入 `init_mode`
- [x] 4.3 验证前端编译通过，新建连接和新建从站的完整流程正常工作

## 5. 端到端验证

- [ ] 5.1 端到端测试：新建连接（零值模式）→ 验证寄存器全为 0 → 新建连接（随机模式）→ 验证寄存器有随机值
- [ ] 5.2 端到端测试：新建从站（随机模式）→ 启动连接 → 使用外部 Modbus 客户端读取验证随机值
- [x] 5.3 确认程序中无任何主站相关的 UI 元素或命令
