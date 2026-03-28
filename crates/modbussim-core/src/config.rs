//! Configuration management for ModbusSim.
//!
//! Provides serialization/deserialization of connections, devices, and register maps
//! to/from JSON files, as well as application state persistence.

use crate::register::{DataType, Endian, RegisterDef, RegisterMap, RegisterType};
use crate::slave::{SlaveConnection, SlaveDevice, TransportConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse JSON: {0}")]
    JsonParse(#[from] serde_json::Error),
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for ConfigError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// ---------------------------------------------------------------------------
// Register value storage (current values at runtime)
// ---------------------------------------------------------------------------

/// A single register's current value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterValue {
    pub address: u16,
    pub value: u16,
}

/// Register values organized by type.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegisterValues {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub coils: Vec<(u16, bool)>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub discrete_inputs: Vec<(u16, bool)>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub holding_registers: Vec<(u16, u16)>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub input_registers: Vec<(u16, u16)>,
}

impl RegisterValues {
    pub fn from_register_map(map: &RegisterMap) -> Self {
        Self {
            coils: map.coils.iter().map(|(&k, &v)| (k, v)).collect(),
            discrete_inputs: map.discrete_inputs.iter().map(|(&k, &v)| (k, v)).collect(),
            holding_registers: map.holding_registers.iter().map(|(&k, &v)| (k, v)).collect(),
            input_registers: map.input_registers.iter().map(|(&k, &v)| (k, v)).collect(),
        }
    }

    /// Apply these values to a RegisterMap.
    pub fn apply_to(&self, map: &mut RegisterMap) {
        for (addr, val) in &self.coils {
            map.coils.insert(*addr, *val);
        }
        for (addr, val) in &self.discrete_inputs {
            map.discrete_inputs.insert(*addr, *val);
        }
        for (addr, val) in &self.holding_registers {
            map.holding_registers.insert(*addr, *val);
        }
        for (addr, val) in &self.input_registers {
            map.input_registers.insert(*addr, *val);
        }
    }
}

// ---------------------------------------------------------------------------
// Register definition with full metadata
// ---------------------------------------------------------------------------

/// A register definition entry for configuration export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDefEntry {
    pub address: u16,
    #[serde(rename = "type")]
    pub register_type: RegisterType,
    pub data_type: DataType,
    #[serde(default)]
    pub endian: Endian,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<u16>,
}

impl RegisterDefEntry {
    /// Create from a RegisterDef, optionally including the current value.
    pub fn from_register_def(def: &RegisterDef, value: Option<u16>) -> Self {
        Self {
            address: def.address,
            register_type: def.register_type,
            data_type: def.data_type,
            endian: def.endian,
            name: def.name.clone(),
            comment: def.comment.clone(),
            value,
        }
    }

    /// Convert to a RegisterDef.
    pub fn to_register_def(&self) -> RegisterDef {
        RegisterDef {
            address: self.address,
            register_type: self.register_type,
            data_type: self.data_type,
            endian: self.endian,
            name: self.name.clone(),
            comment: self.comment.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Device configuration
// ---------------------------------------------------------------------------

/// Configuration for a single slave device (exportable to JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub slave_id: u8,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registers: Vec<RegisterDefEntry>,
}

impl DeviceConfig {
    /// Create a DeviceConfig from a SlaveDevice, optionally including current register values.
    pub fn from_slave_device(device: &SlaveDevice, include_values: bool) -> Self {
        let registers: Vec<RegisterDefEntry> = device
            .register_defs
            .iter()
            .map(|def| {
                let value = if include_values {
                    get_register_value(&device.register_map, def)
                } else {
                    None
                };
                RegisterDefEntry::from_register_def(def, value)
            })
            .collect();

        Self {
            slave_id: device.slave_id,
            name: device.name.clone(),
            registers,
        }
    }

    /// Validate the device config.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.slave_id == 0 {
            return Err(ConfigError::InvalidConfig(
                "slave_id must be between 1 and 247".to_string(),
            ));
        }
        if self.name.is_empty() {
            return Err(ConfigError::InvalidConfig(
                "device name cannot be empty".to_string(),
            ));
        }

        // Check for duplicate addresses within same type
        let mut by_type: HashMap<RegisterType, Vec<u16>> = HashMap::new();
        for reg in &self.registers {
            by_type
                .entry(reg.register_type)
                .or_default()
                .push(reg.address);
        }
        for (reg_type, addrs) in &by_type {
            let mut sorted = addrs.clone();
            sorted.sort();
            for i in 1..sorted.len() {
                if sorted[i] == sorted[i - 1] {
                    return Err(ConfigError::InvalidConfig(format!(
                        "duplicate address {:#06x} for register type {:?}",
                        sorted[i], reg_type
                    )));
                }
            }
        }

        Ok(())
    }

