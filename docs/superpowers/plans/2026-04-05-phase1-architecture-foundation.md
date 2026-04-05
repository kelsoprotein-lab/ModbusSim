# Phase 1: Architecture Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate code duplication between master/slave apps by extracting shared code into core crate and shared frontend package, and add project file persistence.

**Architecture:** Extract parse functions and log command helpers into `modbussim-core`. Create `shared-frontend/` npm workspace package with shared Vue composables, components, and types. Add `.modbusproj` project file save/load to core and both apps.

**Tech Stack:** Rust (modbussim-core), Vue 3 + TypeScript (shared-frontend), npm workspaces, Tauri 2 file dialog

---

## File Structure

### Backend (Rust)

| Action | File | Responsibility |
|--------|------|---------------|
| Create | `crates/modbussim-core/src/parse.rs` | Unified parse functions for RegisterType, Endian, DataType, ReadFunction |
| Create | `crates/modbussim-core/src/log_helpers.rs` | Generic log query/export/clear functions operating on LogCollector |
| Create | `crates/modbussim-core/src/project.rs` | ProjectFile struct, save/load/migrate, serde definitions |
| Modify | `crates/modbussim-core/src/lib.rs` | Add `pub mod parse; pub mod log_helpers; pub mod project;` |
| Modify | `crates/modbussim-app/src/commands.rs:303-333` | Remove local parse_* fns, use core::parse |
| Modify | `crates/modbussim-app/src/commands.rs:538-573` | Thin log commands calling core::log_helpers |
| Modify | `crates/modbusmaster-app/src/commands.rs:25-42` | Remove local string_to_function/function_to_string, use core::parse |
| Modify | `crates/modbusmaster-app/src/commands.rs:770-808` | Thin log commands calling core::log_helpers |
| Modify | `crates/modbusmaster-app/src/commands.rs:947-955` | Remove duplicate parse_read_function |

### Frontend (Vue/TypeScript)

| Action | File | Responsibility |
|--------|------|---------------|
| Create | `shared-frontend/package.json` | npm workspace package config |
| Create | `shared-frontend/tsconfig.json` | TypeScript config |
| Create | `shared-frontend/src/types/modbus.ts` | Shared TS types: LogEntry, DialogState, DialogMode |
| Create | `shared-frontend/src/composables/useDialog.ts` | Dialog state management (extracted from both apps) |
| Create | `shared-frontend/src/composables/useValueFormat.ts` | 16/32/64-bit value formatting composables |
| Create | `shared-frontend/src/composables/useLogPanel.ts` | Log load/clear/export logic |
| Create | `shared-frontend/src/components/AppDialog.vue` | Shared dialog component |
| Create | `package.json` (root) | npm workspaces config |
| Modify | `frontend/package.json` | Add shared-frontend dependency |
| Modify | `master-frontend/package.json` | Add shared-frontend dependency |
| Modify | `frontend/tsconfig.app.json` | Add shared-frontend path |
| Modify | `master-frontend/tsconfig.app.json` | Add shared-frontend path |
| Modify | `frontend/src/composables/useDialog.ts` | Re-export from shared |
| Modify | `frontend/src/components/AppDialog.vue` | Re-export from shared |
| Modify | `frontend/src/components/LogPanel.vue` | Use shared useLogPanel composable |
| Modify | `frontend/src/components/ValuePanel.vue` | Use shared useValueFormat composable |
| Modify | `master-frontend/src/composables/useDialog.ts` | Re-export from shared |
| Modify | `master-frontend/src/components/AppDialog.vue` | Re-export from shared |
| Modify | `master-frontend/src/components/LogPanel.vue` | Use shared useLogPanel composable |
| Modify | `master-frontend/src/components/ValuePanel.vue` | Use shared useValueFormat composable |

---

## Task 1: Extract parse functions to core

**Files:**
- Create: `crates/modbussim-core/src/parse.rs`
- Modify: `crates/modbussim-core/src/lib.rs`
- Test: `crates/modbussim-core/src/parse.rs` (inline tests)

- [ ] **Step 1: Write failing tests for parse functions**

```rust
// crates/modbussim-core/src/parse.rs

use crate::register::{RegisterType, DataType, Endian};
use crate::master::ReadFunction;

/// Parse a register type string into RegisterType.
pub fn parse_register_type(s: &str) -> Result<RegisterType, String> {
    todo!()
}

/// Parse an endian string into Endian.
pub fn parse_endian(s: &str) -> Result<Endian, String> {
    todo!()
}

/// Parse a data type string into DataType.
pub fn parse_data_type(s: &str) -> Result<DataType, String> {
    todo!()
}

/// Parse a read function string into ReadFunction.
pub fn parse_read_function(s: &str) -> Result<ReadFunction, String> {
    todo!()
}

/// Convert a ReadFunction to its string representation.
pub fn read_function_to_string(f: ReadFunction) -> &'static str {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_register_type_valid() {
        assert_eq!(parse_register_type("coil").unwrap(), RegisterType::Coil);
        assert_eq!(parse_register_type("discrete_input").unwrap(), RegisterType::DiscreteInput);
        assert_eq!(parse_register_type("input_register").unwrap(), RegisterType::InputRegister);
        assert_eq!(parse_register_type("holding_register").unwrap(), RegisterType::HoldingRegister);
    }

    #[test]
    fn test_parse_register_type_invalid() {
        assert!(parse_register_type("unknown").is_err());
        assert!(parse_register_type("").is_err());
    }

    #[test]
    fn test_parse_endian_valid() {
        assert_eq!(parse_endian("big").unwrap(), Endian::Big);
        assert_eq!(parse_endian("little").unwrap(), Endian::Little);
        assert_eq!(parse_endian("mid_big").unwrap(), Endian::MidBig);
        assert_eq!(parse_endian("mid_little").unwrap(), Endian::MidLittle);
    }

    #[test]
    fn test_parse_endian_invalid() {
        assert!(parse_endian("unknown").is_err());
    }

    #[test]
    fn test_parse_data_type_valid() {
        assert_eq!(parse_data_type("bool").unwrap(), DataType::Bool);
        assert_eq!(parse_data_type("uint16").unwrap(), DataType::UInt16);
        assert_eq!(parse_data_type("int16").unwrap(), DataType::Int16);
        assert_eq!(parse_data_type("uint32").unwrap(), DataType::UInt32);
        assert_eq!(parse_data_type("int32").unwrap(), DataType::Int32);
        assert_eq!(parse_data_type("float32").unwrap(), DataType::Float32);
    }

    #[test]
    fn test_parse_data_type_invalid() {
        assert!(parse_data_type("float64").is_err());
    }

    #[test]
    fn test_parse_read_function_valid() {
        assert_eq!(parse_read_function("read_coils").unwrap(), ReadFunction::ReadCoils);
        assert_eq!(parse_read_function("read_discrete_inputs").unwrap(), ReadFunction::ReadDiscreteInputs);
        assert_eq!(parse_read_function("read_holding_registers").unwrap(), ReadFunction::ReadHoldingRegisters);
        assert_eq!(parse_read_function("read_input_registers").unwrap(), ReadFunction::ReadInputRegisters);
    }

    #[test]
    fn test_parse_read_function_invalid() {
        assert!(parse_read_function("write_coils").is_err());
    }

    #[test]
    fn test_read_function_to_string() {
        assert_eq!(read_function_to_string(ReadFunction::ReadCoils), "read_coils");
        assert_eq!(read_function_to_string(ReadFunction::ReadDiscreteInputs), "read_discrete_inputs");
        assert_eq!(read_function_to_string(ReadFunction::ReadHoldingRegisters), "read_holding_registers");
        assert_eq!(read_function_to_string(ReadFunction::ReadInputRegisters), "read_input_registers");
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

Add to `crates/modbussim-core/src/lib.rs`:

```rust
pub mod register;
pub mod slave;
pub mod master;
pub mod log_entry;
pub mod log_collector;
pub mod config;
pub mod tools;
pub mod parse;
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd crates/modbussim-core && cargo test parse`
Expected: FAIL with "not yet implemented"

- [ ] **Step 4: Implement parse functions**

Replace the `todo!()` bodies in `crates/modbussim-core/src/parse.rs`:

```rust
use crate::register::{RegisterType, DataType, Endian};
use crate::master::ReadFunction;

