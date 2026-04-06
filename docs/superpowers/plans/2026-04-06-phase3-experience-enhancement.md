# Phase 3: Experience Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enhance user experience with unified error handling, auto-reconnect, virtual scrolling for large datasets, log filtering/search, and dynamic data generation for slave registers.

**Architecture:** Five independent improvements: (1) Unified `ModbusError` enum replacing scattered `String` errors, with serde serialization for structured frontend handling. (2) Reconnect state machine in `MasterConnection` with exponential backoff and Tauri event notifications. (3) `@tanstack/vue-virtual` for RegisterTable and DataTable. (4) Log filtering composable with direction/FC/text search. (5) `DataSource` enum in core with background timer for slave register simulation.

**Tech Stack:** Rust (thiserror, serde), Vue 3 (`@tanstack/vue-virtual`), TypeScript

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `crates/modbussim-core/src/error.rs` | Unified `ModbusError` enum with serde support |
| `crates/modbussim-core/src/reconnect.rs` | `ReconnectPolicy` and reconnect state machine |
| `crates/modbussim-core/src/data_source.rs` | `DataSource` enum and `DataSourceRunner` for register simulation |
| `shared-frontend/src/composables/useErrorHandler.ts` | Structured error classification and toast display |
| `shared-frontend/src/composables/useLogFilter.ts` | Log search/filter composable |

### Modified Files

| File | Changes |
|------|---------|
| `crates/modbussim-core/src/lib.rs` | Add new module declarations |
| `crates/modbussim-core/src/master.rs` | Add `MasterState::Reconnecting`, reconnect loop, `ReconnectPolicy` field |
| `crates/modbussim-core/src/slave.rs` | Add `DataSourceRunner` integration |
| `crates/modbussim-app/src/commands.rs` | Data source Tauri commands |
| `crates/modbusmaster-app/src/commands.rs` | Reconnect control commands, cancel_reconnect |
| `frontend/src/components/RegisterTable.vue` | Virtual scrolling integration |
| `frontend/src/components/LogPanel.vue` | Search/filter UI |
| `master-frontend/src/components/DataTable.vue` | Virtual scrolling integration |
| `master-frontend/src/components/LogPanel.vue` | Search/filter UI |
| `master-frontend/src/components/ConnectionTree.vue` | Reconnect status indicators |
| `shared-frontend/src/index.ts` | Export new composables |

---

## Task 1: Unified ModbusError Enum

**Files:**
- Create: `crates/modbussim-core/src/error.rs`
- Modify: `crates/modbussim-core/src/lib.rs`

- [ ] **Step 1: Create error.rs with ModbusError and tests**

