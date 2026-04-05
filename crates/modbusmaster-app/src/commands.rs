//! Tauri commands for ModbusMaster.

use crate::state::{
    AppState, CachedPollData, ConnectionStateEvent, FoundRegisterDto, MasterConnectionInfo,
    MasterConnectionState, PollDataPayload, PollErrorPayload, ReadResultDto, RegisterScanEvent,
    RegisterValueDto, ScanGroupInfo, SlaveIdScanEvent,
};
use modbussim_core::log_collector::LogCollector;
use modbussim_core::log_entry::LogEntry;
use modbussim_core::log_helpers;
use modbussim_core::master::{
    scan_registers_with_ctx, scan_slave_ids_with_ctx, MasterConfig, MasterConnection, ReadFunction,
    ReadResult, ScanGroup,
};
use modbussim_core::parse::{parse_read_function, read_function_to_string};
use modbussim_core::tools;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{mpsc, oneshot};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_result_to_dto(
    scan_group_id: &str,
    function: ReadFunction,
    start_address: u16,
    result: &ReadResult,
    timestamp: &str,
) -> ReadResultDto {
    let values = match result {
        ReadResult::Coils(vals) => vals
            .iter()
            .enumerate()
            .map(|(i, v)| RegisterValueDto {
                address: start_address + i as u16,
                raw_value: if *v { 1 } else { 0 },
                display_value: if *v {
                    "ON".to_string()
                } else {
                    "OFF".to_string()
                },
                is_bool: true,
            })
            .collect(),
        ReadResult::DiscreteInputs(vals) => vals
            .iter()
            .enumerate()
            .map(|(i, v)| RegisterValueDto {
                address: start_address + i as u16,
                raw_value: if *v { 1 } else { 0 },
                display_value: if *v {
                    "ON".to_string()
                } else {
                    "OFF".to_string()
                },
                is_bool: true,
            })
            .collect(),
        ReadResult::HoldingRegisters(vals) => vals
            .iter()
            .enumerate()
            .map(|(i, v)| RegisterValueDto {
                address: start_address + i as u16,
                raw_value: *v as u64,
                display_value: v.to_string(),
                is_bool: false,
            })
            .collect(),
        ReadResult::InputRegisters(vals) => vals
            .iter()
            .enumerate()
            .map(|(i, v)| RegisterValueDto {
                address: start_address + i as u16,
                raw_value: *v as u64,
                display_value: v.to_string(),
                is_bool: false,
            })
            .collect(),
    };

    ReadResultDto {
        scan_group_id: scan_group_id.to_string(),
        function: read_function_to_string(function).to_string(),
        start_address,
        values,
        timestamp: timestamp.to_string(),
        error: None,
    }
}

fn now_timestamp() -> String {
    chrono_like_now()
}

fn chrono_like_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let millis = now.as_millis() % 1000;
    // Simple ISO-ish format
    format!("{}.{:03}", secs, millis)
}

// ---------------------------------------------------------------------------
// Connection Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateMasterRequest {
    pub target_address: String,
    pub port: u16,
    pub slave_id: u8,
    pub timeout_ms: Option<u64>,
}

#[tauri::command]
pub async fn create_master_connection(
    state: State<'_, AppState>,
    request: CreateMasterRequest,
) -> Result<MasterConnectionInfo, String> {
    let id = {
        let mut counter = state.next_conn_id.write().await;
        let id = format!("master_{}", *counter);
        *counter += 1;
        id
    };

    let config = MasterConfig {
        target_address: request.target_address,
        port: request.port,
        slave_id: request.slave_id,
        timeout_ms: request.timeout_ms.unwrap_or(3000),
    };

    let log_collector = Arc::new(LogCollector::new());
    let connection = MasterConnection::new(config.clone()).with_log_collector(log_collector.clone());

    let info = MasterConnectionInfo {
        id: id.clone(),
        target_address: config.target_address.clone(),
        port: config.port,
        slave_id: config.slave_id,
        state: format!("{:?}", connection.state()),
        scan_group_count: 0,
    };

    state.master_connections.write().await.insert(
        id,
        MasterConnectionState {
            connection,
            scan_groups: Vec::new(),
            log_collector,
            cached_data: std::collections::HashMap::new(),
        },
    );

    Ok(info)
}