/// Parse a register type string into RegisterType.
pub fn parse_register_type(s: &str) -> Result<RegisterType, String> {
    match s {
        "coil" => Ok(RegisterType::Coil),
        "discrete_input" => Ok(RegisterType::DiscreteInput),
        "input_register" => Ok(RegisterType::InputRegister),
        "holding_register" => Ok(RegisterType::HoldingRegister),
        _ => Err(format!("unknown register type: {}", s)),
    }
}

/// Parse an endian string into Endian.
pub fn parse_endian(s: &str) -> Result<Endian, String> {
    match s {
        "big" => Ok(Endian::Big),
        "little" => Ok(Endian::Little),
        "mid_big" => Ok(Endian::MidBig),
        "mid_little" => Ok(Endian::MidLittle),
        _ => Err(format!("unknown endian: {}", s)),
    }
}

/// Parse a data type string into DataType.
pub fn parse_data_type(s: &str) -> Result<DataType, String> {
    match s {
        "bool" => Ok(DataType::Bool),
        "uint16" => Ok(DataType::UInt16),
        "int16" => Ok(DataType::Int16),
        "uint32" => Ok(DataType::UInt32),
        "int32" => Ok(DataType::Int32),
        "float32" => Ok(DataType::Float32),
        _ => Err(format!("unknown data type: {}", s)),
    }
}

/// Parse a read function string into ReadFunction.
pub fn parse_read_function(s: &str) -> Result<ReadFunction, String> {
    match s {
        "read_coils" => Ok(ReadFunction::ReadCoils),
        "read_discrete_inputs" => Ok(ReadFunction::ReadDiscreteInputs),
        "read_holding_registers" => Ok(ReadFunction::ReadHoldingRegisters),
        "read_input_registers" => Ok(ReadFunction::ReadInputRegisters),
        _ => Err(format!("unknown function: {}", s)),
    }
}

