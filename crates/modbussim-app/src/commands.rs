//! Tauri commands for ModbusSim.
//!
//! These commands are invoked from the frontend via the Tauri IPC bridge.

use crate::state::{
    AppState, RegisterValueInfo, SlaveConnectionInfo, SlaveConnectionState, SlaveDeviceInfo,
};
use modbussim_core::log_collector::LogCollector;
use modbussim_core::log_entry::LogEntry;
use modbussim_core::log_helpers;
use modbussim_core::parse::{parse_data_type, parse_endian, parse_register_type};
use modbussim_core::register::{Endian, RegisterDef, RegisterType};
use modbussim_core::project::{self, ProjectFile};
use modbussim_core::slave::{SlaveConnection, SlaveDevice};
use modbussim_core::transport::Transport;
use modbussim_core::tools;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

// ---------------------------------------------------------------------------
// Event Payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SlaveConnectionEvent {
    pub id: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RegisterValueEvent {
    pub connection_id: String,
    pub slave_id: u8,
    pub register_type: String,
    pub address: u16,
    pub value: u16,
}

// ---------------------------------------------------------------------------
// Slave Connection Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateSlaveRequest {
    pub bind_address: Option<String>,
    pub port: u16,
    pub init_mode: Option<String>,
}

#[tauri::command]
pub async fn create_slave_connection(
    state: State<'_, AppState>,
    request: CreateSlaveRequest,
) -> Result<SlaveConnectionInfo, String> {
    let id = {
        let mut counter = state.next_slave_id.write().await;
        let id = format!("slave_{}", *counter);
        *counter += 1;
        id
    };

    let host = request.bind_address.unwrap_or_else(|| "0.0.0.0".to_string());
    let port = request.port;
    let transport = Transport::Tcp {
        host: host.clone(),
        port,
    };

    let log_collector = Arc::new(LogCollector::new());
    let connection = SlaveConnection::new(transport);
    let connection = connection.with_log_collector(log_collector.clone());

    // Auto-create default slave device (slave_id=1) with pre-filled registers
    let default_device = match request.init_mode.as_deref() {
        Some("random") => SlaveDevice::with_random_registers(1, "从站 1", 100),
        _ => SlaveDevice::with_default_registers(1, "从站 1", 100),
    };
    connection
        .add_device(default_device)
        .await
        .map_err(|e| format!("failed to add default device: {}", e))?;

    let info = SlaveConnectionInfo {
        id: id.clone(),
        bind_address: host,
        port,
        state: format!("{:?}", connection.state()),
        device_count: 1,
    };

    state.slave_connections.write().await.insert(
        id,
        SlaveConnectionState {
            connection,
            log_collector,
        },
    );

    Ok(info)
}

