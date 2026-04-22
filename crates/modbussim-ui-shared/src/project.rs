//! Minimal project-file schema v2 for the egui edition.
//!
//! Not compatible with the legacy Tauri `.modbusproj` (v1) — users were
//! informed during the refactor brainstorm that back-compat is not required.
//! Same extension for UX familiarity; the `schema_version` field disambiguates.

use serde::{Deserialize, Serialize};

pub const EGUI_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EguiProjectType {
    Slave,
    Master,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpSpec {
    pub host: String,
    pub port: u16,
    /// 可选 TLS 配置；旧项目文件无此字段时按 None（明文 TCP）解析。
    #[serde(default)]
    pub tls: Option<TlsSpec>,
}

/// 子站 TLS 持久化字段，与 `modbussim_core::transport::SlaveTlsConfig`
/// 字段一一对应。任何字段缺失视为空字符串/false，便于演进。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TlsSpec {
    #[serde(default)]
    pub cert_file: String,
    #[serde(default)]
    pub key_file: String,
    #[serde(default)]
    pub ca_file: String,
    #[serde(default)]
    pub require_client_cert: bool,
    #[serde(default)]
    pub pkcs12_file: String,
    #[serde(default)]
    pub pkcs12_password: String,
}

// --- Slave ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveDeviceSave {
    pub slave_id: u8,
    pub name: String,
    /// Max populated address when the device was initialized (for rebuilding
    /// default registers on load). `None` = device was created empty.
    pub max_address: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveConnectionSave {
    pub label: String,
    pub tcp: TcpSpec,
    #[serde(default)]
    pub devices: Vec<SlaveDeviceSave>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveProject {
    pub schema_version: u32,
    #[serde(rename = "type")]
    pub project_type: EguiProjectType,
    #[serde(default)]
    pub connections: Vec<SlaveConnectionSave>,
}

impl SlaveProject {
    pub fn new() -> Self {
        Self {
            schema_version: EGUI_SCHEMA_VERSION,
            project_type: EguiProjectType::Slave,
            connections: Vec::new(),
        }
    }
}

impl Default for SlaveProject {
    fn default() -> Self {
        Self::new()
    }
}

// --- Master ---

/// Saved polling configuration (maps to MasterApp::PollUi).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollSave {
    /// `read_coils` / `read_discrete_inputs` / `read_holding_registers` / `read_input_registers`
    pub function: String,
    pub addr: u16,
    pub qty: u16,
    pub interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterConnectionSave {
    pub label: String,
    pub tcp: TcpSpec,
    pub slave_id: u8,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub poll: Option<PollSave>,
}

fn default_timeout() -> u64 {
    3000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterProject {
    pub schema_version: u32,
    #[serde(rename = "type")]
    pub project_type: EguiProjectType,
    #[serde(default)]
    pub connections: Vec<MasterConnectionSave>,
}

impl MasterProject {
    pub fn new() -> Self {
        Self {
            schema_version: EGUI_SCHEMA_VERSION,
            project_type: EguiProjectType::Master,
            connections: Vec::new(),
        }
    }
}

impl Default for MasterProject {
    fn default() -> Self {
        Self::new()
    }
}

pub fn serialize_slave(p: &SlaveProject) -> Result<String, String> {
    serde_json::to_string_pretty(p).map_err(|e| e.to_string())
}

pub fn deserialize_slave(s: &str) -> Result<SlaveProject, String> {
    let p: SlaveProject = serde_json::from_str(s).map_err(|e| e.to_string())?;
    if p.project_type != EguiProjectType::Slave {
        return Err("项目文件不是 Slave 类型".to_string());
    }
    if p.schema_version != EGUI_SCHEMA_VERSION {
        return Err(format!(
            "不支持的 schema_version: {}（期望 {}）",
            p.schema_version, EGUI_SCHEMA_VERSION
        ));
    }
    Ok(p)
}

pub fn serialize_master(p: &MasterProject) -> Result<String, String> {
    serde_json::to_string_pretty(p).map_err(|e| e.to_string())
}

pub fn deserialize_master(s: &str) -> Result<MasterProject, String> {
    let p: MasterProject = serde_json::from_str(s).map_err(|e| e.to_string())?;
    if p.project_type != EguiProjectType::Master {
        return Err("项目文件不是 Master 类型".to_string());
    }
    if p.schema_version != EGUI_SCHEMA_VERSION {
        return Err(format!(
            "不支持的 schema_version: {}（期望 {}）",
            p.schema_version, EGUI_SCHEMA_VERSION
        ));
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slave_roundtrip() {
        let mut p = SlaveProject::new();
        p.connections.push(SlaveConnectionSave {
            label: "TCP 0.0.0.0:502".into(),
            tcp: TcpSpec {
                host: "0.0.0.0".into(),
                port: 502,
                tls: None,
            },
            devices: vec![SlaveDeviceSave {
                slave_id: 1,
                name: "从站 1".into(),
                max_address: Some(20000),
            }],
        });
        let json = serialize_slave(&p).unwrap();
        let q = deserialize_slave(&json).unwrap();
        assert_eq!(q.connections.len(), 1);
        assert_eq!(q.connections[0].devices[0].slave_id, 1);
    }

    #[test]
    fn master_roundtrip() {
        let mut p = MasterProject::new();
        p.connections.push(MasterConnectionSave {
            label: "Remote".into(),
            tcp: TcpSpec {
                host: "127.0.0.1".into(),
                port: 5502,
                tls: None,
            },
            slave_id: 1,
            timeout_ms: 3000,
            poll: Some(PollSave {
                function: "read_holding_registers".into(),
                addr: 0,
                qty: 10,
                interval_ms: 500,
            }),
        });
        let json = serialize_master(&p).unwrap();
        let q = deserialize_master(&json).unwrap();
        assert_eq!(q.connections[0].poll.as_ref().unwrap().qty, 10);
    }

    #[test]
    fn slave_tls_roundtrip_and_legacy_compat() {
        let mut p = SlaveProject::new();
        p.connections.push(SlaveConnectionSave {
            label: "TLS 0.0.0.0:8502".into(),
            tcp: TcpSpec {
                host: "0.0.0.0".into(),
                port: 8502,
                tls: Some(TlsSpec {
                    cert_file: "/etc/cert.pem".into(),
                    key_file: "/etc/key.pem".into(),
                    ca_file: String::new(),
                    require_client_cert: false,
                    pkcs12_file: String::new(),
                    pkcs12_password: String::new(),
                }),
            },
            devices: Vec::new(),
        });
        let json = serialize_slave(&p).unwrap();
        let q = deserialize_slave(&json).unwrap();
        let tls = q.connections[0].tcp.tls.clone();
        assert_eq!(tls.unwrap().cert_file, "/etc/cert.pem");

        // 旧文件（无 tls 字段）应当照常解析、tls=None
        let legacy = r#"{
            "schema_version": 2,
            "type": "slave",
            "connections": [
                {"label":"L","tcp":{"host":"0.0.0.0","port":502},"devices":[]}
            ]
        }"#;
        let parsed = deserialize_slave(legacy).unwrap();
        assert!(parsed.connections[0].tcp.tls.is_none());
    }

    #[test]
    fn wrong_type_rejected() {
        let master = MasterProject::new();
        let json = serialize_master(&master).unwrap();
        assert!(deserialize_slave(&json).is_err());
    }
}
