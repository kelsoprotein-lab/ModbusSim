# ModbusSim

跨平台 Modbus TCP 仿真套件 — 包含 **ModbusSlave**（从站模拟器）和 **ModbusMaster**（主站工具），基于 Tauri、Rust 和 Vue 3 构建。

[English](README.md)

## 下载

**[最新版本下载](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest)**

| 平台 | ModbusSlave | ModbusMaster |
|------|------------|--------------|
| macOS (Apple Silicon) | [.dmg](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_aarch64.dmg) | [.dmg](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_aarch64.dmg) |
| macOS (Intel) | [.dmg](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_x64.dmg) | [.dmg](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_x64.dmg) |
| Windows | [.exe](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_x64-setup.exe) / [.msi](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_x64_en-US.msi) | [.exe](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_x64-setup.exe) / [.msi](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_x64_en-US.msi) |
| Linux | [.deb](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_amd64.deb) / [.AppImage](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_amd64.AppImage) / [.rpm](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave-0.1.0-1.x86_64.rpm) | [.deb](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_amd64.deb) / [.AppImage](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_amd64.AppImage) / [.rpm](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster-0.1.0-1.x86_64.rpm) |

## 功能

### ModbusSlave — 从站模拟器

- **Modbus TCP 从站仿真** — 在任意端口创建从站连接，每个连接支持多个从站设备
- **四种寄存器类型** — 线圈 (FC01)、离散输入 (FC02)、保持寄存器 (FC03)、输入寄存器 (FC04)
- **寄存器表格** — 地址搜索/过滤、行内值编辑、Ctrl/Shift 多选
- **值解析面板** — 多格式显示：有符号/无符号/十六进制/二进制 (16位)、Long/Float (32位)、Double (64位)
- **值格式切换** — 支持 U16、I16、HEX、BIN 显示格式快速切换
- **随机初始化** — 创建时可填充随机值，方便快速测试
- **通信日志** — 实时请求/响应日志记录，支持 CSV 导出
- **配置持久化** — 支持 JSON 格式导出/导入应用状态

### ModbusMaster — 主站工具

- **Modbus TCP 主站** — 同时连接多个 Modbus TCP 从站设备
- **扫描组** — 按寄存器组配置周期性轮询，自定义轮询间隔
- **多格式数据视图** — 无符号、有符号、十六进制、二进制、Float32 (AB CD / CD AB)
- **通信日志** — 按连接记录 TX/RX 日志，显示功能码
- **自动刷新** — 事件驱动 + 定时器双机制实时数据更新
- **连接状态跟踪** — 实时连接状态显示，自动检测重连

## 技术栈

- **后端**: Rust + [tokio-modbus](https://github.com/slowtec/tokio-modbus)
- **前端**: Vue 3 + TypeScript
- **框架**: [Tauri 2](https://tauri.app/)

## 项目结构

```
ModbusSim/
├── crates/
│   ├── modbussim-core/      # 核心 Modbus 库（从站、主站、寄存器、日志）
│   ├── modbussim-app/       # Tauri 应用 — ModbusSlave
│   └── modbusmaster-app/    # Tauri 应用 — ModbusMaster
├── frontend/                # Vue 3 前端 — ModbusSlave
└── master-frontend/         # Vue 3 前端 — ModbusMaster
```

## 开发

### 前置要求

- [Rust](https://rustup.rs/)（stable）
- [Node.js](https://nodejs.org/)（v18+）
- [Tauri CLI](https://tauri.app/start/prerequisites/)

### 启动开发

```bash
# ModbusSlave
cd frontend && npm install && cd ..
cargo tauri dev -p modbussim-app

# ModbusMaster
cd master-frontend && npm install && cd ..
cargo tauri dev -p modbusmaster-app
```

### 构建

```bash
cargo tauri build -p modbussim-app
cargo tauri build -p modbusmaster-app
```

### 运行测试

```bash
cargo test
```

## 许可证

MIT

## 作者

[kelsoprotein-lab](https://github.com/kelsoprotein-lab)