#[tauri::command]
pub async fn start_slave_connection(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    let state_str: String;
    {
        let mut connections = state.slave_connections.write().await;
        let conn = connections
            .get_mut(&id)
            .ok_or_else(|| format!("connection {} not found", id))?;

        conn.connection
            .start()
            .await
            .map_err(|e| format!("failed to start: {}", e))?;
        state_str = format!("{:?}", conn.connection.state());
    }

    let event = SlaveConnectionEvent {
        id: id.clone(),
        state: state_str,
    };
    app_handle.emit("slave-connection-state", event).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn stop_slave_connection(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    id: String,
) -> Result<(), String> {
    let state_str: String;
    {
        let mut connections = state.slave_connections.write().await;
        let conn = connections
            .get_mut(&id)
            .ok_or_else(|| format!("connection {} not found", id))?;

        conn.connection
            .stop()
            .await
            .map_err(|e| format!("failed to stop: {}", e))?;
        state_str = format!("{:?}", conn.connection.state());
    }

    let event = SlaveConnectionEvent {
        id: id.clone(),
        state: state_str,
    };
    app_handle.emit("slave-connection-state", event).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn delete_slave_connection(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let mut connections = state.slave_connections.write().await;
    connections
        .remove(&id)
        .ok_or_else(|| format!("connection {} not found", id))?;
    Ok(())
}

#[tauri::command]
pub async fn list_slave_connections(
    state: State<'_, AppState>,
) -> Result<Vec<SlaveConnectionInfo>, String> {
    let connections = state.slave_connections.read().await;
    let mut result = Vec::new();

    for (id, conn_state) in connections.iter() {
        let device_count = conn_state.connection.devices.read().await.len();
        let (bind_address, port) = match &conn_state.connection.transport {
            Transport::Tcp { host, port } | Transport::RtuOverTcp { host, port } => {
                (host.clone(), *port)
            }
            Transport::Rtu(sc) | Transport::Ascii(sc) => (sc.port.clone(), 0),
        };
        result.push(SlaveConnectionInfo {
            id: id.clone(),
            bind_address,
            port,
            state: format!("{:?}", conn_state.connection.state()),
            device_count,
        });
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Slave Device Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AddSlaveDeviceRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub name: String,
    pub init_mode: Option<String>,
}

#[tauri::command]
pub async fn add_slave_device(
    state: State<'_, AppState>,
    request: AddSlaveDeviceRequest,
) -> Result<SlaveDeviceInfo, String> {
    let mut connections = state.slave_connections.write().await;
    let conn = connections
        .get_mut(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;

    let name = request.name.clone();
    let device = match request.init_mode.as_deref() {
        Some("random") => SlaveDevice::with_random_registers(request.slave_id, name.clone(), 100),
        Some("zero") => SlaveDevice::with_default_registers(request.slave_id, name.clone(), 100),
        _ => SlaveDevice::new(request.slave_id, name.clone()),
    };
    let register_count = device.register_defs.len();
    conn.connection
        .add_device(device)
        .await
        .map_err(|e| format!("failed to add device: {}", e))?;

    Ok(SlaveDeviceInfo {
        slave_id: request.slave_id,
        name,
        register_count,
    })
}

#[tauri::command]
pub async fn remove_slave_device(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
) -> Result<(), String> {
    let mut connections = state.slave_connections.write().await;
    let conn = connections
        .get_mut(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    conn.connection
        .remove_device(slave_id)
        .await
        .map_err(|e| format!("failed to remove device: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn list_slave_devices(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<SlaveDeviceInfo>, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    let devices = conn.connection.devices.read().await;
    let result: Vec<SlaveDeviceInfo> = devices
        .values()
        .map(|d| SlaveDeviceInfo {
            slave_id: d.slave_id,
            name: d.name.clone(),
            register_count: d.register_defs.len(),
        })
        .collect();

    Ok(result)
}

// ---------------------------------------------------------------------------
// Register Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AddRegisterRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub address: u16,
    pub register_type: String,
    pub data_type: String,
    pub endian: Option<String>,
    pub name: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WriteRegisterRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub register_type: String,
    pub address: u16,
    pub value: u16,
}


#[tauri::command]
pub async fn add_register(
    state: State<'_, AppState>,
    request: AddRegisterRequest,
) -> Result<(), String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;

    let register_type = parse_register_type(&request.register_type)?;
    let data_type = parse_data_type(&request.data_type)?;
    let endian = match &request.endian {
        Some(s) => parse_endian(s)?,
        None => Endian::Big,
    };

    let def = RegisterDef {
        address: request.address,
        register_type,
        data_type,
        endian,
        name: request.name.unwrap_or_default(),
        comment: request.comment.unwrap_or_default(),
    };

    let mut devices = conn.connection.devices.write().await;
    let device = devices
        .get_mut(&request.slave_id)
        .ok_or_else(|| format!("slave {} not found", request.slave_id))?;

    device.register_defs.push(def);
    Ok(())
}

#[tauri::command]
pub async fn remove_register(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
    address: u16,
    register_type: String,
) -> Result<(), String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    let reg_type = parse_register_type(&register_type)?;

    let mut devices = conn.connection.devices.write().await;
    let device = devices
        .get_mut(&slave_id)
        .ok_or_else(|| format!("slave {} not found", slave_id))?;

    device.register_defs.retain(|d| !(d.address == address && d.register_type == reg_type));
    Ok(())
}

#[tauri::command]
pub async fn read_register(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
    register_type: String,
    address: u16,
) -> Result<RegisterValueInfo, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    let reg_type = parse_register_type(&register_type)?;

    let devices = conn.connection.devices.read().await;
    let device = devices
        .get(&slave_id)
        .ok_or_else(|| format!("slave {} not found", slave_id))?;

    let value = match reg_type {
        RegisterType::Coil => device.register_map.coils.get(&address).copied().unwrap_or(false) as u16,
        RegisterType::DiscreteInput => device.register_map.discrete_inputs.get(&address).copied().unwrap_or(false) as u16,
        RegisterType::HoldingRegister => device.register_map.holding_registers.get(&address).copied().unwrap_or(0),
        RegisterType::InputRegister => device.register_map.input_registers.get(&address).copied().unwrap_or(0),
    };

    Ok(RegisterValueInfo { address, value })
}

#[tauri::command]
pub async fn write_register(
    state: State<'_, AppState>,
    app_handle: AppHandle,
    request: WriteRegisterRequest,
) -> Result<(), String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;

    let reg_type = parse_register_type(&request.register_type)?;

    let mut devices = conn.connection.devices.write().await;
    let device = devices
        .get_mut(&request.slave_id)
        .ok_or_else(|| format!("slave {} not found", request.slave_id))?;

    match reg_type {
        RegisterType::Coil => device.register_map.write_coil(request.address, request.value != 0),
        RegisterType::DiscreteInput => { device.register_map.discrete_inputs.insert(request.address, request.value != 0); },
        RegisterType::HoldingRegister => device.register_map.write_holding_register(request.address, request.value),
        RegisterType::InputRegister => { device.register_map.input_registers.insert(request.address, request.value); },
    }

    let event = RegisterValueEvent {
        connection_id: request.connection_id,
        slave_id: request.slave_id,
        register_type: request.register_type,
        address: request.address,
        value: request.value,
    };
    app_handle.emit("register-value-changed", event).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn list_registers(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
) -> Result<Vec<RegisterDef>, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    let devices = conn.connection.devices.read().await;
    let device = devices
        .get(&slave_id)
        .ok_or_else(|| format!("slave {} not found", slave_id))?;

    Ok(device.register_defs.clone())
}

#[tauri::command]
pub async fn export_registers(
    state: State<'_, AppState>,
    connection_id: String,
    slave_id: u8,
) -> Result<String, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&connection_id)
        .ok_or_else(|| format!("connection {} not found", connection_id))?;

    let devices = conn.connection.devices.read().await;
    let device = devices
        .get(&slave_id)
        .ok_or_else(|| format!("slave {} not found", slave_id))?;

    serde_json::to_string_pretty(&device.register_defs)
        .map_err(|e| format!("failed to serialize: {}", e))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ImportRegistersRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub registers: Vec<RegisterDef>,
}