/// Convert a ReadFunction to its string representation.
pub fn read_function_to_string(f: ReadFunction) -> &'static str {
    match f {
        ReadFunction::ReadCoils => "read_coils",
        ReadFunction::ReadDiscreteInputs => "read_discrete_inputs",
        ReadFunction::ReadHoldingRegisters => "read_holding_registers",
        ReadFunction::ReadInputRegisters => "read_input_registers",
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd crates/modbussim-core && cargo test parse`
Expected: All 7 tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/modbussim-core/src/parse.rs crates/modbussim-core/src/lib.rs
git commit -m "feat(core): add unified parse module for RegisterType, Endian, DataType, ReadFunction"
```

---

## Task 2: Extract log helpers to core

**Files:**
- Create: `crates/modbussim-core/src/log_helpers.rs`
- Modify: `crates/modbussim-core/src/lib.rs`
- Test: `crates/modbussim-core/src/log_helpers.rs` (inline tests)

- [ ] **Step 1: Write failing tests for log helpers**

```rust
// crates/modbussim-core/src/log_helpers.rs

use crate::log_collector::LogCollector;
use crate::log_entry::LogEntry;

/// Get all log entries from a collector.
pub async fn get_all_logs(collector: &LogCollector) -> Vec<LogEntry> {
    todo!()
}

/// Get a paginated slice of log entries.
pub async fn get_logs_paginated(collector: &LogCollector, offset: usize, limit: usize) -> Vec<LogEntry> {
    todo!()
}

/// Export all logs as CSV string.
pub async fn export_csv(collector: &LogCollector) -> String {
    todo!()
}

/// Export all logs as plain text string.
pub async fn export_text(collector: &LogCollector) -> String {
    todo!()
}

/// Clear all log entries.
pub async fn clear_logs(collector: &LogCollector) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log_entry::{Direction, FunctionCode};
    use chrono::Utc;

    fn make_entry(detail: &str) -> LogEntry {
        LogEntry {
            timestamp: Utc::now(),
            direction: Direction::Tx,
            function_code: FunctionCode::ReadHoldingRegisters,
            slave_id: 1,
            detail: detail.to_string(),
        }
    }

    #[tokio::test]
    async fn test_get_all_logs_empty() {
        let collector = LogCollector::new();
        let logs = get_all_logs(&collector).await;
        assert!(logs.is_empty());
    }

    #[tokio::test]
    async fn test_get_all_logs_with_entries() {
        let collector = LogCollector::new();
        collector.add(make_entry("entry1")).await;
        collector.add(make_entry("entry2")).await;
        let logs = get_all_logs(&collector).await;
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].detail, "entry1");
        assert_eq!(logs[1].detail, "entry2");
    }

    #[tokio::test]
    async fn test_get_logs_paginated() {
        let collector = LogCollector::new();
        for i in 0..10 {
            collector.add(make_entry(&format!("entry{}", i))).await;
        }
        let page = get_logs_paginated(&collector, 3, 4).await;
        assert_eq!(page.len(), 4);
        assert_eq!(page[0].detail, "entry3");
        assert_eq!(page[3].detail, "entry6");
    }

    #[tokio::test]
    async fn test_get_logs_paginated_beyond_end() {
        let collector = LogCollector::new();
        collector.add(make_entry("entry0")).await;
        let page = get_logs_paginated(&collector, 5, 10).await;
        assert!(page.is_empty());
    }

    #[tokio::test]
    async fn test_export_csv_contains_header() {
        let collector = LogCollector::new();
        collector.add(make_entry("test")).await;
        let csv = export_csv(&collector).await;
        assert!(csv.starts_with("timestamp,"));
        assert!(csv.contains("test"));
    }

    #[tokio::test]
    async fn test_export_text_format() {
        let collector = LogCollector::new();
        collector.add(make_entry("test detail")).await;
        let text = export_text(&collector).await;
        assert!(text.contains("TX"));
        assert!(text.contains("FC03"));
        assert!(text.contains("test detail"));
    }

    #[tokio::test]
    async fn test_clear_logs() {
        let collector = LogCollector::new();
        collector.add(make_entry("entry")).await;
        assert_eq!(get_all_logs(&collector).await.len(), 1);
        clear_logs(&collector).await;
        assert!(get_all_logs(&collector).await.is_empty());
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

Add `pub mod log_helpers;` to `crates/modbussim-core/src/lib.rs`:

```rust
pub mod register;
pub mod slave;
pub mod master;
pub mod log_entry;
pub mod log_collector;
pub mod config;
pub mod tools;
pub mod parse;
pub mod log_helpers;
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd crates/modbussim-core && cargo test log_helpers`
Expected: FAIL with "not yet implemented"

- [ ] **Step 4: Implement log helpers**

Replace the `todo!()` bodies in `crates/modbussim-core/src/log_helpers.rs`:

```rust
use crate::log_collector::LogCollector;
use crate::log_entry::LogEntry;

/// Get all log entries from a collector.
pub async fn get_all_logs(collector: &LogCollector) -> Vec<LogEntry> {
    collector.get_all().await
}

/// Get a paginated slice of log entries.
pub async fn get_logs_paginated(collector: &LogCollector, offset: usize, limit: usize) -> Vec<LogEntry> {
    let all = collector.get_all().await;
    all.into_iter().skip(offset).take(limit).collect()
}

/// Export all logs as CSV string.
pub async fn export_csv(collector: &LogCollector) -> String {
    collector.export_csv().await
}

/// Export all logs as plain text string.
pub async fn export_text(collector: &LogCollector) -> String {
    collector.export_text().await
}

/// Clear all log entries.
pub async fn clear_logs(collector: &LogCollector) {
    collector.clear().await;
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd crates/modbussim-core && cargo test log_helpers`
Expected: All 7 tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/modbussim-core/src/log_helpers.rs crates/modbussim-core/src/lib.rs
git commit -m "feat(core): add log_helpers module with get/export/clear functions"
```

---

## Task 3: Migrate modbussim-app to use core parse & log_helpers

**Files:**
- Modify: `crates/modbussim-app/src/commands.rs`

- [ ] **Step 1: Replace local parse functions with core imports**

At the top of `crates/modbussim-app/src/commands.rs`, add the import:

```rust
use modbussim_core::parse::{parse_register_type, parse_endian, parse_data_type};
```

Then delete the three local functions at lines 303-333:
- `fn parse_register_type(s: &str) -> Result<RegisterType, String>`
- `fn parse_endian(s: &str) -> Result<Endian, String>`
- `fn parse_data_type(s: &str) -> Result<DataType, String>`

- [ ] **Step 2: Replace log commands with core log_helpers**

Add import:
```rust
use modbussim_core::log_helpers;
```

Replace `get_communication_logs` (lines 538-548):
```rust
#[tauri::command]
pub async fn get_communication_logs(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<LogEntry>, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;
    Ok(log_helpers::get_all_logs(&conn.log_collector).await)
}
```

Replace `clear_communication_logs` (lines 550-561):
```rust
#[tauri::command]
pub async fn clear_communication_logs(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;
    log_helpers::clear_logs(&conn.log_collector).await;
    Ok(())
}
```

Replace `export_logs_csv` (lines 563-573):
```rust
#[tauri::command]
pub async fn export_logs_csv(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<String, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;
    Ok(log_helpers::export_csv(&conn.log_collector).await)
}
```

- [ ] **Step 3: Build to verify compilation**

Run: `cargo build -p modbussim-app`
Expected: Successful compilation with no errors

- [ ] **Step 4: Run all core tests to verify no regressions**

Run: `cargo test -p modbussim-core`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/modbussim-app/src/commands.rs
git commit -m "refactor(slave-app): use core parse and log_helpers, remove local duplicates"
```

---

## Task 4: Migrate modbusmaster-app to use core parse & log_helpers

**Files:**
- Modify: `crates/modbusmaster-app/src/commands.rs`

- [ ] **Step 1: Replace local parse/convert functions with core imports**

At the top of `crates/modbusmaster-app/src/commands.rs`, add:

```rust
use modbussim_core::parse::{parse_read_function, read_function_to_string};
```

Delete `function_to_string` (lines 25-31):
```rust
// DELETE THIS:
fn function_to_string(f: ReadFunction) -> String {
    match f {
        ReadFunction::ReadCoils => "read_coils".to_string(),
        ...
    }
}
```

Delete `string_to_function` (lines 34-42):
```rust
// DELETE THIS:
fn string_to_function(s: &str) -> Result<ReadFunction, String> {
    ...
}
```

Delete `parse_read_function` (lines 947-955):
```rust
// DELETE THIS (duplicate of string_to_function):
fn parse_read_function(s: &str) -> Result<ReadFunction, String> {
    ...
}
```

Then update all call sites:
- Replace `string_to_function(...)` with `parse_read_function(...)`
- Replace `function_to_string(f)` with `read_function_to_string(f).to_string()` where a `String` is needed, or `read_function_to_string(f)` where `&str` suffices

**Note:** `function_to_string` returned `String`, `read_function_to_string` returns `&'static str`. Most call sites accept `&str` via `.as_str()` or string formatting. Search for `function_to_string(` to find all call sites and verify each one compiles.

- [ ] **Step 2: Replace log commands with core log_helpers**

Add import:
```rust
use modbussim_core::log_helpers;
```

Replace `get_communication_logs` (lines 770-781):
```rust
#[tauri::command]
pub async fn get_communication_logs(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<LogEntry>, String> {
    let conns = state.master_connections.read().await;
    let conn_state = conns
        .get(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;
    Ok(log_helpers::get_all_logs(&conn_state.log_collector).await)
}
```

Replace `clear_communication_logs` (lines 783-795):
```rust
#[tauri::command]
pub async fn clear_communication_logs(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let conns = state.master_connections.read().await;
    let conn_state = conns
        .get(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;
    log_helpers::clear_logs(&conn_state.log_collector).await;
    Ok(())
}
```

Replace `export_logs_csv` (lines 797-808):
```rust
#[tauri::command]
pub async fn export_logs_csv(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<String, String> {
    let conns = state.master_connections.read().await;
    let conn_state = conns
        .get(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;
    Ok(log_helpers::export_csv(&conn_state.log_collector).await)
}
```

- [ ] **Step 3: Build to verify compilation**

Run: `cargo build -p modbusmaster-app`
Expected: Successful compilation with no errors

- [ ] **Step 4: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/modbusmaster-app/src/commands.rs
git commit -m "refactor(master-app): use core parse and log_helpers, remove local duplicates"
```

---

## Task 5: Create shared-frontend package with types and useDialog

**Files:**
- Create: `package.json` (root)
- Create: `shared-frontend/package.json`
- Create: `shared-frontend/tsconfig.json`
- Create: `shared-frontend/src/types/modbus.ts`
- Create: `shared-frontend/src/composables/useDialog.ts`
- Create: `shared-frontend/src/components/AppDialog.vue`
- Create: `shared-frontend/src/index.ts`

- [ ] **Step 1: Create root package.json for npm workspaces**

Create `package.json` in project root:

```json
{
  "private": true,
  "workspaces": [
    "shared-frontend",
    "frontend",
    "master-frontend"
  ]
}
```

- [ ] **Step 2: Create shared-frontend package**

Create `shared-frontend/package.json`:

```json
{
  "name": "shared-frontend",
  "private": true,
  "version": "0.0.0",
  "type": "module",
  "main": "src/index.ts",
  "dependencies": {
    "vue": "^3.5.30",
    "@tauri-apps/api": "^2.10.1"
  }
}
```

Create `shared-frontend/tsconfig.json`:

```json
{
  "extends": "@vue/tsconfig/tsconfig.dom.json",
  "compilerOptions": {
    "composite": true,
    "tsBuildInfoFile": "./node_modules/.tmp/tsconfig.tsbuildinfo",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "erasableSyntaxOnly": true
  },
  "include": ["src/**/*.ts", "src/**/*.vue"]
}
```

- [ ] **Step 3: Create shared types**

Create `shared-frontend/src/types/modbus.ts`:

```typescript
export interface LogEntry {
  timestamp: string
  direction: string
  function_code: string
  detail: string
}

export type DialogMode = 'alert' | 'confirm' | 'prompt'

export interface DialogState {
  visible: boolean
  mode: DialogMode
  title: string
  message: string
  defaultValue: string
  inputValue: string
}
```

- [ ] **Step 4: Create shared useDialog composable**

Create `shared-frontend/src/composables/useDialog.ts`:

```typescript
import { ref, readonly } from 'vue'
import type { DialogMode, DialogState } from '../types/modbus'

const state = ref<DialogState>({
  visible: false,
  mode: 'alert',
  title: '',
  message: '',
  defaultValue: '',
  inputValue: '',
})

let resolvePromise: ((value: string | boolean | null) => void) | null = null

function open(mode: DialogMode, message: string, defaultValue = ''): Promise<string | boolean | null | void> {
  return new Promise((resolve) => {
    resolvePromise = resolve as (value: string | boolean | null) => void
    state.value = {
      visible: true,
      mode,
      title: mode === 'alert' ? '提示' : mode === 'confirm' ? '确认' : '输入',
      message,
      defaultValue,
      inputValue: defaultValue,
    }
  })
}

export function showAlert(message: string): Promise<void> {
  return open('alert', message) as Promise<void>
}

export function showConfirm(message: string): Promise<boolean> {
  return open('confirm', message) as Promise<boolean>
}

export function showPrompt(message: string, defaultValue = ''): Promise<string | null> {
  return open('prompt', message, defaultValue) as Promise<string | null>
}

export function dialogConfirm(value?: string) {
  if (!resolvePromise) return
  const mode = state.value.mode
  state.value.visible = false
  if (mode === 'alert') resolvePromise(undefined as unknown as null)
  else if (mode === 'confirm') resolvePromise(true)
  else resolvePromise(value ?? state.value.inputValue)
  resolvePromise = null
}

export function dialogCancel() {
  if (!resolvePromise) return
  const mode = state.value.mode
  state.value.visible = false
  if (mode === 'alert') resolvePromise(undefined as unknown as null)
  else if (mode === 'confirm') resolvePromise(false)
  else resolvePromise(null)
  resolvePromise = null
}

export function useDialogState() {
  return { state: readonly(state), dialogConfirm, dialogCancel }
}

export const dialogKey = Symbol('dialog') as symbol
```

- [ ] **Step 5: Create shared AppDialog component**

Create `shared-frontend/src/components/AppDialog.vue` — copy the exact content from `frontend/src/components/AppDialog.vue` but change the import path:

```vue
<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import { useDialogState } from '../composables/useDialog'

const { state, dialogConfirm, dialogCancel } = useDialogState()
const inputRef = ref<HTMLInputElement | null>(null)
const inputValue = ref('')

watch(() => state.value.visible, async (visible) => {
  if (visible && state.value.mode === 'prompt') {
    inputValue.value = state.value.defaultValue
    await nextTick()
    inputRef.value?.focus()
    inputRef.value?.select()
  }
})

function handleConfirm() {
  if (state.value.mode === 'prompt') {
    dialogConfirm(inputValue.value)
  } else {
    dialogConfirm()
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') {
    handleConfirm()
  } else if (e.key === 'Escape') {
    dialogCancel()
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="state.visible" class="dialog-backdrop" @click.self="dialogCancel" @keydown="handleKeydown">
      <div class="dialog" role="dialog" aria-modal="true">
        <div class="dialog-header">
          <span class="dialog-title">{{ state.title }}</span>
        </div>
        <div class="dialog-body">
          <p class="dialog-message">{{ state.message }}</p>
          <input
            v-if="state.mode === 'prompt'"
            ref="inputRef"
            v-model="inputValue"
            class="dialog-input"
            type="text"
            @keydown.enter="handleConfirm"
            @keydown.escape="dialogCancel"
          />
        </div>
        <div class="dialog-footer">
          <button
            v-if="state.mode !== 'alert'"
            class="btn btn-secondary"
            @click="dialogCancel"
          >取消</button>
          <button
            class="btn btn-primary"
            @click="handleConfirm"
          >确定</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.dialog-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}

.dialog {
  background: #1e1e2e;
  border: 1px solid #45475a;
  border-radius: 8px;
  width: 360px;
  max-width: 90vw;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.dialog-header {
  padding: 16px 20px 0;
}

.dialog-title {
  font-size: 15px;
  font-weight: 600;
  color: #cdd6f4;
}

.dialog-body {
  padding: 12px 20px 16px;
}

.dialog-message {
  font-size: 13px;
  color: #bac2de;
  line-height: 1.5;
  margin: 0 0 8px;
  word-break: break-word;
}

.dialog-input {
  width: 100%;
  padding: 8px 12px;
  background: #11111b;
  border: 1px solid #45475a;
  border-radius: 6px;
  color: #cdd6f4;
  font-size: 14px;
  box-sizing: border-box;
  margin-top: 4px;
}

.dialog-input:focus {
  outline: none;
  border-color: #89b4fa;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 0 20px 16px;
}

.btn {
  padding: 7px 20px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
}

.btn-primary {
  background: #89b4fa;
  color: #1e1e2e;
}

.btn-primary:hover {
  background: #74c7ec;
}

.btn-secondary {
  background: #45475a;
  color: #cdd6f4;
}

.btn-secondary:hover {
  background: #585b70;
}
</style>
```

- [ ] **Step 6: Create barrel export**

Create `shared-frontend/src/index.ts`:

```typescript
// Types
export type { LogEntry, DialogMode, DialogState } from './types/modbus'

// Composables
export {
  showAlert,
  showConfirm,
  showPrompt,
  dialogConfirm,
  dialogCancel,
  useDialogState,
  dialogKey,
} from './composables/useDialog'

// Components
export { default as AppDialog } from './components/AppDialog.vue'
```

- [ ] **Step 7: Install workspace dependencies**

Run: `cd "/Users/daichangyu/Library/Mobile Documents/com~apple~CloudDocs/code/ModbusSim" && npm install`
Expected: Successful install, `node_modules` created at root level with workspace links

- [ ] **Step 8: Update frontend to use shared package**

Add shared-frontend dependency to `frontend/package.json`:

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2.10.1",
    "vue": "^3.5.30",
    "shared-frontend": "*"
  }
}
```

Add path reference to `frontend/tsconfig.app.json`:

```json
{
  "extends": "@vue/tsconfig/tsconfig.dom.json",
  "compilerOptions": {
    "tsBuildInfoFile": "./node_modules/.tmp/tsconfig.app.tsbuildinfo",
    "types": ["vite/client"],
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "erasableSyntaxOnly": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedSideEffectImports": true
  },
  "include": ["src/**/*.ts", "src/**/*.tsx", "src/**/*.vue"],
  "references": [
    { "path": "../shared-frontend/tsconfig.json" }
  ]
}
```

- [ ] **Step 9: Update master-frontend to use shared package**

Add shared-frontend dependency to `master-frontend/package.json`:

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2.10.1",
    "vue": "^3.5.30",
    "shared-frontend": "*"
  }
}
```

Add path reference to `master-frontend/tsconfig.app.json` (same pattern as frontend).

- [ ] **Step 10: Replace frontend useDialog and AppDialog with re-exports**

Replace `frontend/src/composables/useDialog.ts`:

```typescript
export {
  showAlert,
  showConfirm,
  showPrompt,
  dialogConfirm,
  dialogCancel,
  useDialogState,
  dialogKey,
} from 'shared-frontend'
export type { DialogMode, DialogState } from 'shared-frontend'
```

Replace `frontend/src/components/AppDialog.vue`:

```vue
<script setup lang="ts">
// Re-exported from shared-frontend for backwards compatibility.
// All components importing from this path continue to work unchanged.
</script>
<template>
  <AppDialog />
</template>
<script lang="ts">
import { AppDialog } from 'shared-frontend'
export default { components: { AppDialog } }
</script>
```

Actually, a simpler approach — just update the import in `frontend/src/App.vue` to import directly from shared-frontend:

Change in `frontend/src/App.vue`:
```typescript
// Before:
import AppDialog from './components/AppDialog.vue'
// After:
import { AppDialog } from 'shared-frontend'
```

Then delete `frontend/src/components/AppDialog.vue`.

- [ ] **Step 11: Replace master-frontend useDialog and AppDialog with re-exports**

Same changes as Step 10 but for master-frontend:

Change in `master-frontend/src/App.vue`:
```typescript
// Before:
import AppDialog from './components/AppDialog.vue'
// After:
import { AppDialog } from 'shared-frontend'
```

Then delete `master-frontend/src/components/AppDialog.vue`.

Update `master-frontend/src/composables/useDialog.ts` to re-export from shared (same content as Step 10's useDialog replacement).

- [ ] **Step 12: Run npm install and verify both frontends build**

```bash
cd "/Users/daichangyu/Library/Mobile Documents/com~apple~CloudDocs/code/ModbusSim"
npm install
cd frontend && npm run build
cd ../master-frontend && npm run build
```

Expected: Both frontends build successfully

- [ ] **Step 13: Commit**

```bash
git add package.json shared-frontend/ frontend/ master-frontend/
git commit -m "feat(shared-frontend): create shared package with types, useDialog, and AppDialog"
```

---

## Task 6: Extract useValueFormat composable to shared-frontend

**Files:**
- Create: `shared-frontend/src/composables/useValueFormat.ts`
- Modify: `shared-frontend/src/index.ts`
- Modify: `frontend/src/components/ValuePanel.vue`
- Modify: `master-frontend/src/components/ValuePanel.vue`

- [ ] **Step 1: Create useValueFormat composable**

Create `shared-frontend/src/composables/useValueFormat.ts`:

```typescript
import { computed, type Ref } from 'vue'

/**
 * Swap bytes within a 16-bit word: 0xAABB -> 0xBBAA
 */
export function swapBytes16(v: number): number {
  return ((v & 0xFF) << 8) | ((v >> 8) & 0xFF)
}

/**
 * Convert two 16-bit registers to a Float32 string.
 */
export function toFloat32(hi: number, lo: number): string {
  const buf = new ArrayBuffer(4)
  const view = new DataView(buf)
  view.setUint16(0, hi)
  view.setUint16(2, lo)
  return view.getFloat32(0).toPrecision(7)
}

/**
 * 16-bit value formatting composable.
 * @param rawValue - A ref to the raw 16-bit register value (number).
 */
export function use16BitFormat(rawValue: Ref<number>) {
  const signed16 = computed(() => {
    const v = rawValue.value & 0xFFFF
    return v >= 0x8000 ? v - 0x10000 : v
  })

  const unsigned16 = computed(() => {
    return rawValue.value & 0xFFFF
  })

  const hex16 = computed(() => {
    return '0x' + (rawValue.value & 0xFFFF).toString(16).toUpperCase().padStart(4, '0')
  })

  const binary16 = computed(() => {
    const b = (rawValue.value & 0xFFFF).toString(2).padStart(16, '0')
    return `${b.slice(0, 4)} ${b.slice(4, 8)} ${b.slice(8, 12)} ${b.slice(12, 16)}`
  })

  return { signed16, unsigned16, hex16, binary16 }
}

/**
 * 32-bit value formatting composable.
 * @param hi - Ref to the high 16-bit register value.
 * @param lo - Ref to the low 16-bit register value.
 * @param enabled - Ref<boolean> indicating whether 32-bit display is active.
 */
export function use32BitFormat(hi: Ref<number>, lo: Ref<number>, enabled: Ref<boolean>) {
  const longABCD = computed(() => {
    if (!enabled.value) return '-'
    return (((hi.value << 16) | lo.value) >>> 0).toString()
  })

  const longCDAB = computed(() => {
    if (!enabled.value) return '-'
    return (((lo.value << 16) | hi.value) >>> 0).toString()
  })

  const longBADC = computed(() => {
    if (!enabled.value) return '-'
    return (((swapBytes16(hi.value) << 16) | swapBytes16(lo.value)) >>> 0).toString()
  })

  const longDCBA = computed(() => {
    if (!enabled.value) return '-'
    return (((swapBytes16(lo.value) << 16) | swapBytes16(hi.value)) >>> 0).toString()
  })

  const floatABCD = computed(() => {
    if (!enabled.value) return '-'
    return toFloat32(hi.value, lo.value)
  })

  const floatCDAB = computed(() => {
    if (!enabled.value) return '-'
    return toFloat32(lo.value, hi.value)
  })

  const floatBADC = computed(() => {
    if (!enabled.value) return '-'
    return toFloat32(swapBytes16(hi.value), swapBytes16(lo.value))
  })

  const floatDCBA = computed(() => {
    if (!enabled.value) return '-'
    return toFloat32(swapBytes16(lo.value), swapBytes16(hi.value))
  })

  return { longABCD, longCDAB, longBADC, longDCBA, floatABCD, floatCDAB, floatBADC, floatDCBA }
}

/**
 * 64-bit (Float64) value formatting composable.
 * @param values - Ref to array of 4 raw 16-bit register values [r0, r1, r2, r3].
 * @param enabled - Ref<boolean> indicating whether 64-bit display is active.
 */
export function use64BitFormat(values: Ref<number[]>, enabled: Ref<boolean>) {
  function makeDouble(reorder: (i: number) => number, byteSwap: boolean): string {
    const buf = new ArrayBuffer(8)
    const view = new DataView(buf)
    for (let i = 0; i < 4; i++) {
      const v = values.value[reorder(i)] & 0xFFFF
      view.setUint16(i * 2, byteSwap ? swapBytes16(v) : v)
    }
    return view.getFloat64(0).toPrecision(15)
  }

  const doubleValue = computed(() => {
    if (!enabled.value) return '-'
    return makeDouble(i => i, false)
  })

  const doubleReversed = computed(() => {
    if (!enabled.value) return '-'
    return makeDouble(i => 3 - i, false)
  })

  const doubleByteSwap = computed(() => {
    if (!enabled.value) return '-'
    return makeDouble(i => i, true)
  })

  const doubleLittleEndian = computed(() => {
    if (!enabled.value) return '-'
    return makeDouble(i => 3 - i, true)
  })

  return { doubleValue, doubleReversed, doubleByteSwap, doubleLittleEndian }
}
```

- [ ] **Step 2: Export from index.ts**

Add to `shared-frontend/src/index.ts`:

```typescript
// Value formatting
export {
  swapBytes16,
  toFloat32,
  use16BitFormat,
  use32BitFormat,
  use64BitFormat,
} from './composables/useValueFormat'
```

- [ ] **Step 3: Refactor frontend ValuePanel.vue to use shared composable**

In `frontend/src/components/ValuePanel.vue`, replace the inline 16/32/64-bit computed properties (lines ~77-209) with:

```typescript
import { use16BitFormat, use32BitFormat, use64BitFormat, swapBytes16, toFloat32 } from 'shared-frontend'

// ... existing code for firstReg, sortedRegs, etc. ...

const rawValue = computed(() => firstReg.value ? firstReg.value.value & 0xFFFF : 0)
const { signed16, unsigned16, hex16, binary16 } = use16BitFormat(computed(() => rawValue.value))

const reg32Hi = computed(() => show32bit.value ? sortedRegs.value[0].value & 0xFFFF : 0)
const reg32Lo = computed(() => show32bit.value ? sortedRegs.value[1].value & 0xFFFF : 0)
const { longABCD, longCDAB, longBADC, longDCBA, floatABCD, floatCDAB, floatBADC, floatDCBA } =
  use32BitFormat(reg32Hi, reg32Lo, show32bit)

const reg64Values = computed(() => {
  if (!show64bit.value) return [0, 0, 0, 0]
  return sortedRegs.value.slice(0, 4).map(r => r.value & 0xFFFF)
})
const { doubleValue, doubleReversed, doubleByteSwap, doubleLittleEndian } =
  use64BitFormat(reg64Values, show64bit)
```

Remove the following inline code that was replaced:
- `signed16`, `unsigned16`, `hex16`, `binary16` computed properties
- `swapBytes16` function
- `reg32Values` computed property
- `longABCD`, `longCDAB`, `longBADC`, `longDCBA` computed properties
- `toFloat32` function
- `floatABCD`, `floatCDAB`, `floatBADC`, `floatDCBA` computed properties
- `doubleValue`, `doubleReversed`, `doubleByteSwap`, `doubleLittleEndian` computed properties

Keep `show32bit`, `show64bit`, and all editing/writing logic unchanged.

- [ ] **Step 4: Refactor master-frontend ValuePanel.vue similarly**

In `master-frontend/src/components/ValuePanel.vue`, apply the same pattern but using `raw_value` instead of `value`:

```typescript
import { use16BitFormat, use32BitFormat, use64BitFormat, swapBytes16, toFloat32 } from 'shared-frontend'

const rawValue = computed(() => firstReg.value ? Number(firstReg.value.raw_value) & 0xFFFF : 0)
const { signed16, unsigned16, hex16, binary16 } = use16BitFormat(computed(() => rawValue.value))

const reg32Hi = computed(() => show32bit.value ? Number(sortedRegs.value[0].raw_value) & 0xFFFF : 0)
const reg32Lo = computed(() => show32bit.value ? Number(sortedRegs.value[1].raw_value) & 0xFFFF : 0)
const { longABCD, longCDAB, longBADC, longDCBA, floatABCD, floatCDAB, floatBADC, floatDCBA } =
  use32BitFormat(reg32Hi, reg32Lo, show32bit)

const reg64Values = computed(() => {
  if (!show64bit.value) return [0, 0, 0, 0]
  return sortedRegs.value.slice(0, 4).map(r => Number(r.raw_value) & 0xFFFF)
})
const { doubleValue, doubleReversed, doubleByteSwap, doubleLittleEndian } =
  use64BitFormat(reg64Values, show64bit)
```

Remove the same set of inline code as in Step 3.

- [ ] **Step 5: Build both frontends**

```bash
cd frontend && npm run build && cd ../master-frontend && npm run build
```

Expected: Both build successfully

- [ ] **Step 6: Commit**

```bash
git add shared-frontend/src/composables/useValueFormat.ts shared-frontend/src/index.ts \
        frontend/src/components/ValuePanel.vue master-frontend/src/components/ValuePanel.vue
git commit -m "refactor(frontend): extract value formatting to shared useValueFormat composable"
```

---

## Task 7: Extract useLogPanel composable to shared-frontend

**Files:**
- Create: `shared-frontend/src/composables/useLogPanel.ts`
- Modify: `shared-frontend/src/index.ts`
- Modify: `frontend/src/components/LogPanel.vue`
- Modify: `master-frontend/src/components/LogPanel.vue`

- [ ] **Step 1: Create useLogPanel composable**

Create `shared-frontend/src/composables/useLogPanel.ts`:

```typescript
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { LogEntry } from '../types/modbus'

/**
 * Shared log panel logic for both slave and master frontends.
 */
export function useLogPanel() {
  const logs = ref<LogEntry[]>([])
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  async function loadLogs(connectionId: string) {
    if (!connectionId) return
    isLoading.value = true
    try {
      logs.value = await invoke<LogEntry[]>('get_communication_logs', {
        connectionId,
      })
    } catch (e) {
      error.value = String(e)
    }
    isLoading.value = false
  }

  async function clearLogs(connectionId: string) {
    if (!connectionId) return
    try {
      await invoke('clear_communication_logs', {
        connectionId,
      })
      logs.value = []
    } catch (e) {
      error.value = String(e)
    }
  }

  async function exportLogsCsv(connectionId: string, filenamePrefix = 'modbus_log') {
    if (!connectionId) return
    try {
      const csv = await invoke<string>('export_logs_csv', {
        connectionId,
      })
      const blob = new Blob([csv], { type: 'text/csv' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `${filenamePrefix}_${Date.now()}.csv`
      a.click()
      URL.revokeObjectURL(url)
    } catch (e) {
      error.value = String(e)
    }
  }

  return { logs, isLoading, error, loadLogs, clearLogs, exportLogsCsv }
}
```

- [ ] **Step 2: Export from index.ts**

Add to `shared-frontend/src/index.ts`:

```typescript
// Log panel
export { useLogPanel } from './composables/useLogPanel'
```

- [ ] **Step 3: Refactor frontend LogPanel.vue**

In `frontend/src/components/LogPanel.vue`, replace the inline `logs`, `isLoading`, `error`, `loadLogs`, `clearLogs`, `exportLogs` with:

```typescript
import { useLogPanel } from 'shared-frontend'

const { logs, isLoading, error, loadLogs, clearLogs, exportLogsCsv } = useLogPanel()

// Wrap calls to pass selectedConnId:
async function doLoadLogs() {
  await loadLogs(selectedConnId.value)
}

async function doClearLogs() {
  await clearLogs(selectedConnId.value)
}

async function doExportLogs() {
  await exportLogsCsv(selectedConnId.value, 'modbus_slave_log')
}
```

Remove the old inline `loadLogs`, `clearLogs`, `exportLogs` functions. Update the template to call `doLoadLogs`, `doClearLogs`, `doExportLogs`.

Keep `loadConnections`, `formatTimestamp`, `startAutoRefresh`, `stopAutoRefresh` as-is (these differ between the two apps).

- [ ] **Step 4: Refactor master-frontend LogPanel.vue**

Same pattern:

```typescript
import { useLogPanel } from 'shared-frontend'

const { logs, isLoading, error, loadLogs, clearLogs, exportLogsCsv } = useLogPanel()

async function doLoadLogs() {
  await loadLogs(selectedConnId.value)
}

async function doClearLogs() {
  await clearLogs(selectedConnId.value)
}

async function doExportLogs() {
  await exportLogsCsv(selectedConnId.value, 'modbus_master_log')
}
```

Remove old inline functions. Keep the master-specific `formatTimestamp` (with milliseconds), `formatDirection`, `fcNameMap`, `formatFunctionCode`.

- [ ] **Step 5: Build both frontends**

```bash
cd frontend && npm run build && cd ../master-frontend && npm run build
```

Expected: Both build successfully

- [ ] **Step 6: Commit**

```bash
git add shared-frontend/src/composables/useLogPanel.ts shared-frontend/src/index.ts \
        frontend/src/components/LogPanel.vue master-frontend/src/components/LogPanel.vue
git commit -m "refactor(frontend): extract log panel logic to shared useLogPanel composable"
```

---

## Task 8: Add project file persistence to core

**Files:**
- Create: `crates/modbussim-core/src/project.rs`
- Modify: `crates/modbussim-core/src/lib.rs`
- Test: `crates/modbussim-core/src/project.rs` (inline tests)

- [ ] **Step 1: Write failing tests for project file module**

```rust
// crates/modbussim-core/src/project.rs

use crate::register::{RegisterType, DataType, Endian, RegisterDef, RegisterMap};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Project type: slave or master.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectType {
    Slave,
    Master,
}

/// Transport configuration (Phase 2 will extend with RTU/ASCII/RtuOverTcp).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransportConfig {
    Tcp { host: String, port: u16 },
}

/// A register block definition in a project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterBlockConfig {
    pub address: u16,
    pub count: u16,
    #[serde(default)]
    pub data_type: Option<String>,
    #[serde(default)]
    pub endian: Option<String>,
    #[serde(default)]
    pub values: Vec<serde_json::Value>,
    #[serde(default)]
    pub names: HashMap<String, String>,
}

/// A slave device definition in a project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub slave_id: u8,
    #[serde(default)]
    pub registers: RegistersConfig,
}

/// Register configuration grouped by type.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistersConfig {
    #[serde(default)]
    pub coils: Vec<RegisterBlockConfig>,
    #[serde(default)]
    pub discrete_inputs: Vec<RegisterBlockConfig>,
    #[serde(default)]
    pub holding: Vec<RegisterBlockConfig>,
    #[serde(default)]
    pub input: Vec<RegisterBlockConfig>,
}