```rust
// crates/modbussim-core/src/error.rs

use serde::Serialize;
use thiserror::Error;

/// Unified error type for the ModbusSim system.
/// Serializable so frontends can parse structured error data.
#[derive(Debug, Error, Serialize)]
#[serde(tag = "category", rename_all = "snake_case")]
pub enum ModbusError {
    // Connection layer
    #[error("connection refused: {addr}")]
    ConnectionRefused { addr: String },
    #[error("connection timeout: {addr} ({timeout_ms}ms)")]
    ConnectionTimeout { addr: String, timeout_ms: u64 },
    #[error("connection lost: {reason}")]
    ConnectionLost { reason: String },
    #[error("serial port busy: {port}")]
    SerialPortBusy { port: String },
    #[error("serial port not found: {port}")]
    SerialPortNotFound { port: String },
    #[error("serial port permission denied: {port}")]
    SerialPortPermissionDenied { port: String },

    // Protocol layer
    #[error("illegal function: FC{fc:02X}")]
    IllegalFunction { fc: u8 },
    #[error("illegal data address: {addr} count={count}")]
    IllegalDataAddress { addr: u16, count: u16 },
    #[error("illegal data value: {detail}")]
    IllegalDataValue { detail: String },
    #[error("slave device failure: slave {slave_id}")]
    SlaveDeviceFailure { slave_id: u8 },
    #[error("response timeout: slave {slave_id} FC{fc:02X}")]
    ResponseTimeout { slave_id: u8, fc: u8 },
    #[error("CRC mismatch: expected {expected:#06X}, got {actual:#06X}")]
    CrcMismatch { expected: u16, actual: u16 },
    #[error("LRC mismatch: expected {expected:#04X}, got {actual:#04X}")]
    LrcMismatch { expected: u8, actual: u8 },
    #[error("frame error: {detail}")]
    FrameError { detail: String },

    // Application layer
    #[error("slave ID conflict: {id}")]
    SlaveIdConflict { id: u8 },
    #[error("project file corrupt: {path}")]
    ProjectFileCorrupt { path: String },
    #[error("unsupported project version: {version}")]
    ProjectVersionUnsupported { version: u32 },

    // Generic
    #[error("I/O error: {0}")]
    Io(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl ModbusError {
    /// Returns the error category for frontend classification.
    pub fn category(&self) -> &'static str {
        match self {
            Self::ConnectionRefused { .. }
            | Self::ConnectionTimeout { .. }
            | Self::ConnectionLost { .. }
            | Self::SerialPortBusy { .. }
            | Self::SerialPortNotFound { .. }
            | Self::SerialPortPermissionDenied { .. } => "connection",

            Self::IllegalFunction { .. }
            | Self::IllegalDataAddress { .. }
            | Self::IllegalDataValue { .. }
            | Self::SlaveDeviceFailure { .. }
            | Self::ResponseTimeout { .. }
            | Self::CrcMismatch { .. }
            | Self::LrcMismatch { .. }
            | Self::FrameError { .. } => "protocol",

            Self::SlaveIdConflict { .. }
            | Self::ProjectFileCorrupt { .. }
            | Self::ProjectVersionUnsupported { .. } => "application",

            Self::Io(_) | Self::Internal(_) => "generic",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ModbusError::ConnectionTimeout { addr: "127.0.0.1:502".into(), timeout_ms: 3000 };
        assert!(err.to_string().contains("127.0.0.1:502"));
        assert!(err.to_string().contains("3000"));
    }

    #[test]
    fn test_error_category() {
        assert_eq!(ModbusError::ConnectionRefused { addr: "x".into() }.category(), "connection");
        assert_eq!(ModbusError::IllegalFunction { fc: 0x03 }.category(), "protocol");
        assert_eq!(ModbusError::SlaveIdConflict { id: 1 }.category(), "application");
        assert_eq!(ModbusError::Io("x".into()).category(), "generic");
    }

    #[test]
    fn test_error_serialize_json() {
        let err = ModbusError::ConnectionTimeout { addr: "127.0.0.1:502".into(), timeout_ms: 3000 };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"category\":\"connection_timeout\""));
        assert!(json.contains("\"addr\":\"127.0.0.1:502\""));
        assert!(json.contains("\"timeout_ms\":3000"));
    }

    #[test]
    fn test_error_serialize_protocol() {
        let err = ModbusError::CrcMismatch { expected: 0x1234, actual: 0x5678 };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("crc_mismatch"));
    }

    #[test]
    fn test_error_serialize_io() {
        let err = ModbusError::Io("disk full".into());
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"category\":\"io\""));
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

Add `pub mod error;` to `crates/modbussim-core/src/lib.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p modbussim-core error`
Expected: All 5 tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/modbussim-core/src/error.rs crates/modbussim-core/src/lib.rs
git commit -m "feat(core): add unified ModbusError enum with serde serialization"
```

---

## Task 2: Frontend Error Handler Composable

**Files:**
- Create: `shared-frontend/src/composables/useErrorHandler.ts`
- Modify: `shared-frontend/src/index.ts`

- [ ] **Step 1: Create useErrorHandler.ts**

```typescript
// shared-frontend/src/composables/useErrorHandler.ts

import { ref, readonly } from 'vue'

export interface ModbusErrorInfo {
  category: string
  message: string
  level: 'error' | 'warning' | 'info'
  persistent: boolean
}

export interface Toast {
  id: number
  message: string
  level: 'error' | 'warning' | 'info'
  persistent: boolean
  timestamp: number
}

const toasts = ref<Toast[]>([])
let nextId = 0

function classifyError(errorString: string): ModbusErrorInfo {
  // Try to parse as JSON (structured ModbusError from backend)
  try {
    const parsed = JSON.parse(errorString)
    if (parsed.category) {
      const category = parsed.category as string
      // Connection errors are persistent (red)
      if (category.startsWith('connection') || category.startsWith('serial_port')) {
        return { category, message: errorString, level: 'error', persistent: true }
      }
      // Protocol errors are transient warnings (orange, 3s)
      if (['illegal_function', 'illegal_data_address', 'illegal_data_value',
           'slave_device_failure', 'response_timeout', 'crc_mismatch',
           'lrc_mismatch', 'frame_error'].includes(category)) {
        return { category, message: parsed.detail || errorString, level: 'warning', persistent: false }
      }
      // Application errors are info (blue)
      return { category, message: errorString, level: 'info', persistent: false }
    }
  } catch {
    // Not JSON, treat as generic string error
  }

  // Heuristic classification for plain string errors
  const lower = errorString.toLowerCase()
  if (lower.includes('connection') || lower.includes('refused') || lower.includes('timeout')) {
    return { category: 'connection', message: errorString, level: 'error', persistent: true }
  }
  if (lower.includes('not found') || lower.includes('not connected')) {
    return { category: 'application', message: errorString, level: 'warning', persistent: false }
  }
  return { category: 'generic', message: errorString, level: 'info', persistent: false }
}

function addToast(message: string, level: Toast['level'], persistent: boolean) {
  const id = nextId++
  const toast: Toast = { id, message, level, persistent, timestamp: Date.now() }
  toasts.value.push(toast)

  if (!persistent) {
    setTimeout(() => {
      removeToast(id)
    }, 3000)
  }
}

function removeToast(id: number) {
  toasts.value = toasts.value.filter(t => t.id !== id)
}

function clearToasts() {
  toasts.value = []
}

export function useErrorHandler() {
  function handleError(error: unknown) {
    const errorStr = String(error)
    const info = classifyError(errorStr)
    addToast(info.message, info.level, info.persistent)
  }

  return {
    toasts: readonly(toasts),
    handleError,
    removeToast,
    clearToasts,
  }
}
```

