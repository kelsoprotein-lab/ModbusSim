# ModbusSim

Cross-platform Modbus simulation suite — includes **ModbusSlave** and **ModbusMaster**, built with Tauri 2, Rust, and Vue 3. Supports **TCP, RTU, ASCII, and RTU-over-TCP** transports.

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

- **Multi-Transport Support** — TCP, RTU (serial), ASCII (serial), RTU-over-TCP
- **Multiple Slave Devices** — Create connections on any port, add multiple slave devices per connection
- **Four Register Types** — Coils (FC01), Discrete Inputs (FC02), Holding Registers (FC03), Input Registers (FC04)
- **Full Protocol Support** — Read (FC01-04), Write Single (FC05/06), Write Multiple (FC15/16), with Modbus exception codes
- **Register Table** — Address search/filter, inline value editing, Ctrl/Shift multi-select, virtual scrolling (10,000+ registers)
- **Value Panel** — Multi-format display: Signed/Unsigned/Hex/Binary (16-bit), Long/Float (32-bit), Double (64-bit), all byte orders (AB CD / CD AB / BA DC / DC BA)
- **Dynamic Data Sources** — Simulate changing register values: Fixed, Random, Sine wave, Sawtooth, Triangle, Counter, CSV playback
- **Communication Log** — Real-time TX/RX logging with search, direction/function-code filtering, and CSV export
- **Project Files** — Save/load complete configurations as `.modbusproj` files for quick scenario switching
- **Serial Port Support** — Auto-detect system serial ports, configurable baud rate, data bits, stop bits, parity

### ModbusMaster — Master Tool

- **Multi-Transport Support** — TCP, RTU (serial), ASCII (serial), RTU-over-TCP
- **Scan Groups** — Configure periodic polling with custom intervals per register group, per-group slave ID override
- **Device Discovery** — Slave ID scan (1-247), register address scan, auto-add discovered devices to scan groups
- **Multi-format Data View** — Unsigned, Signed, Hex, Binary, Float32 (AB CD / CD AB), virtual scrolling
- **Write Operations** — Write single/multiple coils and registers (FC05/06/15/16)
- **Communication Log** — TX/RX logging with search/filter (direction, function code, text)
- **Auto-Reconnect** — Configurable reconnection with exponential backoff (1s → 2s → 4s → ... → 30s max)
- **Project Files** — Save/load connection and scan group configurations
- **Connection-on-Scan** — Auto-prompt to scan devices after successful connection

### Shared Architecture

- **Unified Error System** — Structured `ModbusError` with categorized error types (connection/protocol/application), serialized to JSON for frontend parsing
- **Shared Frontend Components** — Common composables and types shared between both apps via `shared-frontend` npm workspace

## Supported Function Codes

| Code | Function | Slave (Server) | Master (Client) |
|------|----------|:-:|:-:|
| FC01 | Read Coils | Read | Read/Poll |
| FC02 | Read Discrete Inputs | Read | Read/Poll |
| FC03 | Read Holding Registers | Read | Read/Poll |
| FC04 | Read Input Registers | Read | Read/Poll |
| FC05 | Write Single Coil | Write | Write |
| FC06 | Write Single Register | Write | Write |
| FC15 | Write Multiple Coils | Write | Write |
| FC16 | Write Multiple Registers | Write | Write |

## Transport Modes

| Mode | Transport | Framing | Use Case |
|------|-----------|---------|----------|
| TCP | TCP/IP socket | MBAP header | Standard Modbus TCP |
| RTU | Serial port | Slave ID + CRC-16 | RS-485/RS-232 devices |
| ASCII | Serial port | `:` + hex + LRC + CRLF | Legacy serial devices |
| RTU-over-TCP | TCP/IP socket | Slave ID + CRC-16 | Industrial gateways |

## Tech Stack

- **Backend**: Rust + [tokio-modbus](https://github.com/slowtec/tokio-modbus) + [tokio-serial](https://github.com/berkowski/tokio-serial)
- **Frontend**: Vue 3 + TypeScript + [@tanstack/vue-virtual](https://tanstack.com/virtual)
- **Framework**: [Tauri 2](https://tauri.app/)
- **Serial**: [serialport](https://crates.io/crates/serialport) for port enumeration

## Project Structure

```
ModbusSim/
├── crates/
│   ├── modbussim-core/        # Core library: protocol, transport, registers, logging
│   │   ├── src/
│   │   │   ├── slave.rs       # Slave connection (TCP/RTU/ASCII/RtuOverTcp dispatch)
│   │   │   ├── master.rs      # Master connection with multi-transport support
│   │   │   ├── frame.rs       # RTU/ASCII frame encode/decode
│   │   │   ├── pdu.rs         # Modbus PDU request/response parsing
│   │   │   ├── transport.rs   # Transport enum, serial config, port enumeration
│   │   │   ├── register.rs    # Register types, encoding/decoding
│   │   │   ├── data_source.rs # Dynamic data sources for register simulation
│   │   │   ├── reconnect.rs   # Reconnect policy with exponential backoff
│   │   │   ├── error.rs       # Unified ModbusError enum
│   │   │   ├── project.rs     # .modbusproj file save/load/migrate
│   │   │   ├── log_collector.rs # Thread-safe log ring buffer
│   │   │   └── ...
│   ├── modbussim-app/         # Tauri app — ModbusSlave
│   └── modbusmaster-app/      # Tauri app — ModbusMaster
├── shared-frontend/           # Shared Vue composables and components
│   └── src/
│       ├── composables/       # useDialog, useValueFormat, useLogPanel, useLogFilter, useErrorHandler
│       ├── components/        # AppDialog
│       └── types/             # Shared TypeScript types
├── frontend/                  # Vue 3 frontend — ModbusSlave
└── master-frontend/           # Vue 3 frontend — ModbusMaster
```

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (v18+)
- [Tauri CLI](https://tauri.app/start/prerequisites/)

### Setup

```bash
# Install frontend dependencies (npm workspaces)
npm install

# ModbusSlave (dev mode)
cd crates/modbussim-app && cargo tauri dev

# ModbusMaster (dev mode)
cd crates/modbusmaster-app && cargo tauri dev
```

### Build

```bash
cd crates/modbussim-app && cargo tauri build
cd crates/modbusmaster-app && cargo tauri build
```

### Run Tests

```bash
cargo test --workspace
```

## License

MIT

## Author

[kelsoprotein-lab](https://github.com/kelsoprotein-lab)
