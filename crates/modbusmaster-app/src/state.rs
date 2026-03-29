//! Application state management for ModbusMaster Tauri app.

use modbussim_core::log_collector::LogCollector;
use modbussim_core::master::{MasterConnection, ReadResult, ScanGroup};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Runtime state for a single master connection.
pub struct MasterConnectionState {
    pub connection: MasterConnection,
    pub scan_groups: Vec<ScanGroup>,
    pub log_collector: Arc<LogCollector>,
    /// Latest polled data for each scan group, keyed by scan_group_id.
    pub cached_data: HashMap<String, CachedPollData>,
}

/// Cached poll result for a scan group.
#[derive(Debug, Clone, Serialize)]
pub struct CachedPollData {
    pub result: ReadResult,
    pub timestamp: String,
}

/// Application state holding all master connections.
pub struct AppState {
    pub master_connections: Arc<RwLock<HashMap<String, MasterConnectionState>>>,
    pub next_conn_id: RwLock<u32>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            master_connections: Arc::new(RwLock::new(HashMap::new())),
            next_conn_id: RwLock::new(1),
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// DTOs for API responses
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterConnectionInfo {
    pub id: String,
    pub target_address: String,
    pub port: u16,
    pub slave_id: u8,
    pub state: String,
    pub scan_group_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanGroupInfo {
    pub id: String,
    pub name: String,
    pub function: String,
    pub start_address: u16,
    pub quantity: u16,
    pub interval_ms: u64,
    pub enabled: bool,
    pub is_polling: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterValueDto {
    pub address: u16,
    pub raw_value: u64,
    pub display_value: String,
    pub is_bool: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResultDto {
    pub scan_group_id: String,
    pub function: String,
    pub start_address: u16,
    pub values: Vec<RegisterValueDto>,
    pub timestamp: String,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Event payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct PollDataPayload {
    pub connection_id: String,
    pub scan_group_id: String,
    pub result: ReadResultDto,
}

#[derive(Debug, Clone, Serialize)]
pub struct PollErrorPayload {
    pub connection_id: String,
    pub scan_group_id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionStateEvent {
    pub id: String,
    pub state: String,
}
