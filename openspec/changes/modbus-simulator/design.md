## 上下文

团队需要一个跨平台 Modbus 模拟器用于工业自动化开发调试。项目从零开始构建，无历史代码约束。技术栈已确定为 Rust + Tauri + Web 前端。不使用 Qt。

关键约束：
- 必须支持 macOS 和 Windows
- 团队使用，需便于分发和配置共享
- 主站和子站为两个独立模式
- 一个连接下支持多个从站 ID

## 目标 / 非目标

**目标：**
- 提供完整的 Modbus TCP 主站和子站模拟功能
- 支持一个连接下多从站 ID，各自拥有独立寄存器映射
- 寄存器支持多种数据类型解读（bool/uint16/int16/uint32/int32/float32）和字节序选择
- 提供实时通信日志，方便调试和问题排查
- 设备配置可通过 JSON 文件导入/导出，实现团队共享
- 跨平台打包分发（macOS .dmg、Windows .msi）

**非目标：**
- P0 阶段不实现 RTU 串口支持（列入 P1）
- 不实现实时数据图表（列入 P2）
- 不实现数据模拟/缓动函数（列入 P1）
- 不实现错误/异常注入（列入 P2）
- 不做 Modbus 协议栈的自研实现，使用现有库

## 决策

### D1: 编程语言 — Rust

**选择**: Rust
**考虑过**: C#(.NET + Avalonia)、C++(无 Qt)、Go、Python
**理由**:
- 编译为原生二进制，无运行时依赖，分发最简单
- 内存安全，适合长时间运行的通信工具
- tokio 异步生态成熟，天然适合网络 I/O 密集的 Modbus 通信
- 团队有意采用 Rust

### D2: 桌面框架 — Tauri v2

**选择**: Tauri v2
**考虑过**: egui、iced、C++ Dear ImGui
**理由**:
- Rust 后端 + Web 前端，UI 自由度最高
- 使用系统 WebView，打包体积小（~5-10MB）
- 内建跨平台打包工具（msi/dmg/AppImage）
- 团队有前端经验，可以利用现有 Web 技术栈
- Qt 不可用，排除了 C++ 最佳 GUI 方案

### D3: Modbus 协议库 — tokio-modbus

**选择**: tokio-modbus
**考虑过**: rodbus、libmodbus(FFI)
**理由**:
- 纯 Rust 实现，无 C 依赖，编译简单
- 基于 tokio 异步运行时，与 Tauri 的异步架构一致
- 支持 TCP client/server 和 RTU client/server（为 P1 做准备）
- 社区活跃，维护持续

### D4: 项目结构 — Cargo Workspace

**选择**: Cargo workspace 多 crate 结构
```
modbussim/
├── Cargo.toml              (workspace root)
├── crates/
│   ├── modbussim-core/     (协议引擎、寄存器模型、配置)
│   └── modbussim-app/      (Tauri 应用，含前端)
└── frontend/               (Web UI 源码)
```
**理由**:
- core 与 GUI 分离：核心逻辑可独立测试，不依赖 Tauri
- 未来可扩展 CLI 版本（modbussim-cli crate）
- 前端代码独立目录，构建流程清晰

### D5: 前端框架 — 待定（Svelte / Vue / React）

**选择**: 待团队确认，倾向 Svelte
**理由**:
- Svelte 是 Tauri 官方推荐搭档，编译时框架，包体积最小
- Vue 和 React 也可行，取决于团队熟悉度
- 均可配合 UI 组件库实现表格、表单等界面需求

### D6: 前后端通信 — Tauri IPC (Commands + Events)

**选择**: Tauri Commands（前端调后端）+ Tauri Events（后端推前端）
**理由**:
- Tauri 原生机制，类型安全（配合 specta/ts-rs 生成 TypeScript 类型）
- Commands 用于：启动/停止连接、读写寄存器、导入/导出配置
- Events 用于：通信日志实时推送、连接状态变化通知、寄存器值更新

### D7: 配置文件格式 — JSON

**选择**: JSON
**考虑过**: TOML、YAML
**理由**:
- 前端原生支持解析/生成
- 团队成员熟悉
- 方便与其他工具互操作

## 风险 / 权衡

| 风险 | 缓解措施 |
|------|---------|
| tokio-modbus 的 server 端 API 可能不够灵活（多从站路由） | 提前做原型验证；必要时在 tokio-modbus 之上封装路由层，根据从站 ID 分发请求到对应的寄存器映射 |
| Tauri v2 在 macOS 和 Windows 上 WebView 行为差异 | 使用标准 Web API，避免平台特定特性；CI 同时在两个平台构建和测试 |
| 前端框架尚未最终确定 | core crate 与前端完全解耦，更换前端框架不影响后端逻辑 |
| Rust 学习曲线对部分团队成员偏高 | core/app 分离降低了同时需要理解的代码量；前端开发者只需关注 Web 部分 |
| 串口（RTU）跨平台兼容性（P1 阶段） | P0 先聚焦 TCP，RTU 阶段使用 tokio-serial，提前调研 macOS/Windows 串口枚举差异 |