#[tauri::command]
pub async fn connect_master(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let mut conns = state.master_connections.write().await;
    let conn_state = conns
        .get_mut(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    conn_state
        .connection
        .connect()
        .await
        .map_err(|e| format!("{}", e))?;

    let _ = app.emit(
        "master-connection-state",
        ConnectionStateEvent {
            id: connection_id,
            state: format!("{:?}", conn_state.connection.state()),
        },
    );

    Ok(())
}

#[tauri::command]
pub async fn disconnect_master(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let mut conns = state.master_connections.write().await;
    let conn_state = conns
        .get_mut(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    conn_state
        .connection
        .disconnect()
        .await
        .map_err(|e| format!("{}", e))?;

    let _ = app.emit(
        "master-connection-state",
        ConnectionStateEvent {
            id: connection_id,
            state: format!("{:?}", conn_state.connection.state()),
        },
    );

    Ok(())
}

#[tauri::command]
pub async fn delete_master_connection(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let mut conns = state.master_connections.write().await;
    let mut conn_state = conns
        .remove(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    // Ensure disconnected
    let _ = conn_state.connection.disconnect().await;
    Ok(())
}

#[tauri::command]
pub async fn list_master_connections(
    state: State<'_, AppState>,
) -> Result<Vec<MasterConnectionInfo>, String> {
    let conns = state.master_connections.read().await;
    let mut list: Vec<MasterConnectionInfo> = conns
        .iter()
        .map(|(id, cs)| MasterConnectionInfo {
            id: id.clone(),
            target_address: cs.connection.config.target_address.clone(),
            port: cs.connection.config.port,
            slave_id: cs.connection.config.slave_id,
            state: format!("{:?}", cs.connection.state()),
            scan_group_count: cs.scan_groups.len(),
        })
        .collect();
    list.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(list)
}

// ---------------------------------------------------------------------------
// Scan Group Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AddScanGroupRequest {
    pub name: String,
    pub function: String,
    pub start_address: u16,
    pub quantity: u16,
    pub interval_ms: u64,
    pub enabled: Option<bool>,
    pub slave_id: Option<u8>,
}

#[tauri::command]
pub async fn add_scan_group(
    state: State<'_, AppState>,
    connection_id: String,
    request: AddScanGroupRequest,
) -> Result<ScanGroupInfo, String> {
    let function = parse_read_function(&request.function)?;
    let group_id = uuid::Uuid::new_v4().to_string();

    let group = ScanGroup {
        id: group_id.clone(),
        name: request.name,
        function,
        start_address: request.start_address,
        quantity: request.quantity,
        interval_ms: request.interval_ms,
        enabled: request.enabled.unwrap_or(true),
        slave_id: request.slave_id,
    };

    let mut conns = state.master_connections.write().await;
    let conn_state = conns
        .get_mut(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    let info = ScanGroupInfo {
        id: group.id.clone(),
        name: group.name.clone(),
        function: read_function_to_string(group.function).to_string(),
        start_address: group.start_address,
        quantity: group.quantity,
        interval_ms: group.interval_ms,
        enabled: group.enabled,
        is_polling: false,
        slave_id: group.slave_id,
    };

    conn_state.scan_groups.push(group);
    Ok(info)
}

#[derive(Debug, Deserialize)]
pub struct UpdateScanGroupRequest {
    pub name: Option<String>,
    pub function: Option<String>,
    pub start_address: Option<u16>,
    pub quantity: Option<u16>,
    pub interval_ms: Option<u64>,
    pub enabled: Option<bool>,
}

#[tauri::command]
pub async fn update_scan_group(
    state: State<'_, AppState>,
    connection_id: String,
    group_id: String,
    request: UpdateScanGroupRequest,
) -> Result<ScanGroupInfo, String> {
    let mut conns = state.master_connections.write().await;
    let conn_state = conns
        .get_mut(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    let group = conn_state
        .scan_groups
        .iter_mut()
        .find(|g| g.id == group_id)
        .ok_or_else(|| format!("Scan group not found: {}", group_id))?;

    if let Some(name) = request.name {
        group.name = name;
    }
    if let Some(function) = request.function {
        group.function = parse_read_function(&function)?;
    }
    if let Some(addr) = request.start_address {
        group.start_address = addr;
    }
    if let Some(qty) = request.quantity {
        group.quantity = qty;
    }
    if let Some(ms) = request.interval_ms {
        group.interval_ms = ms;
    }
    if let Some(enabled) = request.enabled {
        group.enabled = enabled;
    }

    let is_polling = conn_state.connection.is_scan_active(&group_id);

    Ok(ScanGroupInfo {
        id: group.id.clone(),
        name: group.name.clone(),
        function: read_function_to_string(group.function).to_string(),
        start_address: group.start_address,
        quantity: group.quantity,
        interval_ms: group.interval_ms,
        enabled: group.enabled,
        is_polling,
        slave_id: group.slave_id,
    })
}

#[tauri::command]
pub async fn remove_scan_group(
    state: State<'_, AppState>,
    connection_id: String,
    group_id: String,
) -> Result<(), String> {
    let mut conns = state.master_connections.write().await;
    let conn_state = conns
        .get_mut(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    // Stop polling if active
    conn_state
        .connection
        .stop_scan_group(&group_id)
        .await
        .map_err(|e| format!("{}", e))?;

    conn_state.scan_groups.retain(|g| g.id != group_id);
    conn_state.cached_data.remove(&group_id);
    Ok(())
}

#[tauri::command]
pub async fn list_scan_groups(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<ScanGroupInfo>, String> {
    let conns = state.master_connections.read().await;
    let conn_state = conns
        .get(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    Ok(conn_state
        .scan_groups
        .iter()
        .map(|g| ScanGroupInfo {
            id: g.id.clone(),
            name: g.name.clone(),
            function: read_function_to_string(g.function).to_string(),
            start_address: g.start_address,
            quantity: g.quantity,
            interval_ms: g.interval_ms,
            enabled: g.enabled,
            is_polling: conn_state.connection.is_scan_active(&g.id),
            slave_id: g.slave_id,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Polling Commands
// ---------------------------------------------------------------------------

/// Internal helper: start polling for a single group and spawn bridge task.
async fn start_polling_inner(
    app: &AppHandle,
    master_conns: &std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, MasterConnectionState>>>,
    connection_id: &str,
    group_id: &str,
) -> Result<(), String> {
    let (mut rx, function, start_address) = {
        let mut conns = master_conns.write().await;
        let conn_state = conns
            .get_mut(connection_id)
            .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

        let group = conn_state
            .scan_groups
            .iter()
            .find(|g| g.id == group_id)
            .cloned()
            .ok_or_else(|| format!("Scan group not found: {}", group_id))?;

        let rx = conn_state
            .connection
            .start_scan_group(&group)
            .await
            .map_err(|e| format!("{}", e))?;

        (rx, group.function, group.start_address)
    };

    // Spawn a bridge task to forward poll events to Tauri events and cache data
    let app_handle = app.clone();
    let conn_id = connection_id.to_string();
    let sg_id = group_id.to_string();
    let cache_ref = master_conns.clone();

    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                modbussim_core::master::PollEvent::Data(result) => {
                    let ts = now_timestamp();
                    let dto = read_result_to_dto(&sg_id, function, start_address, &result, &ts);

                    // Update cache
                    {
                        let mut conns = cache_ref.write().await;
                        if let Some(cs) = conns.get_mut(&conn_id) {
                            cs.cached_data.insert(
                                sg_id.clone(),
                                CachedPollData {
                                    result: result.clone(),
                                    timestamp: ts.clone(),
                                },
                            );
                        }
                    }

                    let _ = app_handle.emit(
                        "master-poll-data",
                        PollDataPayload {
                            connection_id: conn_id.clone(),
                            scan_group_id: sg_id.clone(),
                            result: dto,
                        },
                    );
                }
                modbussim_core::master::PollEvent::Error(err) => {
                    let _ = app_handle.emit(
                        "master-poll-error",
                        PollErrorPayload {
                            connection_id: conn_id.clone(),
                            scan_group_id: sg_id.clone(),
                            error: err,
                        },
                    );
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn start_polling(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    group_id: String,
) -> Result<(), String> {
    let master_conns = state.master_connections.clone();
    start_polling_inner(&app, &master_conns, &connection_id, &group_id).await
}

#[tauri::command]
pub async fn stop_polling(
    state: State<'_, AppState>,
    connection_id: String,
    group_id: String,
) -> Result<(), String> {
    let mut conns = state.master_connections.write().await;
    let conn_state = conns
        .get_mut(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    conn_state
        .connection
        .stop_scan_group(&group_id)
        .await
        .map_err(|e| format!("{}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn start_all_polling(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let master_conns = state.master_connections.clone();

    // Collect enabled group IDs
    let group_ids: Vec<String> = {
        let conns = master_conns.read().await;
        let conn_state = conns
            .get(&connection_id)
            .ok_or_else(|| format!("Connection not found: {}", connection_id))?;
        conn_state
            .scan_groups
            .iter()
            .filter(|g| g.enabled)
            .map(|g| g.id.clone())
            .collect()
    };

    for group_id in group_ids {
        start_polling_inner(&app, &master_conns, &connection_id, &group_id).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn stop_all_polling(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let mut conns = state.master_connections.write().await;
    let conn_state = conns
        .get_mut(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    conn_state.connection.stop_all_scans().await;
    Ok(())
}

#[tauri::command]
pub async fn get_poll_data(
    state: State<'_, AppState>,
    connection_id: String,
    group_id: String,
) -> Result<Option<ReadResultDto>, String> {
    let conns = state.master_connections.read().await;
    let conn_state = conns
        .get(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    let group = conn_state
        .scan_groups
        .iter()
        .find(|g| g.id == group_id)
        .ok_or_else(|| format!("Scan group not found: {}", group_id))?;

    if let Some(cached) = conn_state.cached_data.get(&group_id) {
        Ok(Some(read_result_to_dto(
            &group_id,
            group.function,
            group.start_address,
            &cached.result,
            &cached.timestamp,
        )))
    } else {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Read/Write Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ReadOnceRequest {
    pub function: String,
    pub start_address: u16,
    pub quantity: u16,
}

#[tauri::command]
pub async fn read_once(
    state: State<'_, AppState>,
    connection_id: String,
    request: ReadOnceRequest,
) -> Result<ReadResultDto, String> {
    let function = parse_read_function(&request.function)?;
    let mut conns = state.master_connections.write().await;
    let conn_state = conns
        .get_mut(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    let result = conn_state
        .connection
        .read(function, request.start_address, request.quantity)
        .await
        .map_err(|e| format!("{}", e))?;

    let ts = now_timestamp();
    Ok(read_result_to_dto(
        "__read_once__",
        function,
        request.start_address,
        &result,
        &ts,
    ))
}

#[derive(Debug, Deserialize)]
pub struct WriteSingleRegRequest {
    pub address: u16,
    pub value: u16,
}

#[tauri::command]
pub async fn write_single_register(
    state: State<'_, AppState>,
    connection_id: String,
    request: WriteSingleRegRequest,
) -> Result<(), String> {
    let mut conns = state.master_connections.write().await;
    let conn_state = conns
        .get_mut(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    conn_state
        .connection
        .write_single_register(request.address, request.value)
        .await
        .map_err(|e| format!("{}", e))
}

#[derive(Debug, Deserialize)]
pub struct WriteSingleCoilRequest {
    pub address: u16,
    pub value: bool,
}

#[tauri::command]
pub async fn write_single_coil(
    state: State<'_, AppState>,
    connection_id: String,
    request: WriteSingleCoilRequest,
) -> Result<(), String> {
    let mut conns = state.master_connections.write().await;
    let conn_state = conns
        .get_mut(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    conn_state
        .connection
        .write_single_coil(request.address, request.value)
        .await
        .map_err(|e| format!("{}", e))
}

#[derive(Debug, Deserialize)]
pub struct WriteMultiRegsRequest {
    pub address: u16,
    pub values: Vec<u16>,
}

#[tauri::command]
pub async fn write_multiple_registers(
    state: State<'_, AppState>,
    connection_id: String,
    request: WriteMultiRegsRequest,
) -> Result<(), String> {
    let mut conns = state.master_connections.write().await;
    let conn_state = conns
        .get_mut(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    conn_state
        .connection
        .write_multiple_registers(request.address, &request.values)
        .await
        .map_err(|e| format!("{}", e))
}

#[derive(Debug, Deserialize)]
pub struct WriteMultiCoilsRequest {
    pub address: u16,
    pub values: Vec<bool>,
}

#[tauri::command]
pub async fn write_multiple_coils(
    state: State<'_, AppState>,
    connection_id: String,
    request: WriteMultiCoilsRequest,
) -> Result<(), String> {
    let mut conns = state.master_connections.write().await;
    let conn_state = conns
        .get_mut(&connection_id)
        .ok_or_else(|| format!("Connection not found: {}", connection_id))?;

    conn_state
        .connection
        .write_multiple_coils(request.address, &request.values)
        .await
        .map_err(|e| format!("{}", e))
}

// ---------------------------------------------------------------------------
// Log Commands
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tool Commands (stateless, reuse core logic)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PlcToModbusRequest {
    pub plc_address: u32,
}

#[derive(Debug, Serialize)]
pub struct PlcToModbusResult {
    pub register_type: String,
    pub modbus_address: u16,
}

#[tauri::command]
pub fn convert_plc_to_modbus(request: PlcToModbusRequest) -> Result<PlcToModbusResult, String> {
    let result =
        tools::plc_to_modbus_address(request.plc_address).map_err(|e| format!("{}", e))?;
    Ok(PlcToModbusResult {
        register_type: format!("{:?}", result.address_type),
        modbus_address: result.address,
    })
}

#[derive(Debug, Deserialize)]
pub struct ModbusToPlcRequest {
    pub register_type: String,
    pub modbus_address: u16,
}

#[tauri::command]
pub fn convert_modbus_to_plc(request: ModbusToPlcRequest) -> Result<u32, String> {
    let addr_type = match request.register_type.as_str() {
        "Coil" | "coil" => tools::ModbusAddressType::Coil,
        "DiscreteInput" | "discrete_input" => tools::ModbusAddressType::DiscreteInput,
        "InputRegister" | "input_register" => tools::ModbusAddressType::InputRegister,
        "HoldingRegister" | "holding_register" => tools::ModbusAddressType::HoldingRegister,
        _ => return Err(format!("Unknown register type: {}", request.register_type)),
    };
    Ok(tools::modbus_to_plc_address(
        request.modbus_address,
        addr_type,
    ))
}

#[tauri::command]
pub fn calculate_crc16(data: Vec<u8>) -> u16 {
    tools::crc16(&data)
}

#[tauri::command]
pub fn calculate_lrc(data: Vec<u8>) -> u8 {
    tools::lrc(&data)
}

// ---------------------------------------------------------------------------
// Scan Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SlaveIdScanRequest {
    pub start_id: Option<u8>,
    pub end_id: Option<u8>,
    pub timeout_ms: Option<u64>,
}

#[tauri::command]
pub async fn start_slave_id_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    request: SlaveIdScanRequest,
) -> Result<(), String> {
    let timeout_ms = request.timeout_ms.unwrap_or(500);
    let start_id = request.start_id.unwrap_or(1);
    let end_id = request.end_id.unwrap_or(247);

    // Extract ctx handle and original slave_id (short lock)
    let (ctx_handle, original_slave_id) = {
        let conns = state.master_connections.read().await;
        let cs = conns
            .get(&connection_id)
            .ok_or_else(|| format!("connection {} not found", connection_id))?;
        let ctx = cs.connection.get_ctx_handle().map_err(|e| e.to_string())?;
        (ctx, cs.connection.config.slave_id)
    };

    // Create cancel channel
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let scan_key = format!("{}:slave_scan", connection_id);
    state.active_scans.write().await.insert(scan_key.clone(), cancel_tx);

    let (progress_tx, mut progress_rx) = mpsc::channel(32);
    let conn_id = connection_id.clone();
    let active_scans = state.active_scans.clone();
    let scan_key_clone = scan_key.clone();

    // Spawn scan task
    tokio::spawn(async move {
        scan_slave_ids_with_ctx(
            ctx_handle,
            original_slave_id,
            start_id,
            end_id,
            Duration::from_millis(timeout_ms),
            cancel_rx,
            progress_tx,
        )
        .await;
    });

    // Spawn bridge task to forward progress as Tauri events
    tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let _ = app.emit(
                "scan-slave-progress",
                SlaveIdScanEvent {
                    connection_id: conn_id.clone(),
                    current_id: progress.current_id,
                    total: progress.total,
                    found_ids: progress.found_ids,
                    done: progress.done,
                    cancelled: progress.cancelled,
                },
            );
            if progress.done || progress.cancelled {
                break;
            }
        }
        // Cleanup
        active_scans.write().await.remove(&scan_key_clone);
    });

    Ok(())
}


#[derive(Debug, Deserialize)]
pub struct RegisterScanRequest {
    pub function: String,
    pub start_address: u16,
    pub end_address: u16,
    pub chunk_size: Option<u16>,
    pub timeout_ms: Option<u64>,
}

#[tauri::command]
pub async fn start_register_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    request: RegisterScanRequest,
) -> Result<(), String> {
    let function = parse_read_function(&request.function)?;
    let chunk_size = request.chunk_size.unwrap_or(10);
    let timeout_ms = request.timeout_ms.unwrap_or(1000);

    // Extract ctx handle (short lock)
    let ctx_handle = {
        let conns = state.master_connections.read().await;
        let cs = conns
            .get(&connection_id)
            .ok_or_else(|| format!("connection {} not found", connection_id))?;
        cs.connection.get_ctx_handle().map_err(|e| e.to_string())?
    };

    // Create cancel channel
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let scan_key = format!("{}:register_scan", connection_id);
    state.active_scans.write().await.insert(scan_key.clone(), cancel_tx);

    let (progress_tx, mut progress_rx) = mpsc::channel(32);
    let conn_id = connection_id.clone();
    let active_scans = state.active_scans.clone();
    let scan_key_clone = scan_key.clone();
    let end_address = request.end_address;

    // Spawn scan task
    tokio::spawn(async move {
        scan_registers_with_ctx(
            ctx_handle,
            function,
            request.start_address,
            end_address,
            chunk_size,
            Duration::from_millis(timeout_ms),
            cancel_rx,
            progress_tx,
        )
        .await;
    });

    // Spawn bridge task
    tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let _ = app.emit(
                "scan-register-progress",
                RegisterScanEvent {
                    connection_id: conn_id.clone(),
                    current_address: progress.current_address,
                    end_address: progress.end_address,
                    found_count: progress.found_registers.len() as u16,
                    found_registers: progress
                        .found_registers
                        .iter()
                        .map(|r| FoundRegisterDto {
                            address: r.address,
                            value: r.value,
                        })
                        .collect(),
                    done: progress.done,
                    cancelled: progress.cancelled,
                },
            );
            if progress.done || progress.cancelled {
                break;
            }
        }
        active_scans.write().await.remove(&scan_key_clone);
    });

    Ok(())
}

#[tauri::command]
pub async fn cancel_scan(
    state: State<'_, AppState>,
    connection_id: String,
    scan_type: String,
) -> Result<(), String> {
    let scan_key = format!("{}:{}", connection_id, scan_type);
    let sender = state.active_scans.write().await.remove(&scan_key);
    if let Some(tx) = sender {
        let _ = tx.send(());
    }
    Ok(())
}

#[tauri::command]
pub fn parse_hex(hex_string: String) -> Result<Vec<u8>, String> {
    tools::parse_hex_string(&hex_string).map_err(|e| format!("{}", e))
}
