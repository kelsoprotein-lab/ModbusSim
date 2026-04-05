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
    let value: serde_json::Value =
        serde_json::from_str(data).map_err(|e| format!("invalid JSON: {}", e))?;

    let version = value
        .get("version")
        .and_then(|v| v.as_u64())
        .ok_or("missing or invalid version field")?;

    match version {
        1 => serde_json::from_value(value)
            .map_err(|e| format!("failed to parse project v1: {}", e)),
        v => Err(format!("unsupported project version: {}", v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_slave_project() {
        let p = ProjectFile::new_slave();
        assert_eq!(p.version, 1);
        assert_eq!(p.project_type, ProjectType::Slave);
        assert!(p.connections.is_empty());
    }

    #[test]
    fn test_new_master_project() {
        let p = ProjectFile::new_master();
        assert_eq!(p.version, 1);
        assert_eq!(p.project_type, ProjectType::Master);
        assert!(p.connections.is_empty());
    }

    #[test]
    fn test_save_and_load_slave_project() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.modbusproj");

        let mut project = ProjectFile::new_slave();
        project.connections.push(ConnectionConfig {
            id: "conn-1".into(),
            name: "Local TCP".into(),
            transport: TransportConfig::Tcp {
                host: "127.0.0.1".into(),
                port: 502,
            },
            devices: vec![DeviceConfig {
                slave_id: 1,
                registers: RegistersConfig {
                    holding: vec![RegisterBlockConfig {
                        address: 0,
                        count: 10,
                        data_type: Some("uint16".into()),
                        endian: None,
                        values: vec![serde_json::json!(0), serde_json::json!(100)],
                        names: HashMap::from([("0".into(), "Temperature".into())]),
                    }],
                    ..Default::default()
                },
            }],
            scan_groups: vec![],
        });

        save_project(&project, &path).unwrap();
        let loaded = load_project(&path).unwrap();

        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.project_type, ProjectType::Slave);
        assert_eq!(loaded.connections.len(), 1);

        let conn = &loaded.connections[0];
        assert_eq!(conn.id, "conn-1");
        assert_eq!(conn.name, "Local TCP");
        match &conn.transport {
            TransportConfig::Tcp { host, port } => {
                assert_eq!(host, "127.0.0.1");
                assert_eq!(*port, 502);
            }
        }

        assert_eq!(conn.devices.len(), 1);
        let dev = &conn.devices[0];
        assert_eq!(dev.slave_id, 1);
        assert_eq!(dev.registers.holding.len(), 1);
        assert_eq!(dev.registers.holding[0].address, 0);
        assert_eq!(dev.registers.holding[0].count, 10);
        assert_eq!(
            dev.registers.holding[0].data_type,
            Some("uint16".to_string())
        );
        assert_eq!(dev.registers.holding[0].values.len(), 2);
        assert_eq!(
            dev.registers.holding[0].names.get("0"),
            Some(&"Temperature".to_string())
        );
    }

    #[test]
    fn test_save_and_load_master_project() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("master.modbusproj");

        let mut project = ProjectFile::new_master();
        project.connections.push(ConnectionConfig {
            id: "conn-m1".into(),
            name: "Remote PLC".into(),
            transport: TransportConfig::Tcp {
                host: "192.168.1.10".into(),
                port: 502,
            },
            devices: vec![],
            scan_groups: vec![ScanGroupConfig {
                name: "Fast Poll".into(),
                slave_id: 1,
                function_code: 3,
                start_address: 0,
                count: 10,
                interval_ms: 1000,
            }],
        });

        save_project(&project, &path).unwrap();
        let loaded = load_project(&path).unwrap();

        assert_eq!(loaded.project_type, ProjectType::Master);
        assert_eq!(loaded.connections.len(), 1);

        let conn = &loaded.connections[0];
        assert_eq!(conn.id, "conn-m1");
        assert_eq!(conn.scan_groups.len(), 1);

        let sg = &conn.scan_groups[0];
        assert_eq!(sg.name, "Fast Poll");
        assert_eq!(sg.slave_id, 1);
        assert_eq!(sg.function_code, 3);
        assert_eq!(sg.start_address, 0);
        assert_eq!(sg.count, 10);
        assert_eq!(sg.interval_ms, 1000);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = load_project(Path::new("/tmp/does_not_exist_12345.modbusproj"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_invalid_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bad.modbusproj");
        std::fs::write(&path, "not json").unwrap();
        let result = load_project(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_migrate_current_version() {
        let data = r#"{"version":1,"type":"slave","connections":[]}"#;
        let project = migrate_project(data).unwrap();
        assert_eq!(project.version, 1);
        assert_eq!(project.project_type, ProjectType::Slave);
        assert!(project.connections.is_empty());
    }

    #[test]
    fn test_migrate_unknown_version() {
        let data = r#"{"version":99,"type":"slave","connections":[]}"#;
        let result = migrate_project(data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsupported project version"));
    }

    #[test]
    fn test_json_roundtrip_preserves_transport_tag() {
        let mut project = ProjectFile::new_slave();
        project.connections.push(ConnectionConfig {
            id: "c1".into(),
            name: "Test".into(),
            transport: TransportConfig::Tcp {
                host: "localhost".into(),
                port: 5020,
            },
            devices: vec![],
            scan_groups: vec![],
        });

        let json = serde_json::to_string(&project).unwrap();
        assert!(json.contains(r#""type":"tcp""#));

        let loaded: ProjectFile = serde_json::from_str(&json).unwrap();
        match &loaded.connections[0].transport {
            TransportConfig::Tcp { host, port } => {
                assert_eq!(host, "localhost");
                assert_eq!(*port, 5020);
            }
        }
    }
}