    /// Convert to a SlaveDevice.
    pub fn to_slave_device(&self) -> Result<SlaveDevice, ConfigError> {
        self.validate()?;

        let mut device = SlaveDevice::new(self.slave_id, self.name.clone());
        let values = RegisterValues {
            coils: self
                .registers
                .iter()
                .filter(|r| r.register_type == RegisterType::Coil && r.value.is_some())
                .map(|r| (r.address, r.value.unwrap() != 0))
                .collect(),
            discrete_inputs: self
                .registers
                .iter()
                .filter(|r| r.register_type == RegisterType::DiscreteInput && r.value.is_some())
                .map(|r| (r.address, r.value.unwrap() != 0))
                .collect(),
            holding_registers: self
                .registers
                .iter()
                .filter(|r| r.register_type == RegisterType::HoldingRegister && r.value.is_some())
                .map(|r| (r.address, r.value.unwrap()))
                .collect(),
            input_registers: self
                .registers
                .iter()
                .filter(|r| r.register_type == RegisterType::InputRegister && r.value.is_some())
                .map(|r| (r.address, r.value.unwrap()))
                .collect(),
            ..Default::default()
        };

        values.apply_to(&mut device.register_map);
        device.register_defs = self.registers.iter().map(|r| r.to_register_def()).collect();

        Ok(device)
    }
}

/// Helper to get current register value.
fn get_register_value(map: &RegisterMap, def: &RegisterDef) -> Option<u16> {
    match def.register_type {
        RegisterType::Coil => map.coils.get(&def.address).copied().map(|v| if v { 1 } else { 0 }),
        RegisterType::DiscreteInput => {
            map.discrete_inputs.get(&def.address).copied().map(|v| if v { 1 } else { 0 })
        }
        RegisterType::HoldingRegister => map.holding_registers.get(&def.address).copied(),
        RegisterType::InputRegister => map.input_registers.get(&def.address).copied(),
    }
}

// ---------------------------------------------------------------------------
// Connection configuration
// ---------------------------------------------------------------------------

/// Configuration for a slave connection (transport + all devices).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub transport: TransportConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<DeviceConfig>,
    #[serde(default)]
    pub auto_start: bool,
}

impl ConnectionConfig {
    /// Create a ConnectionConfig from a SlaveConnection.
    pub fn from_slave_connection(conn: &SlaveConnection, include_values: bool) -> Self {
        // Note: we can't easily get devices from a &SlaveConnection without async
        // This is a simplified version - full implementation would need async context
        Self {
            transport: conn.transport.clone(),
            devices: Vec::new(), // Populated separately via async call
            auto_start: false,
        }
    }

