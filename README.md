# ModbusSim

Cross-platform Modbus simulation suite — includes **ModbusSlave** and **ModbusMaster**, with two UIs: a **Tauri 2 + Vue 3** desktop build and a **native Rust + egui** build. Supports **TCP, TCP+TLS, RTU, ASCII, and RTU-over-TCP** transports.

[中文文档](README_CN.md)

## Download

**→ [Latest Release (v0.12.0)](https://github.com/kelsoprotein-lab/ModbusSim/releases/latest)**

Pre-built Tauri installers are attached to every GitHub Release:

| Platform | Installer formats |
|----------|-------------------|
| macOS (Apple Silicon / Intel) | `.dmg` |
| Windows (x64) | `.exe` (NSIS) / `.msi` |
| Linux (x64) | `.deb` / `.AppImage` / `.rpm` |

The egui edition is not packaged yet — build locally with `cargo run -p modbussim-egui` / `cargo run -p modbusmaster-egui` (see [Development](#development)).

## Unreleased (main branch)

- **egui i18n — EN / ZH toggle** — `modbussim-egui` now ships a "Language / 语言" menu with instant switching between English and 简体中文. The choice is persisted via eframe storage (`lang_v1`) so the next launch restores it. Translation lives in the new `modbussim-ui-shared::i18n` module (`Lang { Zh, En }` + `tr/tr1/tr2`); missing keys fall back to the key literal itself for easy dev-time diagnosis. Covers the menu bar, welcome hero, sidebar, TLS form, status bar, register-table toolbar + headers, data-source / jitter panels, and the add-slave / batch-add dialogs. Runtime error messages remain in Chinese for now.

## What's New in v0.12.0

- **egui edition** — new pure-Rust desktop builds for both ModbusSlave and ModbusMaster (`modbussim-egui`, `modbusmaster-egui`), sharing a `modbussim-ui-shared` crate (theme / fonts / log panel / value panel / project I/O).
- **Slave register jitter** — per-slave `JitterConfig` with a background runner (bool flip probability, u16 % drift); serialized to `.modbusproj` with backward-compat defaults.
- **Register table UX** — search + address jump (`Cmd/Ctrl+F`), bool `○ / ●` toggle, 4-column layout (Address / Value / Name / Comment), `RegViewCache` for name/comment lookup.
- **Data sources** — egui slave adds Sawtooth / Triangle / CsvPlayback to the quick-add menu.
- **Visual refresh** — flat layered "redisant industrial" light theme, refined Darcula dark palette, region-based frame hierarchy (L0/L1/L2), iOS-style toggle switch.
- **CI** — `ci-egui.yml` verifies egui binaries build on macOS / Linux / Windows.

## Features

### ModbusSlave — Slave Simulator

- **Multi-Transport Support** — TCP, TCP+TLS, RTU (serial), ASCII (serial), RTU-over-TCP
- **Modbus TCP over TLS** — TLS 1.2+ encryption, PEM and PKCS#12 certificate formats, optional mTLS (mutual authentication with client certificates)
- **Multiple Slave Devices** — Create connections on any port, add multiple slave devices per connection
- **Four Register Types** — Coils (FC01), Discrete Inputs (FC02), Holding Registers (FC03), Input Registers (FC04)
- **Full Protocol Support** — Read (FC01-04), Write Single (FC05/06), Write Multiple (FC15/16), with Modbus exception codes
- **Register Table** — Address search/filter, inline value editing, Ctrl/Shift multi-select, virtual scrolling (20,000+ registers), multi-format display (Auto / U16 / I16 / Hex / Bin / Float32 with 4 byte orders)
- **Default Initialization** — New slaves pre-fill addresses 0-20,000 across all four register types, batch-add supports up to 50,000 entries per operation
- **Value Panel** — Multi-format display: Signed/Unsigned/Hex/Binary (16-bit), Long/Float (32-bit), Double (64-bit), all byte orders (AB CD / CD AB / BA DC / DC BA)
- **Dynamic Data Sources** — Simulate changing register values: Fixed, Random, Sine wave, Sawtooth, Triangle, Counter, CSV playback
- **Communication Log** — Real-time TX/RX logging with search, direction/function-code filtering, and CSV export
- **Project Files** — Save/load complete configurations as `.modbusproj` files for quick scenario switching
- **Serial Port Support** — Auto-detect system serial ports, configurable baud rate, data bits, stop bits, parity

### ModbusMaster — Master Tool

- **Multi-Transport Support** — TCP, TCP+TLS, RTU (serial), ASCII (serial), RTU-over-TCP
- **Modbus TCP over TLS** — TLS 1.2+ encryption, PEM and PKCS#12 certificate formats, accept-invalid-certs mode for self-signed certificate testing
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
- **Shared Vue Components** — Common composables and types shared between both Tauri apps via the `shared-frontend` npm workspace
- **Shared egui Widgets** — `modbussim-ui-shared` crate: flat layered theme, CJK font injection, log panel, value panel, register search, `.modbusproj` project I/O

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
| TCP+TLS | TLS over TCP | MBAP header | Secure Modbus TCP (TLS 1.2+) |
| RTU | Serial port | Slave ID + CRC-16 | RS-485/RS-232 devices |
| ASCII | Serial port | `:` + hex + LRC + CRLF | Legacy serial devices |
| RTU-over-TCP | TCP/IP socket | Slave ID + CRC-16 | Industrial gateways |

## Tech Stack

- **Core (Rust)**: [tokio-modbus](https://github.com/slowtec/tokio-modbus) + [tokio-serial](https://github.com/berkowski/tokio-serial)
- **TLS**: [native-tls](https://crates.io/crates/native-tls) (system TLS: macOS Security.framework, Linux OpenSSL, Windows SChannel)
- **Tauri UI**: Vue 3 + TypeScript + [@tanstack/vue-virtual](https://tanstack.com/virtual) on [Tauri 2](https://tauri.app/)
- **egui UI**: [eframe](https://crates.io/crates/eframe) + [egui](https://github.com/emilk/egui) + egui_extras / egui-modal / egui-toast
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
│   │   │   ├── transport.rs   # Transport enum, serial config, TLS config, port enumeration
│   │   │   ├── mbap.rs        # MBAP frame encoding/decoding (for TLS mode)
│   │   │   ├── tls_slave.rs   # TLS-enabled Modbus TCP slave server
│   │   │   ├── tls_master.rs  # TLS-enabled Modbus TCP master client
│   │   │   ├── register.rs    # Register types, encoding/decoding
│   │   │   ├── data_source.rs # Dynamic data sources for register simulation
│   │   │   ├── reconnect.rs   # Reconnect policy with exponential backoff
│   │   │   ├── error.rs       # Unified ModbusError enum
│   │   │   ├── project.rs     # .modbusproj file save/load/migrate
│   │   │   ├── log_collector.rs # Thread-safe log ring buffer
│   │   │   └── ...
│   ├── modbussim-app/         # Tauri app — ModbusSlave
│   ├── modbusmaster-app/      # Tauri app — ModbusMaster
│   ├── modbussim-egui/        # egui native app — ModbusSlave
│   ├── modbusmaster-egui/     # egui native app — ModbusMaster
│   └── modbussim-ui-shared/   # Shared egui widgets: theme, fonts, log_panel, value_panel, project
├── shared-frontend/           # Shared Vue composables and components (Tauri UI)
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

### Setup — Tauri edition

```bash
# Install frontend dependencies (npm workspaces)
npm install

# ModbusSlave (dev mode)
cd crates/modbussim-app && cargo tauri dev

# ModbusMaster (dev mode)
cd crates/modbusmaster-app && cargo tauri dev
```

### Build — Tauri edition

```bash
cd crates/modbussim-app && cargo tauri build
cd crates/modbusmaster-app && cargo tauri build
```

### Run — egui edition

```bash
# ModbusSlave (native egui)
cargo run -p modbussim-egui --release

# ModbusMaster (native egui)
cargo run -p modbusmaster-egui --release
```

### Run Tests

```bash
cargo test --workspace
```

## License

MIT

## Author

[kelsoprotein-lab](https://github.com/kelsoprotein-lab)
