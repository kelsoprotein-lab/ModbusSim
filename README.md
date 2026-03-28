# ModbusSim

A cross-platform Modbus TCP slave simulator built with Tauri, Rust, and Vue 3.

## Features

- **Modbus TCP Slave Simulation** — Create slave connections on any port, add multiple slave devices per connection
- **Four Register Types** — Full support for Coils (FC1), Discrete Inputs (FC2), Input Registers (FC4), and Holding Registers (FC3)
- **Tree Navigation** — Hierarchical view: Connection > Slave Device > Register Group
- **Register Table** — Scrollable table with address search/filter, inline value editing, Ctrl/Shift multi-select
- **Value Panel** — Multi-format interpretation: Signed/Unsigned/Hex/Binary (16-bit), Long/Float (32-bit), Double (64-bit)
- **Random Initialization** — Option to fill registers with random values on creation for quick testing
- **Address Display** — Toggle between hexadecimal and decimal address display
- **Communication Log** — Real-time Modbus request/response logging with CSV export
- **Config Persistence** — Export/import application state as JSON

## Tech Stack

- **Backend**: Rust + [tokio-modbus](https://github.com/slowtec/tokio-modbus)
- **Frontend**: Vue 3 + TypeScript
- **Framework**: [Tauri 2](https://tauri.app/)

## Project Structure

```
ModbusSim/
├── crates/
│   ├── modbussim-core/     # Core Modbus library (slave, register, tools)
│   └── modbussim-app/      # Tauri app (commands, state)
└── frontend/               # Vue 3 frontend
    └── src/
        ├── components/     # UI components
        └── composables/    # Shared logic
```

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) (v18+)
- [Tauri CLI](https://tauri.app/start/prerequisites/)

### Setup

```bash
# Install frontend dependencies
cd frontend && npm install && cd ..

# Run in development mode
cargo tauri dev

# Build for production
cargo tauri build
```

### Run Tests

```bash
cargo test
```

## License

MIT

## Author

[kelsoprotein-lab](https://github.com/kelsoprotein-lab)
