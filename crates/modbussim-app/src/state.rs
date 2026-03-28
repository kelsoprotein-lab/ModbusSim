//! Application state management for Tauri.
//!
//! Holds all runtime state: slave connections and log collectors.

use modbussim_core::log_collector::LogCollector;
use modbussim_core::slave::SlaveConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Runtime state for a slave connection.
pub struct SlaveConnectionState {
    pub connection: SlaveConnection,
    pub log_collector: Arc<LogCollector>,
}

/// Application state holding all active connections.
pub struct AppState {
    pub slave_connections: RwLock<HashMap<String, SlaveConnectionState>>,
    pub next_slave_id: RwLock<u32>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            slave_connections: RwLock::new(HashMap::new()),
            next_slave_id: RwLock::new(1),
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

/// Information about a slave connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SlaveConnectionInfo {
    pub id: String,
    pub bind_address: String,
    pub port: u16,
    pub state: String,
    pub device_count: usize,
}

/// Information about a slave device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveDeviceInfo {
    pub slave_id: u8,
    pub name: String,
    pub register_count: usize,
}

/// A single register value for reading/writing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterValueInfo {
    pub address: u16,
    pub value: u16,
}