- [ ] **Step 2: Export from index.ts**

Add to `shared-frontend/src/index.ts`:

```typescript
export { useErrorHandler } from './composables/useErrorHandler'
export type { ModbusErrorInfo, Toast } from './composables/useErrorHandler'
```

- [ ] **Step 3: Build both frontends**

```bash
cd frontend && npm run build && cd ../master-frontend && npm run build
```

Expected: Both build successfully

- [ ] **Step 4: Commit**

```bash
git add shared-frontend/src/composables/useErrorHandler.ts shared-frontend/src/index.ts
git commit -m "feat(shared-frontend): add useErrorHandler composable with toast system"
```

---

## Task 3: Auto-Reconnect for MasterConnection

**Files:**
- Create: `crates/modbussim-core/src/reconnect.rs`
- Modify: `crates/modbussim-core/src/master.rs`
- Modify: `crates/modbussim-core/src/lib.rs`

- [ ] **Step 1: Create reconnect.rs with ReconnectPolicy and tests**

```rust
// crates/modbussim-core/src/reconnect.rs

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for automatic reconnection behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectPolicy {
    pub enabled: bool,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_factor: f64,
    pub max_attempts: Option<u32>,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_factor: 2.0,
            max_attempts: None, // infinite
        }
    }
}

impl ReconnectPolicy {
    /// Calculate the delay for the given attempt number (0-based).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms = (self.initial_delay_ms as f64) * self.backoff_factor.powi(attempt as i32);
        let clamped = delay_ms.min(self.max_delay_ms as f64) as u64;
        Duration::from_millis(clamped)
    }

    /// Check if reconnection should continue for the given attempt.
    pub fn should_retry(&self, attempt: u32) -> bool {
        if !self.enabled {
            return false;
        }
        match self.max_attempts {
            Some(max) => attempt < max,
            None => true,
        }
    }
}

/// State of the reconnection process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconnectState {
    Idle,
    Reconnecting { attempt: u32 },
    GaveUp { attempts: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = ReconnectPolicy::default();
        assert!(policy.enabled);
        assert_eq!(policy.initial_delay_ms, 1000);
        assert_eq!(policy.max_delay_ms, 30000);
        assert_eq!(policy.backoff_factor, 2.0);
        assert!(policy.max_attempts.is_none());
    }

    #[test]
    fn test_delay_exponential_backoff() {
        let policy = ReconnectPolicy::default();
        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(1000));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(2000));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(4000));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(8000));
    }

    #[test]
    fn test_delay_clamped_to_max() {
        let policy = ReconnectPolicy::default();
        // 1000 * 2^5 = 32000 > max 30000
        assert_eq!(policy.delay_for_attempt(5), Duration::from_millis(30000));
        assert_eq!(policy.delay_for_attempt(10), Duration::from_millis(30000));
    }

    #[test]
    fn test_should_retry_unlimited() {
        let policy = ReconnectPolicy::default();
        assert!(policy.should_retry(0));
        assert!(policy.should_retry(100));
        assert!(policy.should_retry(u32::MAX - 1));
    }

    #[test]
    fn test_should_retry_limited() {
        let policy = ReconnectPolicy {
            max_attempts: Some(3),
            ..Default::default()
        };
        assert!(policy.should_retry(0));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));
        assert!(!policy.should_retry(4));
    }

    #[test]
    fn test_should_retry_disabled() {
        let policy = ReconnectPolicy {
            enabled: false,
            ..Default::default()
        };
        assert!(!policy.should_retry(0));
    }

    #[test]
    fn test_reconnect_state_serde() {
        let state = ReconnectState::Reconnecting { attempt: 3 };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("reconnecting"));
        assert!(json.contains("3"));
    }
}
```