/// A scan group definition (master project only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanGroupConfig {
    pub name: String,
    pub slave_id: u8,
    pub function_code: u8,
    pub start_address: u16,
    pub count: u16,
    pub interval_ms: u64,
}

/// A connection definition in a project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    pub transport: TransportConfig,
    #[serde(default)]
    pub devices: Vec<DeviceConfig>,
    #[serde(default)]
    pub scan_groups: Vec<ScanGroupConfig>,
}

/// The top-level project file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub version: u32,
    #[serde(rename = "type")]
    pub project_type: ProjectType,
    pub connections: Vec<ConnectionConfig>,
}

impl ProjectFile {
    pub fn new_slave() -> Self {
        Self {
            version: 1,
            project_type: ProjectType::Slave,
            connections: Vec::new(),
        }
    }

    pub fn new_master() -> Self {
        Self {
            version: 1,
            project_type: ProjectType::Master,
            connections: Vec::new(),
        }
    }
}

/// Save a project file to the given path as pretty-printed JSON.
pub fn save_project(project: &ProjectFile, path: &Path) -> Result<(), String> {
    todo!()
}

/// Load a project file from the given path.
pub fn load_project(path: &Path) -> Result<ProjectFile, String> {
    todo!()
}

/// Migrate an older project file format to the current version.
pub fn migrate_project(data: &str) -> Result<ProjectFile, String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_new_slave_project() {
        let proj = ProjectFile::new_slave();
        assert_eq!(proj.version, 1);
        assert_eq!(proj.project_type, ProjectType::Slave);
        assert!(proj.connections.is_empty());
    }

    #[test]
    fn test_new_master_project() {
        let proj = ProjectFile::new_master();
        assert_eq!(proj.version, 1);
        assert_eq!(proj.project_type, ProjectType::Master);
        assert!(proj.connections.is_empty());
    }

    #[test]
    fn test_save_and_load_slave_project() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.modbusproj");

        let mut proj = ProjectFile::new_slave();
        proj.connections.push(ConnectionConfig {
            id: "conn-1".to_string(),
            name: "Test Connection".to_string(),
            transport: TransportConfig::Tcp {
                host: "0.0.0.0".to_string(),
                port: 502,
            },
            devices: vec![DeviceConfig {
                slave_id: 1,
                registers: RegistersConfig {
                    holding: vec![RegisterBlockConfig {
                        address: 0,
                        count: 5,
                        data_type: Some("uint16".to_string()),
                        endian: Some("big".to_string()),
                        values: vec![
                            serde_json::json!(100),
                            serde_json::json!(200),
                            serde_json::json!(300),
                            serde_json::json!(0),
                            serde_json::json!(0),
                        ],
                        names: {
                            let mut m = HashMap::new();
                            m.insert("0".to_string(), "Temperature".to_string());
                            m
                        },
                    }],
                    ..Default::default()
                },
            }],
            scan_groups: vec![],
        });

        save_project(&proj, &path).unwrap();
        assert!(path.exists());

        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.project_type, ProjectType::Slave);
        assert_eq!(loaded.connections.len(), 1);
        assert_eq!(loaded.connections[0].name, "Test Connection");
        assert_eq!(loaded.connections[0].devices.len(), 1);
        assert_eq!(loaded.connections[0].devices[0].slave_id, 1);
        assert_eq!(loaded.connections[0].devices[0].registers.holding[0].count, 5);
    }

    #[test]
    fn test_save_and_load_master_project() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.modbusproj");

        let mut proj = ProjectFile::new_master();
        proj.connections.push(ConnectionConfig {
            id: "conn-1".to_string(),
            name: "Master Connection".to_string(),
            transport: TransportConfig::Tcp {
                host: "192.168.1.100".to_string(),
                port: 502,
            },
            devices: vec![],
            scan_groups: vec![ScanGroupConfig {
                name: "Temperature".to_string(),
                slave_id: 1,
                function_code: 3,
                start_address: 0,
                count: 10,
                interval_ms: 1000,
            }],
        });

        save_project(&proj, &path).unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.project_type, ProjectType::Master);
        assert_eq!(loaded.connections[0].scan_groups.len(), 1);
        assert_eq!(loaded.connections[0].scan_groups[0].name, "Temperature");
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = load_project(Path::new("/nonexistent/file.modbusproj"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.modbusproj");
        fs::write(&path, "not json").unwrap();
        let result = load_project(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_migrate_current_version() {
        let json = r#"{"version":1,"type":"slave","connections":[]}"#;
        let proj = migrate_project(json).unwrap();
        assert_eq!(proj.version, 1);
    }

    #[test]
    fn test_migrate_unknown_version() {
        let json = r#"{"version":99,"type":"slave","connections":[]}"#;
        let result = migrate_project(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_roundtrip_preserves_transport_tag() {
        let proj = ProjectFile {
            version: 1,
            project_type: ProjectType::Slave,
            connections: vec![ConnectionConfig {
                id: "c1".to_string(),
                name: "test".to_string(),
                transport: TransportConfig::Tcp {
                    host: "0.0.0.0".to_string(),
                    port: 502,
                },
                devices: vec![],
                scan_groups: vec![],
            }],
        };
        let json = serde_json::to_string(&proj).unwrap();
        assert!(json.contains(r#""type":"tcp"#));
        let loaded: ProjectFile = serde_json::from_str(&json).unwrap();
        match &loaded.connections[0].transport {
            TransportConfig::Tcp { host, port } => {
                assert_eq!(host, "0.0.0.0");
                assert_eq!(*port, 502);
            }
        }
    }
}
```

- [ ] **Step 2: Add tempfile dev dependency and register module**

Add to `crates/modbussim-core/Cargo.toml` under `[dev-dependencies]`:

```toml
[dev-dependencies]
tokio = { version = "1", features = ["full", "test-util"] }
tempfile = "3"
```

Add `pub mod project;` to `crates/modbussim-core/src/lib.rs`:

```rust
pub mod register;
pub mod slave;
pub mod master;
pub mod log_entry;
pub mod log_collector;
pub mod config;
pub mod tools;
pub mod parse;
pub mod log_helpers;
pub mod project;
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd crates/modbussim-core && cargo test project`
Expected: FAIL with "not yet implemented"

- [ ] **Step 4: Implement save/load/migrate**

Replace the `todo!()` bodies in `crates/modbussim-core/src/project.rs`:

```rust
/// Save a project file to the given path as pretty-printed JSON.
pub fn save_project(project: &ProjectFile, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(project)
        .map_err(|e| format!("failed to serialize project: {}", e))?;
    std::fs::write(path, json)
        .map_err(|e| format!("failed to write project file: {}", e))
}

/// Load a project file from the given path.
pub fn load_project(path: &Path) -> Result<ProjectFile, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read project file: {}", e))?;
    migrate_project(&data)
}

/// Migrate an older project file format to the current version.
/// Currently only version 1 is supported.
pub fn migrate_project(data: &str) -> Result<ProjectFile, String> {
    let value: serde_json::Value = serde_json::from_str(data)
        .map_err(|e| format!("invalid JSON: {}", e))?;

    let version = value.get("version")
        .and_then(|v| v.as_u64())
        .ok_or("missing or invalid version field")?;

    match version {
        1 => {
            serde_json::from_value(value)
                .map_err(|e| format!("failed to parse project v1: {}", e))
        }
        v => Err(format!("unsupported project version: {}", v)),
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd crates/modbussim-core && cargo test project`
Expected: All 9 tests pass

- [ ] **Step 6: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/modbussim-core/src/project.rs crates/modbussim-core/src/lib.rs crates/modbussim-core/Cargo.toml
git commit -m "feat(core): add project file persistence (save/load/migrate .modbusproj)"
```

---

## Task 9: Add project file Tauri commands to slave app

**Files:**
- Modify: `crates/modbussim-app/src/commands.rs`
- Modify: `crates/modbussim-app/src/main.rs` (register new commands)

- [ ] **Step 1: Add project save/load commands**

Add to `crates/modbussim-app/src/commands.rs`:

```rust
use modbussim_core::project::{self, ProjectFile, ProjectType};

#[tauri::command]
pub async fn save_project_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let connections = state.slave_connections.read().await;
    let mut proj = ProjectFile::new_slave();

    for (id, conn_state) in connections.iter() {
        let conn = &conn_state.connection;
        let conn_config = project::ConnectionConfig {
            id: id.clone(),
            name: format!("{}:{}", conn.bind_address(), conn.port()),
            transport: project::TransportConfig::Tcp {
                host: conn.bind_address().to_string(),
                port: conn.port(),
            },
            devices: conn.devices().iter().map(|dev| {
                project::DeviceConfig {
                    slave_id: dev.slave_id,
                    registers: project::RegistersConfig::default(),
                }
            }).collect(),
            scan_groups: vec![],
        };
        proj.connections.push(conn_config);
    }

    project::save_project(&proj, std::path::Path::new(&path))
}

#[tauri::command]
pub async fn load_project_file(path: String) -> Result<ProjectFile, String> {
    project::load_project(std::path::Path::new(&path))
}
```

- [ ] **Step 2: Register commands in main.rs**

In `crates/modbussim-app/src/main.rs`, add `save_project_file` and `load_project_file` to the `invoke_handler` macro:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    commands::save_project_file,
    commands::load_project_file,
])
```

- [ ] **Step 3: Build to verify compilation**

Run: `cargo build -p modbussim-app`
Expected: Successful compilation

- [ ] **Step 4: Commit**

```bash
git add crates/modbussim-app/src/commands.rs crates/modbussim-app/src/main.rs
git commit -m "feat(slave-app): add save/load project file Tauri commands"
```

---

## Task 10: Add project file Tauri commands to master app

**Files:**
- Modify: `crates/modbusmaster-app/src/commands.rs`
- Modify: `crates/modbusmaster-app/src/main.rs` (register new commands)

- [ ] **Step 1: Add project save/load commands**

Add to `crates/modbusmaster-app/src/commands.rs`:

```rust
use modbussim_core::project::{self, ProjectFile, ProjectType};

#[tauri::command]
pub async fn save_project_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let conns = state.master_connections.read().await;
    let mut proj = ProjectFile::new_master();

    for (id, conn_state) in conns.iter() {
        let conn = &conn_state.connection;
        let conn_config = project::ConnectionConfig {
            id: id.clone(),
            name: format!("{}:{}", conn.config().target_address, conn.config().port),
            transport: project::TransportConfig::Tcp {
                host: conn.config().target_address.clone(),
                port: conn.config().port,
            },
            devices: vec![],
            scan_groups: conn_state.scan_groups.iter().map(|sg| {
                project::ScanGroupConfig {
                    name: sg.name.clone(),
                    slave_id: sg.slave_id.unwrap_or(conn.config().slave_id),
                    function_code: sg.function as u8,
                    start_address: sg.start_address,
                    count: sg.quantity,
                    interval_ms: sg.interval_ms,
                }
            }).collect(),
        };
        proj.connections.push(conn_config);
    }

    project::save_project(&proj, std::path::Path::new(&path))
}

#[tauri::command]
pub async fn load_project_file(path: String) -> Result<ProjectFile, String> {
    project::load_project(std::path::Path::new(&path))
}
```

- [ ] **Step 2: Register commands in main.rs**

Add `save_project_file` and `load_project_file` to the master app's `invoke_handler`.

- [ ] **Step 3: Build to verify compilation**

Run: `cargo build -p modbusmaster-app`
Expected: Successful compilation

- [ ] **Step 4: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/modbusmaster-app/src/commands.rs crates/modbusmaster-app/src/main.rs
git commit -m "feat(master-app): add save/load project file Tauri commands"
```

---

## Task 11: Add project file UI to both frontends

**Files:**
- Modify: `frontend/src/components/Toolbar.vue`
- Modify: `master-frontend/src/components/Toolbar.vue`

- [ ] **Step 1: Add project file buttons to slave Toolbar**

In `frontend/src/components/Toolbar.vue`, add project file management functions:

```typescript
import { invoke } from '@tauri-apps/api/core'
import { save, open } from '@tauri-apps/plugin-dialog'

const currentProjectPath = ref<string | null>(null)
const hasUnsavedChanges = ref(false)

async function newProject() {
  if (hasUnsavedChanges.value) {
    const confirmed = await showConfirm('有未保存的更改，确定新建项目？')
    if (!confirmed) return
  }
  currentProjectPath.value = null
  hasUnsavedChanges.value = false
  // Reset all connections (existing logic or emit event)
}

async function openProject() {
  const path = await open({
    filters: [{ name: 'Modbus Project', extensions: ['modbusproj'] }],
  })
  if (!path) return
  try {
    const project = await invoke('load_project_file', { path })
    currentProjectPath.value = path as string
    hasUnsavedChanges.value = false
    // Apply project data to app state (emit event to App.vue)
  } catch (e) {
    await showAlert(String(e))
  }
}

async function saveProject() {
  if (!currentProjectPath.value) {
    return saveProjectAs()
  }
  try {
    await invoke('save_project_file', { path: currentProjectPath.value })
    hasUnsavedChanges.value = false
  } catch (e) {
    await showAlert(String(e))
  }
}

async function saveProjectAs() {
  const path = await save({
    filters: [{ name: 'Modbus Project', extensions: ['modbusproj'] }],
    defaultPath: 'untitled.modbusproj',
  })
  if (!path) return
  try {
    await invoke('save_project_file', { path })
    currentProjectPath.value = path
    hasUnsavedChanges.value = false
  } catch (e) {
    await showAlert(String(e))
  }
}
```

Add buttons to the toolbar template:

```html
<button class="tool-btn" @click="newProject" title="新建项目">📄</button>
<button class="tool-btn" @click="openProject" title="打开项目">📂</button>
<button class="tool-btn" @click="saveProject" title="保存项目 (Ctrl+S)">💾</button>
```

**Note:** This requires the `@tauri-apps/plugin-dialog` package. Add it:

```bash
cd frontend && npm install @tauri-apps/plugin-dialog
```

And register the plugin in the Tauri app configuration (`tauri.conf.json` or `main.rs`).

- [ ] **Step 2: Add same project file buttons to master Toolbar**

Apply the same pattern to `master-frontend/src/components/Toolbar.vue`. The invoke commands are identical (`save_project_file`, `load_project_file`).

```bash
cd master-frontend && npm install @tauri-apps/plugin-dialog
```

- [ ] **Step 3: Add Ctrl+S keyboard shortcut**

In both `App.vue` files, add a global keydown listener:

```typescript
import { onMounted, onUnmounted } from 'vue'

function handleKeydown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key === 's') {
    e.preventDefault()
    // Emit save event or call save directly
  }
}

onMounted(() => document.addEventListener('keydown', handleKeydown))
onUnmounted(() => document.removeEventListener('keydown', handleKeydown))
```

- [ ] **Step 4: Build both apps**

```bash
cargo build --workspace
cd frontend && npm run build
cd ../master-frontend && npm run build
```

Expected: All compile successfully

- [ ] **Step 5: Commit**

```bash
git add frontend/ master-frontend/ crates/
git commit -m "feat(frontend): add project file UI (new/open/save/save-as) to both apps"
```

---

## Task 12: Final integration verification

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test --workspace`
Expected: All tests pass

- [ ] **Step 2: Build both Tauri apps**

Run: `cargo build --workspace`
Expected: Clean build with no warnings related to our changes

- [ ] **Step 3: Build both frontends**

```bash
cd frontend && npm run build && cd ../master-frontend && npm run build
```

Expected: Both build successfully

- [ ] **Step 4: Verify shared-frontend is properly linked**

```bash
ls -la frontend/node_modules/shared-frontend
ls -la master-frontend/node_modules/shared-frontend
```

Expected: Both point to `../../shared-frontend` (workspace symlinks)

- [ ] **Step 5: Commit any remaining cleanup**

```bash
git status
# If there are any uncommitted changes:
git add -A
git commit -m "chore: Phase 1 final cleanup and integration verification"
```