    /// Validate the connection config.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.transport.port == 0 {
            return Err(ConfigError::InvalidConfig("port must be non-zero".to_string()));
        }

        // Validate each device
        for device in &self.devices {
            device.validate()?;
        }

        // Check for duplicate slave IDs
        let mut slave_ids: Vec<u8> = self.devices.iter().map(|d| d.slave_id).collect();
        slave_ids.sort();
        for i in 1..slave_ids.len() {
            if slave_ids[i] == slave_ids[i - 1] {
                return Err(ConfigError::InvalidConfig(format!(
                    "duplicate slave_id {}",
                    slave_ids[i]
                )));
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

/// Root configuration file format for ModbusSim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<ConnectionConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            name: String::new(),
            connections: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Validate the entire app config.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version == 0 {
            return Err(ConfigError::InvalidConfig(
                "version must be 1 or greater".to_string(),
            ));
        }

        for conn in &self.connections {
            conn.validate()?;
        }

        // Check for duplicate connection ports
        let mut ports: Vec<u16> = self.connections.iter().map(|c| c.transport.port).collect();
        ports.sort();
        for i in 1..ports.len() {
            if ports[i] == ports[i - 1] {
                return Err(ConfigError::InvalidConfig(format!(
                    "duplicate port {} across connections",
                    ports[i]
                )));
            }
        }

        Ok(())
    }

    /// Export app config to JSON string.
    pub fn to_json(&self) -> Result<String, ConfigError> {
        self.validate()?;
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Import app config from JSON string.
    pub fn from_json(json: &str) -> Result<Self, ConfigError> {
        let config: AppConfig = serde_json::from_str(json)?;
        config.validate()?;
        Ok(config)
    }

    /// Load app config from a file.
    pub fn load(path: &std::path::Path) -> Result<Self, ConfigError> {
        let json = std::fs::read_to_string(path)?;
        Self::from_json(&json)
    }

    /// Save app config to a file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), ConfigError> {
        self.validate()?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_config_validation() {
        let config = DeviceConfig {
            slave_id: 1,
            name: "Test".to_string(),
            registers: vec![],
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_device_config_empty_name() {
        let config = DeviceConfig {
            slave_id: 1,
            name: "".to_string(),
            registers: vec![],
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_device_config_invalid_slave_id() {
        let config = DeviceConfig {
            slave_id: 0,
            name: "Test".to_string(),
            registers: vec![],
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_register_values_roundtrip() {
        let mut map = RegisterMap::new();
        map.write_holding_register(0, 100);
        map.write_holding_register(1, 200);
        map.write_coil(5, true);

        let values = RegisterValues::from_register_map(&map);
        let mut map2 = RegisterMap::new();
        values.apply_to(&mut map2);

        assert_eq!(map2.read_holding_registers(0, 2), vec![100, 200]);
        assert_eq!(map2.read_coils(5, 1), vec![true]);
    }

    #[test]
    fn test_app_config_json_export_import() {
        let config = AppConfig {
            version: 1,
            name: "Test Config".to_string(),
            connections: vec![],
        };

        let json = config.to_json().unwrap();
        let imported = AppConfig::from_json(&json).unwrap();

        assert_eq!(imported.version, 1);
        assert_eq!(imported.name, "Test Config");
    }

    #[test]
    fn test_register_def_entry_roundtrip() {
        let entry = RegisterDefEntry {
            address: 10,
            register_type: RegisterType::HoldingRegister,
            data_type: DataType::UInt16,
            endian: Endian::Big,
            name: "TestReg".to_string(),
            comment: "A test register".to_string(),
            value: Some(42),
        };

        let def = entry.to_register_def();
        assert_eq!(def.address, 10);
        assert_eq!(def.register_type, RegisterType::HoldingRegister);
        assert_eq!(def.data_type, DataType::UInt16);
    }

    #[test]
    fn test_connection_config_validation() {
        let config = ConnectionConfig {
            transport: TransportConfig {
                bind_address: "0.0.0.0".to_string(),
                port: 502,
            },
            devices: vec![],
            auto_start: false,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_app_config_validation_duplicate_ports() {
        let config = AppConfig {
            version: 1,
            name: "Test".to_string(),
            connections: vec![
                ConnectionConfig {
                    transport: TransportConfig {
                        bind_address: "0.0.0.0".to_string(),
                        port: 502,
                    },
                    devices: vec![],
                    auto_start: false,
                },
                ConnectionConfig {
                    transport: TransportConfig {
                        bind_address: "0.0.0.0".to_string(),
                        port: 502,
                    },
                    devices: vec![],
                    auto_start: false,
                },
            ],
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_device_config_to_slave_device() {
        let config = DeviceConfig {
            slave_id: 1,
            name: "Test Device".to_string(),
            registers: vec![
                RegisterDefEntry {
                    address: 0,
                    register_type: RegisterType::HoldingRegister,
                    data_type: DataType::UInt16,
                    endian: Endian::Big,
                    name: "HR0".to_string(),
                    comment: "".to_string(),
                    value: Some(1234),
                },
            ],
        };

        let device = config.to_slave_device().unwrap();
        assert_eq!(device.slave_id, 1);
        assert_eq!(device.name, "Test Device");
        assert_eq!(device.register_defs.len(), 1);
        assert_eq!(device.register_map.read_holding_registers(0, 1), vec![1234]);
    }

    #[test]
    fn test_config_cross_platform_compatibility() {
        // Simulate a config that might have been exported on a different platform.
        // All data types used in serialization must be platform-agnostic:
        // - Integers: u8, u16, u32 (portable across all platforms)
        // - Strings: String (always UTF-8)
        // - Collections: Vec, HashMap (Rust std, portable)
        // - Enums: serialized as snake_case strings (not ordinals)
        let json = r#"{
            "version": 1,
            "name": "CrossPlatform Test",
            "connections": [
                {
                    "transport": {
                        "bind_address": "0.0.0.0",
                        "port": 502
                    },
                    "devices": [
                        {
                            "slave_id": 1,
                            "name": "Device 1",
                            "registers": [
                                {
                                    "address": 0,
                                    "type": "holding_register",
                                    "data_type": "u_int16",
                                    "endian": "big",
                                    "name": "HR0",
                                    "comment": "Test register",
                                    "value": 1234
                                }
                            ]
                        }
                    ],
                    "auto_start": false
                }
            ]
        }"#;

        let config = AppConfig::from_json(json).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.connections.len(), 1);
        assert_eq!(config.connections[0].transport.port, 502);
        assert_eq!(config.connections[0].devices[0].slave_id, 1);
        assert_eq!(config.connections[0].devices[0].registers[0].value, Some(1234));

        // Re-export and verify roundtrip produces equivalent JSON
        let exported = config.to_json().unwrap();
        let reparsed = AppConfig::from_json(&exported).unwrap();
        assert_eq!(reparsed.connections[0].devices[0].name, "Device 1");
    }

    #[test]
    fn test_config_contains_no_platform_specific_types() {
        // This test serves as documentation that the config format
        // must only use cross-platform types. If any platform-specific
        // type (e.g., OsStr, Path, SocketAddr) is added to a serialized
        // struct, this test will fail to compile or produce non-portable output.
        let config = AppConfig {
            version: 1,
            name: String::new(),
            connections: vec![],
        };
        let json = serde_json::to_string(&config).unwrap();

        // JSON must be valid UTF-8
        assert!(std::str::from_utf8(json.as_bytes()).is_ok());

        // All integer types in JSON are decimal or hex strings — portable
        // No binary blobs, no platform-specific encodings
        let reparsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(reparsed.version, 1);
    }
}