#[tauri::command]
pub async fn import_registers(
    state: State<'_, AppState>,
    request: ImportRegistersRequest,
) -> Result<usize, String> {
    let connections = state.slave_connections.write().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;

    let mut devices = conn.connection.devices.write().await;
    let device = devices
        .get_mut(&request.slave_id)
        .ok_or_else(|| format!("slave {} not found", request.slave_id))?;

    let count = request.registers.len();
    for reg in request.registers {
        // Validate register type and data type
        let _ = parse_register_type(&format!("{:?}", reg.register_type).to_lowercase())?;
        let _ = parse_data_type(&format!("{:?}", reg.data_type).to_lowercase())?;
        device.register_defs.push(reg);
    }

    Ok(count)
}

// ---------------------------------------------------------------------------
// Log Commands
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tool Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AddressConversionRequest {
    pub address: u32,
    pub address_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AddressConversionResult {
    pub plc_address: u32,
    pub protocol_address: u16,
    pub register_type: String,
}

#[tauri::command]
pub fn convert_plc_to_modbus(request: AddressConversionRequest) -> Result<AddressConversionResult, String> {
    let addr = tools::plc_to_modbus_address(request.address)
        .map_err(|e| format!("{}", e))?;

    Ok(AddressConversionResult {
        plc_address: request.address,
        protocol_address: addr.address,
        register_type: format!("{:?}", addr.address_type).to_lowercase(),
    })
}

#[tauri::command]
pub fn convert_modbus_to_plc(address: u16, register_type: String) -> Result<u32, String> {
    let reg_type = match register_type.as_str() {
        "coil" => tools::ModbusAddressType::Coil,
        "discrete_input" => tools::ModbusAddressType::DiscreteInput,
        "input_register" => tools::ModbusAddressType::InputRegister,
        "holding_register" => tools::ModbusAddressType::HoldingRegister,
        _ => return Err(format!("unknown register type: {}", register_type)),
    };

    Ok(tools::modbus_to_plc_address(address, reg_type))
}

#[tauri::command]
pub fn calculate_crc16(data: String) -> Result<String, String> {
    let bytes = tools::parse_hex_string(&data)
        .map_err(|e| format!("{}", e))?;
    let crc = tools::crc16(&bytes);
    Ok(format!("{:04X}", crc))
}

#[tauri::command]
pub fn calculate_lrc(data: String) -> Result<String, String> {
    let bytes = tools::parse_hex_string(&data)
        .map_err(|e| format!("{}", e))?;
    let lrc = tools::lrc(&bytes);
    Ok(format!("{:02X}", lrc))
}

#[tauri::command]
pub fn parse_hex(data: String) -> Result<Vec<u8>, String> {
    tools::parse_hex_string(&data)
        .map_err(|e| format!("{}", e))
}

// ---------------------------------------------------------------------------
// State Persistence Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PersistedSlaveConnection {
    pub bind_address: String,
    pub port: u16,
    pub devices: Vec<PersistedDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PersistedDevice {
    pub slave_id: u8,
    pub name: String,
    pub registers: Vec<RegisterDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PersistedAppState {
    pub version: u32,
    pub slave_connections: Vec<PersistedSlaveConnection>,
}

#[tauri::command]
pub async fn export_app_state(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let connections = state.slave_connections.read().await;

    let mut persisted_connections = Vec::new();

    for (_id, conn_state) in connections.iter() {
        let devices = conn_state.connection.devices.read().await;
        let mut persisted_devices = Vec::new();

        for (_slave_id, device) in devices.iter() {
            persisted_devices.push(PersistedDevice {
                slave_id: device.slave_id,
                name: device.name.clone(),
                registers: device.register_defs.clone(),
            });
        }

        let (bind_address, port) = match &conn_state.connection.transport {
            Transport::Tcp { host, port } | Transport::RtuOverTcp { host, port } => {
                (host.clone(), *port)
            }
            Transport::Rtu(sc) | Transport::Ascii(sc) => (sc.port.clone(), 0),
        };
        persisted_connections.push(PersistedSlaveConnection {
            bind_address,
            port,
            devices: persisted_devices,
        });
    }

    let app_state = PersistedAppState {
        version: 1,
        slave_connections: persisted_connections,
    };

    serde_json::to_string_pretty(&app_state)
        .map_err(|e| format!("failed to serialize: {}", e))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PersistedAppStateInput {
    pub version: u32,
    pub slave_connections: Vec<PersistedSlaveConnection>,
}

#[tauri::command]
pub async fn import_app_state(
    state: State<'_, AppState>,
    input: PersistedAppStateInput,
) -> Result<usize, String> {
    if input.version != 1 {
        return Err(format!("unsupported state version: {}", input.version));
    }

    let mut total_devices = 0;

    for conn_input in input.slave_connections {
        let id = {
            let mut counter = state.next_slave_id.write().await;
            let id = format!("slave_{}", *counter);
            *counter += 1;
            id
        };

        let transport = Transport::Tcp {
            host: conn_input.bind_address.clone(),
            port: conn_input.port,
        };

        let log_collector = Arc::new(LogCollector::new());
        let connection = SlaveConnection::new(transport);
        let connection = connection.with_log_collector(log_collector.clone());

        // Add devices
        for device_input in conn_input.devices {
            let mut device = SlaveDevice::new(device_input.slave_id, device_input.name.clone());

            // Add registers
            for reg in device_input.registers {
                device.register_defs.push(reg);
            }

            let _ = connection.add_device(device).await;
            total_devices += 1;
        }

        state.slave_connections.write().await.insert(
            id,
            SlaveConnectionState {
                connection,
                log_collector,
            },
        );
    }

    Ok(total_devices)
}

#[tauri::command]
pub async fn clear_app_state(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.slave_connections.write().await.clear();
    *state.next_slave_id.write().await = 0;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RandomMutateRequest {
    pub connection_id: String,
    pub slave_id: u8,
    pub register_types: Vec<String>,
}

#[tauri::command]
pub async fn random_mutate_registers(
    state: State<'_, AppState>,
    request: RandomMutateRequest,
) -> Result<u32, String> {
    let connections = state.slave_connections.read().await;
    let conn = connections
        .get(&request.connection_id)
        .ok_or_else(|| format!("connection {} not found", request.connection_id))?;

    let mut devices = conn.connection.devices.write().await;
    let device = devices
        .get_mut(&request.slave_id)
        .ok_or_else(|| format!("slave {} not found", request.slave_id))?;

    let mut rng = rand::rng();
    let mut mutated = 0u32;

    for rt_str in &request.register_types {
        let reg_type = parse_register_type(rt_str)?;
        let addrs: Vec<u16> = device.register_defs.iter()
            .filter(|d| d.register_type == reg_type)
            .map(|d| d.address)
            .collect();
        if addrs.is_empty() { continue; }

        // Mutate ~30% of registers of this type, at least 3
        let count = (addrs.len() * 30 / 100).max(3).min(addrs.len());
        // Shuffle and take first `count`
        let mut pick = addrs.clone();
        for i in (1..pick.len()).rev() {
            let j = rng.random_range(0..=i);
            pick.swap(i, j);
        }
        for &addr in &pick[..count] {
            match reg_type {
                RegisterType::Coil => {
                    let cur = device.register_map.coils.get(&addr).copied().unwrap_or(false);
                    device.register_map.write_coil(addr, !cur);
                }
                RegisterType::DiscreteInput => {
                    let cur = device.register_map.discrete_inputs.get(&addr).copied().unwrap_or(false);
                    device.register_map.discrete_inputs.insert(addr, !cur);
                }
                RegisterType::HoldingRegister => {
                    let cur = device.register_map.holding_registers.get(&addr).copied().unwrap_or(0);
                    let delta: i32 = rng.random_range(-100..=100);
                    let new_val = (cur as i32 + delta).clamp(0, 65535) as u16;
                    device.register_map.write_holding_register(addr, new_val);
                }
                RegisterType::InputRegister => {
                    let cur = device.register_map.input_registers.get(&addr).copied().unwrap_or(0);
                    let delta: i32 = rng.random_range(-100..=100);
                    let new_val = (cur as i32 + delta).clamp(0, 65535) as u16;
                    device.register_map.input_registers.insert(addr, new_val);
                }
            }
            mutated += 1;
        }
    }

    Ok(mutated)
}

// ---------------------------------------------------------------------------
// Project File Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn save_project_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let connections = state.slave_connections.read().await;
    let mut proj = ProjectFile::new_slave();

    for (id, conn_state) in connections.iter() {
        let conn = &conn_state.connection;
        let (name, proj_transport) = match &conn.transport {
            Transport::Tcp { host, port } => (
                format!("{}:{}", host, port),
                project::TransportConfig::Tcp {
                    host: host.clone(),
                    port: *port,
                },
            ),
            Transport::RtuOverTcp { host, port } => (
                format!("rtu-tcp://{}:{}", host, port),
                project::TransportConfig::RtuOverTcp {
                    host: host.clone(),
                    port: *port,
                },
            ),
            Transport::Rtu(sc) => (
                format!("rtu://{}", sc.port),
                project::TransportConfig::Rtu {
                    port: sc.port.clone(),
                    baud_rate: sc.baud_rate,
                    data_bits: sc.data_bits,
                    stop_bits: sc.stop_bits,
                    parity: format!("{:?}", sc.parity).to_lowercase(),
                },
            ),
            Transport::Ascii(sc) => (
                format!("ascii://{}", sc.port),
                project::TransportConfig::Ascii {
                    port: sc.port.clone(),
                    baud_rate: sc.baud_rate,
                    data_bits: sc.data_bits,
                    stop_bits: sc.stop_bits,
                    parity: format!("{:?}", sc.parity).to_lowercase(),
                },
            ),
        };
        let conn_config = project::ConnectionConfig {
            id: id.clone(),
            name,
            transport: proj_transport,
            devices: vec![],  // Simplified for Phase 1 - register serialization will be added later
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
