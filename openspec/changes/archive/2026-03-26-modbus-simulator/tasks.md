## 1. 项目脚手架

- [x] 1.1 初始化 Cargo workspace，创建 `crates/modbussim-core` 和 `crates/modbussim-app` 两个 crate
- [x] 1.2 使用 `cargo tauri init` 初始化 Tauri v2 项目（在 modbussim-app 中）
- [x] 1.3 初始化前端项目（选定框架 Svelte/Vue/React），配置构建流程与 Tauri 集成
- [x] 1.4 添加核心依赖：tokio-modbus、tokio、serde、serde_json 到 modbussim-core
- [x] 1.5 验证基本的 Tauri 应用可以在 macOS 和 Windows 上编译并运行空窗口

## 2. 寄存器数据模型（modbussim-core）

- [x] 2.1 定义寄存器类型枚举（Coil / DiscreteInput / HoldingRegister / InputRegister）
- [x] 2.2 定义数据类型枚举（Bool / UInt16 / Int16 / UInt32 / Int32 / Float32）和字节序枚举（Big / Little / MidBig / MidLittle）
- [x] 2.3 实现 RegisterDef 结构体（address、name、data_type、endian、comment）
- [x] 2.4 实现 RegisterMap 结构体（四种寄存器类型的存储，基于 HashMap<u16, u16>）
- [x] 2.5 实现数据类型编解码：uint16/int16 直接映射，uint32/int32/float32 跨两个寄存器的读写（含字节序处理）
- [x] 2.6 实现值范围校验（根据 data_type 验证输入值是否合法）
- [x] 2.7 为寄存器模型编写单元测试（类型转换、字节序、值校验）

## 3. 子站引擎（modbussim-core）

- [x] 3.1 定义 SlaveDevice 结构体（slave_id、name、RegisterMap、RegisterDef 列表）
- [x] 3.2 定义 SlaveConnection 结构体（transport 配置、多 SlaveDevice 管理、运行状态）
- [x] 3.3 实现基于 tokio-modbus 的 TCP Server，监听指定地址和端口
- [x] 3.4 实现从站 ID 路由：根据请求中的 unit_id 分发到对应的 SlaveDevice
- [x] 3.5 实现功能码处理：FC01（读线圈）、FC02（读离散输入）、FC03（读保持寄存器）、FC04（读输入寄存器）
- [x] 3.6 实现功能码处理：FC05（写单个线圈）、FC06（写单个保持寄存器）、FC15（写多个线圈）、FC16（写多个保持寄存器）
- [x] 3.7 实现异常响应：未注册的从站 ID 静默丢弃，非法地址/非法功能码返回对应异常码
- [x] 3.8 支持多客户端并发连接（tokio spawn 每个连接的处理任务）
- [x] 3.9 为子站引擎编写集成测试（使用 tokio-modbus client 验证请求-响应）

## 4. 主站引擎（modbussim-core）

- [x] 4.1 定义 MasterConnection 结构体（目标地址、端口、从站 ID、连接状态）
- [x] 4.2 实现基于 tokio-modbus 的 TCP Client 连接/断开
- [x] 4.3 实现读操作：FC01/02/03/04，返回解析后的寄存器值
- [x] 4.4 实现写操作：FC05/06/15/16，发送写入请求并返回结果
- [x] 4.5 实现轮询调度：可配置间隔的定时读取，支持启动/停止
- [x] 4.6 实现超时处理和异常响应解析
- [x] 4.7 为主站引擎编写集成测试（配合子站引擎做回环测试）

## 5. 通信日志（modbussim-core）

- [x] 5.1 定义 LogEntry 结构体（timestamp、direction、function_code、detail、raw_bytes）
- [x] 5.2 实现日志收集器：在子站和主站引擎中插入日志捕获点
- [x] 5.3 实现日志缓冲区（上限 10000 条，超出自动移除最早记录）
- [x] 5.4 实现日志导出为 CSV/TXT 格式

## 6. 配置管理（modbussim-core）

- [x] 6.1 定义 JSON 配置文件的 serde 数据结构（连接配置 + 从站设备 + 寄存器定义）
- [x] 6.2 实现配置导出：序列化当前状态为 JSON 文件
- [x] 6.3 实现配置导入：反序列化 JSON 文件并校验格式，返回友好的错误信息
- [x] 6.4 实现应用状态持久化：退出时自动保存、启动时自动恢复（使用 Tauri 的 app data 目录）
- [x] 6.5 为配置的序列化/反序列化编写测试

## 7. 实用工具（modbussim-core）

- [x] 7.1 实现 Modbus 协议地址与 PLC 地址双向转换
- [x] 7.2 实现 CRC-16（Modbus RTU）校验计算
- [x] 7.3 实现 LRC（Modbus ASCII）校验计算
- [x] 7.4 实现十六进制字符串解析（支持空格/逗号/无分隔符格式）

## 8. Tauri IPC 层（modbussim-app）

- [x] 8.1 定义 Tauri Commands：连接管理（创建/启动/停止/删除连接）
- [x] 8.2 定义 Tauri Commands：从站设备管理（添加/删除/修改从站）
- [x] 8.3 定义 Tauri Commands：寄存器操作（添加/删除/修改寄存器定义、读写寄存器值）
- [x] 8.4 定义 Tauri Commands：主站操作（连接/断开/读取/写入/轮询控制）
- [x] 8.5 定义 Tauri Commands：配置导入/导出、工具函数
- [x] 8.6 定义 Tauri Events：通信日志推送、连接状态变化、寄存器值更新
- [x] 8.7 实现应用状态管理（Tauri State，持有所有连接和设备的运行时状态）

## 9. 前端 — 布局与导航

- [x] 9.1 实现应用整体布局：左侧边栏 + 主内容区
- [x] 9.2 实现侧边栏：Slave/Master/Log/Tools 导航
- [x] 9.3 实现主内容区的标签页切换（点击侧边栏项切换内容）

## 10. 前端 — 子站界面

- [x] 10.1 实现连接配置面板（监听地址、端口、启动/停止按钮、状态显示）
- [x] 10.2 实现从站设备管理（添加/删除从站、从站 ID 和名称编辑）
- [x] 10.3 实现寄存器表格（列：名称、地址、类型、值）
- [x] 10.4 实现寄存器值写入
- [x] 10.5 实现寄存器表格的导入/导出按钮

## 11. 前端 — 主站界面

- [x] 11.1 实现连接配置面板（目标地址、端口、从站 ID、连接/断开按钮、状态显示）
- [x] 11.2 实现读操作面板（功能码选择、起始地址、数量、读取按钮、结果展示）
- [x] 11.3 实现写操作面板（功能码选择、地址、值输入、写入按钮）
- [x] 11.4 实现轮询控制（间隔配置、启动/停止轮询）

## 12. 前端 — 通信日志与工具

- [x] 12.1 实现通信日志面板（时间戳/方向/功能码/详情列）
- [x] 12.2 实现日志清除和导出 CSV 按钮
- [x] 12.3 实现地址转换工具页面
- [x] 12.4 实现 CRC/LRC 校验计算工具页面

## 13. 跨平台打包与测试

- [x] 13.1 配置 Tauri 打包：macOS (.dmg) 和 Windows (.msi) 构建配置
- [x] 13.2 在 macOS 上完成端到端测试（子站+主站回环）
- [ ] 13.3 在 Windows 上完成端到端测试（子站+主站回环）
- [x] 13.4 验证配置文件跨平台兼容（macOS 导出的配置在 Windows 上可导入）
