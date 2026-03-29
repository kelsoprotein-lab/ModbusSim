# ModbusSim

Cross-platform Modbus TCP simulation suite — includes **ModbusSlave** and **ModbusMaster**, built with Tauri, Rust, and Vue 3.

[中文文档](README_CN.md)

## Download

**[Latest Release](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest)**

| Platform | ModbusSlave | ModbusMaster |
|----------|------------|--------------|
| macOS (Apple Silicon) | [.dmg](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_aarch64.dmg) | [.dmg](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_aarch64.dmg) |
| macOS (Intel) | [.dmg](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_x64.dmg) | [.dmg](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_x64.dmg) |
| Windows | [.exe](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_x64-setup.exe) / [.msi](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_x64_en-US.msi) | [.exe](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_x64-setup.exe) / [.msi](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_x64_en-US.msi) |
| Linux | [.deb](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_amd64.deb) / [.AppImage](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave_0.1.0_amd64.AppImage) / [.rpm](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusSlave-0.1.0-1.x86_64.rpm) | [.deb](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_amd64.deb) / [.AppImage](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster_0.1.0_amd64.AppImage) / [.rpm](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest/download/ModbusMaster-0.1.0-1.x86_64.rpm) |

## Features

### ModbusSlave — Slave Simulator

- **Modbus TCP Slave Simulation** — Create slave connections on any port, add multiple slave devices per connection
- **Four Register Types** — Coils (FC01), Discrete Inputs (FC02), Holding Registers (FC03), Input Registers (FC04)
- **Register Table** — Address search/filter, inline value editing, Ctrl/Shift multi-select
- **Value Panel** — Multi-format display: Signed/Unsigned/Hex/Binary (16-bit), Long/Float (32-bit), Double (64-bit)
- **Value Format Toggle** — Switch register display between U16, I16, HEX, BIN
- **Random Initialization** — Fill registers with random values for quick testing
- **Communication Log** — Real-time request/response logging with CSV export
- **Config Persistence** — Export/import application state as JSON

### ModbusMaster — Master Tool

- **Modbus TCP Master** — Connect to multiple Modbus TCP slave devices simultaneously
- **Scan Groups** — Configure periodic polling with custom intervals per register group
- **Multi-format Data View** — Unsigned, Signed, Hex, Binary, Float32 (AB CD / CD AB)
- **Communication Log** — TX/RX logging per connection with function code display
- **Auto-refresh** — Real-time data updates with event-driven and timer-based refresh
- **Connection State Tracking** — Live connection status with auto-reconnect detection

## Tech Stack

- **Backend**: Rust + [tokio-modbus](https://github.com/slowtec/tokio-modbus)
- **Frontend**: Vue 3 + TypeScript
- **Framework**: [Tauri 2](https://tauri.app/)

## Project Structure

```
ModbusSim/
├── crates/
│   ├── modbussim-core/      # Core Modbus library (slave, master, register, log)
│   ├── modbussim-app/       # Tauri app — ModbusSlave
│   └── modbusmaster-app/    # Tauri app — ModbusMaster
├── frontend/                # Vue 3 frontend — ModbusSlave
└── master-frontend/         # Vue 3 frontend — ModbusMaster
```

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (v18+)
- [Tauri CLI](https://tauri.app/start/prerequisites/)

### Setup

```bash
# ModbusSlave
cd frontend && npm install && cd ..
cargo tauri dev -p modbussim-app

# ModbusMaster
cd master-frontend && npm install && cd ..
cargo tauri dev -p modbusmaster-app
```

### Build

```bash
cargo tauri build -p modbussim-app
cargo tauri build -p modbusmaster-app
```

### Run Tests

```bash
cargo test
```

## License

MIT

## Author

[kelsoprotein-lab](https://github.com/kelsoprotein-lab)