- [ ] **Step 2: Register module**

Add `pub mod reconnect;` to lib.rs.

- [ ] **Step 3: Run tests**

Run: `cargo test -p modbussim-core reconnect`
Expected: All 7 tests pass

- [ ] **Step 4: Add Reconnecting state to MasterState**

In `crates/modbussim-core/src/master.rs`, add `Reconnecting` to `MasterState`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MasterState {
    Disconnected,
    Connected,
    Reconnecting,
    Error,
}
```

Add `reconnect_policy` and `reconnect_handle` fields to `MasterConnection`:

```rust
use crate::reconnect::ReconnectPolicy;

pub struct MasterConnection {
    pub config: MasterConfig,
    pub transport: Transport,
    state: MasterState,
    transport_ctx: Option<TransportCtx>,
    poll_tasks: HashMap<String, PollTaskHandle>,
    log_collector: Option<Arc<LogCollector>>,
    pub reconnect_policy: ReconnectPolicy,
    reconnect_handle: Option<tokio::task::JoinHandle<()>>,
}
```

Update `new()` to initialize `reconnect_policy: ReconnectPolicy::default()` and `reconnect_handle: None`.

- [ ] **Step 5: Build to verify**

Run: `cargo build --workspace`
Expected: Clean compilation

- [ ] **Step 6: Commit**

```bash
git add crates/modbussim-core/src/reconnect.rs crates/modbussim-core/src/master.rs crates/modbussim-core/src/lib.rs
git commit -m "feat(core): add ReconnectPolicy with exponential backoff and MasterState::Reconnecting"
```

---

## Task 4: Virtual Scrolling for RegisterTable and DataTable

**Files:**
- Modify: `frontend/package.json`
- Modify: `master-frontend/package.json`
- Modify: `frontend/src/components/RegisterTable.vue`
- Modify: `master-frontend/src/components/DataTable.vue`

- [ ] **Step 1: Install @tanstack/vue-virtual**

Add to both `frontend/package.json` and `master-frontend/package.json` dependencies:

```json
"@tanstack/vue-virtual": "^3"
```

Run: `npm install` from project root.

- [ ] **Step 2: Refactor RegisterTable.vue for virtual scrolling**

Read the current `frontend/src/components/RegisterTable.vue` first to understand the table rendering.

Add the virtualizer import and setup:

```typescript
import { useVirtualizer } from '@tanstack/vue-virtual'

// After filteredRegisters computed:
const parentRef = ref<HTMLElement | null>(null)
const ROW_HEIGHT = 32

const virtualizer = useVirtualizer({
  count: computed(() => filteredRegisters.value.length),
  getScrollElement: () => parentRef.value,
  estimateSize: () => ROW_HEIGHT,
  overscan: 5,
})
```

Replace the current table body with virtual rows:

```html
<div ref="parentRef" class="table-scroll-container" style="overflow-y: auto; contain: strict;">
  <div :style="{ height: `${virtualizer.getTotalSize()}px`, position: 'relative' }">
    <table class="register-table">
      <thead>
        <!-- existing thead -->
      </thead>
    </table>
    <div
      v-for="virtualRow in virtualizer.getVirtualItems()"
      :key="virtualRow.key"
      :style="{
        position: 'absolute',
        top: 0,
        left: 0,
        width: '100%',
        height: `${ROW_HEIGHT}px`,
        transform: `translateY(${virtualRow.start}px)`,
      }"
    >
      <!-- Render the row for filteredRegisters[virtualRow.index] -->
    </div>
  </div>
</div>
```

Keep all existing functionality (search filter, multi-select, inline editing, Tab format switching) working with the virtual row approach.

- [ ] **Step 3: Refactor DataTable.vue similarly**

Apply the same virtual scrolling pattern to `master-frontend/src/components/DataTable.vue`.

- [ ] **Step 4: Build both frontends**

```bash
cd frontend && npm run build && cd ../master-frontend && npm run build
```

Expected: Both build successfully

- [ ] **Step 5: Commit**

```bash
git add frontend/ master-frontend/ package-lock.json
git commit -m "feat(frontend): add virtual scrolling to RegisterTable and DataTable"
```

---

## Task 5: Log Panel Search and Filter

**Files:**
- Create: `shared-frontend/src/composables/useLogFilter.ts`
- Modify: `shared-frontend/src/index.ts`
- Modify: `frontend/src/components/LogPanel.vue`
- Modify: `master-frontend/src/components/LogPanel.vue`

- [ ] **Step 1: Create useLogFilter composable**

```typescript
// shared-frontend/src/composables/useLogFilter.ts

