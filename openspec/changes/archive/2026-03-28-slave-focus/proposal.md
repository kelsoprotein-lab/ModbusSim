## 为什么

当前程序同时包含从站和主站功能，通过工具栏 toggle 切换模式。实际使用中两者场景完全不同——从站是模拟器核心功能，主站更多用于调试验证。混在同一程序中增加了 UI 复杂度（模式切换、布局条件判断），也分散了开发精力。用户希望从站就是从站，专注打磨从站体验。

同时，当前创建从站后所有寄存器值均为 0，不便于快速验证 Modbus 通信和数据解析是否正确。需要一个创建时可选的随机值初始化功能。

## 变更内容

### 软分离：移除主站代码
- **删除** 后端 `commands.rs` 中所有主站相关命令（`create_master_connection`、`connect_master`、`disconnect_master`、`delete_master_connection`、`list_master_connections`、`master_read`、`master_write_single_coil`、`master_write_single_register`）及相关 DTO/Request 结构体
- **删除** 后端 `state.rs` 中 `MasterConnectionState`、`MasterConnectionInfo`、`next_master_id`
- **删除** 后端 `main.rs` 中主站命令的注册
- **修改** 后端日志命令（`get_communication_logs`、`clear_communication_logs`、`export_logs_csv`）移除 `is_master` 参数及主站分支，简化为仅服务从站
- **删除** 前端 `MasterView.vue` 组件
- **删除** 前端 `App.vue` 中 `appMode` 状态及主站布局分支
- **删除** 前端 `Toolbar.vue` 中模式切换按钮和 `toggleMode` 函数
- **保留** `modbussim-core` 中 `master.rs` 模块（未来独立主站程序使用）

### 新增随机值初始化
- **新增** 后端 `SlaveDevice::with_random_registers()` 方法，使用 `rand` crate 生成随机初始值
- **修改** 后端 `create_slave_connection` 和 `add_slave_device` 命令，增加 `init_mode` 参数（`"zero"` | `"random"`）
- **修改** 前端新建从站对话框，增加初始值模式选项
- **新增** 依赖 `rand` crate 到 `modbussim-core`

## 功能 (Capabilities)

### 新增功能
- `random-init`: 随机值初始化 — 创建从站时可选随机初始化寄存器值（Coil/DI 50% 概率 true，Holding/Input Register 全范围 u16 随机）

### 修改功能
- `slave-mode`: 移除主站代码后成为程序唯一模式，UI 布局固定为从站三栏布局，日志命令简化
- `auto-default-slave`: 创建连接时的默认从站支持选择初始值模式（全零或随机）

## 影响

- **前端**：删除 `MasterView.vue`；`App.vue` 删除 `appMode` 及条件分支简化布局；`Toolbar.vue` 删除模式切换按钮；新建从站对话框增加初始值选项
- **后端**：`commands.rs` 删除约 250 行主站命令及相关结构体；`state.rs` 移除主站状态；日志命令移除 `is_master` 参数；`slave.rs` 新增随机初始化方法
- **API**：移除所有 `master_*` Tauri 命令；日志命令签名变更（移除 `is_master`）；`create_slave_connection` 和 `add_slave_device` 新增 `init_mode` 参数
- **依赖**：`modbussim-core` 新增 `rand` crate
- **core 库**：`master.rs` 保留不动，`lib.rs` 中 `pub mod master` 保留