import { ref, computed, type Ref } from 'vue'
import type { LogEntry } from '../types/modbus'

export type DirectionFilter = 'all' | 'rx' | 'tx'
export type FcFilter = 'all' | string  // 'FC01', 'FC02', etc.

export function useLogFilter(logs: Ref<LogEntry[]>) {
  const searchQuery = ref('')
  const directionFilter = ref<DirectionFilter>('all')
  const fcFilter = ref<FcFilter>('all')

  const filteredLogs = computed(() => {
    let result = logs.value

    // Direction filter
    if (directionFilter.value !== 'all') {
      const dir = directionFilter.value.toUpperCase()
      result = result.filter(log => {
        const logDir = (log.direction || '').toUpperCase()
        return logDir === dir || logDir === directionFilter.value.toUpperCase()
      })
    }

    // Function code filter
    if (fcFilter.value !== 'all') {
      result = result.filter(log => {
        const fc = (log.function_code || '').toUpperCase()
        return fc === fcFilter.value || fc.includes(fcFilter.value)
      })
    }

    // Text search (debounced externally if needed)
    if (searchQuery.value.trim()) {
      const q = searchQuery.value.toLowerCase()
      result = result.filter(log =>
        (log.detail || '').toLowerCase().includes(q) ||
        (log.function_code || '').toLowerCase().includes(q) ||
        (log.timestamp || '').toLowerCase().includes(q)
      )
    }

    return result
  })

  // Available FC values from current logs (for dropdown)
  const availableFcs = computed(() => {
    const fcs = new Set<string>()
    logs.value.forEach(log => {
      if (log.function_code) {
        fcs.add(log.function_code)
      }
    })
    return Array.from(fcs).sort()
  })

  const filterSummary = computed(() => {
    const parts: string[] = []
    if (directionFilter.value !== 'all') parts.push(directionFilter.value.toUpperCase())
    if (fcFilter.value !== 'all') parts.push(fcFilter.value)
    if (searchQuery.value.trim()) parts.push(`"${searchQuery.value}"`)
    return parts.length > 0 ? parts.join(' + ') : ''
  })

  function resetFilters() {
    searchQuery.value = ''
    directionFilter.value = 'all'
    fcFilter.value = 'all'
  }

  return {
    searchQuery,
    directionFilter,
    fcFilter,
    filteredLogs,
    availableFcs,
    filterSummary,
    resetFilters,
  }
}
```

- [ ] **Step 2: Export from index.ts**

Add to `shared-frontend/src/index.ts`:

```typescript
export { useLogFilter } from './composables/useLogFilter'
export type { DirectionFilter, FcFilter } from './composables/useLogFilter'
```

- [ ] **Step 3: Update frontend LogPanel.vue with filter UI**

Read the current file. Add the filter controls above the log table:

```html
<!-- Add after the header buttons, before the table -->
<div v-if="expanded" class="log-filters">
  <input
    v-model="searchQuery"
    type="text"
    class="filter-input"
    placeholder="搜索日志..."
  />
  <select v-model="directionFilter" class="filter-select">
    <option value="all">方向: 全部</option>
    <option value="rx">RX</option>
    <option value="tx">TX</option>
  </select>
  <select v-model="fcFilter" class="filter-select">
    <option value="all">功能码: 全部</option>
    <option v-for="fc in availableFcs" :key="fc" :value="fc">{{ fc }}</option>
  </select>
  <span v-if="filterSummary" class="filter-badge">{{ filterSummary }}</span>
</div>
```

In script:

```typescript
import { useLogFilter } from 'shared-frontend'

// After useLogPanel():
const { searchQuery, directionFilter, fcFilter, filteredLogs, availableFcs, filterSummary } = useLogFilter(logs)
```

Replace `logs` with `filteredLogs` in the template's v-for loop.

Add CSS for the filter controls matching the existing dark theme.

- [ ] **Step 4: Update master-frontend LogPanel.vue similarly**

Apply the same filter UI and composable integration.

- [ ] **Step 5: Build both frontends**

```bash
cd frontend && npm run build && cd ../master-frontend && npm run build
```

Expected: Both build successfully

- [ ] **Step 6: Commit**

```bash
git add shared-frontend/ frontend/src/components/LogPanel.vue master-frontend/src/components/LogPanel.vue
git commit -m "feat(frontend): add log search and filter to both LogPanel components"
```

---

## Task 6: Dynamic Data Source Module

**Files:**
- Create: `crates/modbussim-core/src/data_source.rs`
- Modify: `crates/modbussim-core/src/lib.rs`

- [ ] **Step 1: Create data_source.rs with DataSource enum and generation tests**

```rust
// crates/modbussim-core/src/data_source.rs

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Type of data source for register value generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DataSource {
    Fixed { value: u16 },
    Random { min: u16, max: u16 },
    Sine { amplitude: f64, frequency: f64, offset: f64, phase: f64 },
    Sawtooth { min: u16, max: u16, period_ms: u64 },
    Triangle { min: u16, max: u16, period_ms: u64 },
    Counter { start: u16, step: i16, wrap: bool },
    CsvPlayback { values: Vec<u16>, loop_playback: bool },
}

/// Configuration binding a data source to a register.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConfig {
    pub source: DataSource,
    pub update_interval_ms: u64,
}

impl Default for DataSourceConfig {
    fn default() -> Self {
        Self {
            source: DataSource::Fixed { value: 0 },
            update_interval_ms: 1000,
        }
    }
}

/// Runtime state for a data source (tracks time and counters).
#[derive(Debug)]
pub struct DataSourceState {
    pub config: DataSourceConfig,
    start_time: Instant,
    counter_value: i32,
    csv_index: usize,
}

impl DataSourceState {
    pub fn new(config: DataSourceConfig) -> Self {
        let initial = match &config.source {
            DataSource::Counter { start, .. } => *start as i32,
            _ => 0,
        };
        Self {
            config,
            start_time: Instant::now(),
            counter_value: initial,
            csv_index: 0,
        }
    }

    /// Generate the next value based on the data source type.
    pub fn next_value(&mut self) -> u16 {
        let elapsed_ms = self.start_time.elapsed().as_millis() as f64;

        match &self.config.source {
            DataSource::Fixed { value } => *value,

            DataSource::Random { min, max } => {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                rng.gen_range(*min..=*max)
            }

            DataSource::Sine { amplitude, frequency, offset, phase } => {
                let t = elapsed_ms / 1000.0;
                let value = offset + amplitude * (2.0 * std::f64::consts::PI * frequency * t + phase).sin();
                value.round().clamp(0.0, 65535.0) as u16
            }

            DataSource::Sawtooth { min, max, period_ms } => {
                if *period_ms == 0 { return *min; }
                let phase = (elapsed_ms as u64 % period_ms) as f64 / *period_ms as f64;
                let range = *max as f64 - *min as f64;
                (*min as f64 + range * phase).round() as u16
            }

            DataSource::Triangle { min, max, period_ms } => {
                if *period_ms == 0 { return *min; }
                let phase = (elapsed_ms as u64 % period_ms) as f64 / *period_ms as f64;
                let range = *max as f64 - *min as f64;
                let triangle = if phase < 0.5 {
                    phase * 2.0
                } else {
                    2.0 - phase * 2.0
                };
                (*min as f64 + range * triangle).round() as u16
            }

            DataSource::Counter { step, wrap, .. } => {
                let value = self.counter_value;
                let next = self.counter_value + *step as i32;
                if *wrap {
                    self.counter_value = ((next % 65536) + 65536) % 65536;
                } else {
                    self.counter_value = next.clamp(0, 65535);
                }
                value.clamp(0, 65535) as u16
            }

            DataSource::CsvPlayback { values, loop_playback } => {
                if values.is_empty() { return 0; }
                let idx = self.csv_index;
                if *loop_playback {
                    self.csv_index = (idx + 1) % values.len();
                } else if idx < values.len() - 1 {
                    self.csv_index = idx + 1;
                }
                values[idx]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_source() {
        let mut state = DataSourceState::new(DataSourceConfig {
            source: DataSource::Fixed { value: 42 },
            update_interval_ms: 100,
        });
        assert_eq!(state.next_value(), 42);
        assert_eq!(state.next_value(), 42);
    }

    #[test]
    fn test_random_source_in_range() {
        let mut state = DataSourceState::new(DataSourceConfig {
            source: DataSource::Random { min: 10, max: 20 },
            update_interval_ms: 100,
        });
        for _ in 0..100 {
            let v = state.next_value();
            assert!(v >= 10 && v <= 20, "value {} out of range", v);
        }
    }

    #[test]
    fn test_counter_increment() {
        let mut state = DataSourceState::new(DataSourceConfig {
            source: DataSource::Counter { start: 0, step: 1, wrap: false },
            update_interval_ms: 100,
        });
        assert_eq!(state.next_value(), 0);
        assert_eq!(state.next_value(), 1);
        assert_eq!(state.next_value(), 2);
    }

    #[test]
    fn test_counter_wrap() {
        let mut state = DataSourceState::new(DataSourceConfig {
            source: DataSource::Counter { start: 65534, step: 2, wrap: true },
            update_interval_ms: 100,
        });
        assert_eq!(state.next_value(), 65534);
        let v = state.next_value();
        // 65534 + 2 = 65536, wrapped to 0
        assert_eq!(v, 0);
    }

    #[test]
    fn test_counter_no_wrap_clamp() {
        let mut state = DataSourceState::new(DataSourceConfig {
            source: DataSource::Counter { start: 65534, step: 5, wrap: false },
            update_interval_ms: 100,
        });
        assert_eq!(state.next_value(), 65534);
        assert_eq!(state.next_value(), 65535); // clamped
    }

    #[test]
    fn test_csv_playback_loop() {
        let mut state = DataSourceState::new(DataSourceConfig {
            source: DataSource::CsvPlayback { values: vec![10, 20, 30], loop_playback: true },
            update_interval_ms: 100,
        });
        assert_eq!(state.next_value(), 10);
        assert_eq!(state.next_value(), 20);
        assert_eq!(state.next_value(), 30);
        assert_eq!(state.next_value(), 10); // loops
    }

    #[test]
    fn test_csv_playback_no_loop() {
        let mut state = DataSourceState::new(DataSourceConfig {
            source: DataSource::CsvPlayback { values: vec![10, 20, 30], loop_playback: false },
            update_interval_ms: 100,
        });
        assert_eq!(state.next_value(), 10);
        assert_eq!(state.next_value(), 20);
        assert_eq!(state.next_value(), 30);
        assert_eq!(state.next_value(), 30); // stays at last
    }

    #[test]
    fn test_csv_playback_empty() {
        let mut state = DataSourceState::new(DataSourceConfig {
            source: DataSource::CsvPlayback { values: vec![], loop_playback: true },
            update_interval_ms: 100,
        });
        assert_eq!(state.next_value(), 0);
    }

    #[test]
    fn test_data_source_serde_roundtrip() {
        let config = DataSourceConfig {
            source: DataSource::Sine { amplitude: 100.0, frequency: 0.5, offset: 500.0, phase: 0.0 },
            update_interval_ms: 500,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"sine\""));
        let loaded: DataSourceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.update_interval_ms, 500);
    }

    #[test]
    fn test_sawtooth_boundaries() {
        let mut state = DataSourceState::new(DataSourceConfig {
            source: DataSource::Sawtooth { min: 100, max: 200, period_ms: 0 },
            update_interval_ms: 100,
        });
        // period_ms=0 should return min
        assert_eq!(state.next_value(), 100);
    }
}
```

- [ ] **Step 2: Register module**

Add `pub mod data_source;` to lib.rs.

- [ ] **Step 3: Run tests**

Run: `cargo test -p modbussim-core data_source`
Expected: All 10 tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/modbussim-core/src/data_source.rs crates/modbussim-core/src/lib.rs
git commit -m "feat(core): add DataSource module for dynamic register value generation"
```

---

## Task 7: Data Source Tauri Commands for Slave App

**Files:**
- Modify: `crates/modbussim-app/src/commands.rs`
- Modify: `crates/modbussim-app/src/lib.rs`

- [ ] **Step 1: Add data source management commands**

Add to `crates/modbussim-app/src/commands.rs`:

```rust
use modbussim_core::data_source::{DataSource, DataSourceConfig, DataSourceState};

#[derive(Debug, Deserialize)]
pub struct SetDataSourceRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub register_type: String,
    pub address: u16,
    pub source: DataSource,
    pub update_interval_ms: u64,
}

#[tauri::command]
pub async fn set_data_source(
    state: State<'_, AppState>,
    request: SetDataSourceRequest,
) -> Result<(), String> {
    // Store the data source config in the app state
    // For Phase 3, we'll store DataSourceConfig per register address
    // and run a background timer that updates register values
    let config = DataSourceConfig {
        source: request.source,
        update_interval_ms: request.update_interval_ms,
    };
    
    // Store in app state (add a data_sources field to AppState)
    let mut data_sources = state.data_sources.write().await;
    let key = format!("{}:{}:{}:{}", request.connection_id, request.slave_id, request.register_type, request.address);
    data_sources.insert(key, DataSourceState::new(config));
    
    Ok(())
}

#[tauri::command]
pub async fn remove_data_source(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
    register_type: String,
    address: u16,
) -> Result<(), String> {
    let mut data_sources = state.data_sources.write().await;
    let key = format!("{}:{}:{}:{}", connection_id, slave_id, register_type, address);
    data_sources.remove(&key);
    Ok(())
}

#[tauri::command]
pub async fn list_data_sources(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let data_sources = state.data_sources.read().await;
    let prefix = format!("{}:", connection_id);
    let results: Vec<serde_json::Value> = data_sources
        .iter()
        .filter(|(k, _)| k.starts_with(&prefix))
        .map(|(k, ds)| {
            serde_json::json!({
                "key": k,
                "config": ds.config,
            })
        })
        .collect();
    Ok(results)
}
```

- [ ] **Step 2: Add data_sources to AppState**

In `crates/modbussim-app/src/state.rs`, add:

```rust
use modbussim_core::data_source::DataSourceState;

pub struct AppState {
    pub slave_connections: RwLock<HashMap<String, SlaveConnectionState>>,
    pub next_slave_id: RwLock<u32>,
    pub data_sources: RwLock<HashMap<String, DataSourceState>>,
}
```

Update `Default` impl to include `data_sources: RwLock::new(HashMap::new())`.

- [ ] **Step 3: Register commands**

Add `set_data_source`, `remove_data_source`, `list_data_sources` to the invoke_handler in `lib.rs`.

- [ ] **Step 4: Build**

Run: `cargo build -p modbussim-app`
Expected: Clean compilation

- [ ] **Step 5: Commit**

```bash
git add crates/modbussim-app/src/commands.rs crates/modbussim-app/src/state.rs crates/modbussim-app/src/lib.rs
git commit -m "feat(slave-app): add data source management Tauri commands"
```

---

## Task 8: Data Source Background Runner

**Files:**
- Modify: `crates/modbussim-app/src/commands.rs`

- [ ] **Step 1: Add a background timer that updates registers from data sources**

Add a `start_data_source_runner` command that spawns a background task:

```rust
#[tauri::command]
pub async fn start_data_source_runner(
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let data_sources = state.data_sources.clone();
    let connections = state.slave_connections.clone();
    
    // Check if already running (simple flag in AppState)
    // For simplicity, just spawn the task
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
        loop {
            interval.tick().await;
            
            let mut ds = data_sources.write().await;
            let conns = connections.read().await;
            
            for (key, source_state) in ds.iter_mut() {
                // Parse key: "connection_id:slave_id:register_type:address"
                let parts: Vec<&str> = key.splitn(4, ':').collect();
                if parts.len() != 4 { continue; }
                
                let conn_id = parts[0];
                let slave_id: u8 = match parts[1].parse() { Ok(v) => v, Err(_) => continue };
                let reg_type = parts[2];
                let address: u16 = match parts[3].parse() { Ok(v) => v, Err(_) => continue };
                
                // Check if it's time to update
                let elapsed = source_state.start_time.elapsed().as_millis() as u64;
                if elapsed < source_state.config.update_interval_ms { continue; }
                
                // Generate new value
                let value = source_state.next_value();
                
                // Write to register
                if let Some(conn) = conns.get(conn_id) {
                    let mut devices = conn.connection.devices.write().await;
                    if let Some(device) = devices.get_mut(&slave_id) {
                        match reg_type {
                            "holding_register" => {
                                device.register_map.holding_registers.insert(address, value);
                                device.register_map.input_registers.insert(address, value);
                            }
                            "coil" => {
                                device.register_map.coils.insert(address, value != 0);
                                device.register_map.discrete_inputs.insert(address, value != 0);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    });
    
    Ok(())
}
```

**Note:** This is a simplified runner. A production version would track individual update intervals per source. For Phase 3, the 100ms tick checking each source's elapsed time is sufficient.

- [ ] **Step 2: Register command**

Add `start_data_source_runner` to invoke_handler.

- [ ] **Step 3: Build**

Run: `cargo build -p modbussim-app`
Expected: Clean compilation

- [ ] **Step 4: Commit**

```bash
git add crates/modbussim-app/src/commands.rs crates/modbussim-app/src/lib.rs
git commit -m "feat(slave-app): add data source background runner for register simulation"
```

---

## Task 9: Integration Verification

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test --workspace --lib`
Report pass/fail counts.

- [ ] **Step 2: Build everything**

```bash
cargo build --workspace
cd frontend && npm run build
cd ../master-frontend && npm run build
```

Expected: All builds succeed

- [ ] **Step 3: Verify new module tests**

```bash
cargo test -p modbussim-core error
cargo test -p modbussim-core reconnect
cargo test -p modbussim-core data_source
```

Expected: All pass

- [ ] **Step 4: Git log summary**

```bash
git log --oneline
```

- [ ] **Step 5: Final cleanup commit if needed**

```bash
git status
```
