use crate::log_collector::LogCollector;
use crate::log_entry::{Direction, FunctionCode, LogEntry};
use crate::register::{RegisterDef, RegisterMap};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, RwLock};
use tokio_modbus::server::tcp::{accept_tcp_connection, Server};
use tokio_modbus::server::Service;
use tokio_modbus::{ExceptionCode, Request, Response, SlaveRequest};

/// A single Modbus slave device with its own register map and definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveDevice {
    pub slave_id: u8,
    pub name: String,
    pub register_map: RegisterMap,
    pub register_defs: Vec<RegisterDef>,
}

impl SlaveDevice {
    pub fn new(slave_id: u8, name: impl Into<String>) -> Self {
        Self {
            slave_id,
            name: name.into(),
            register_map: RegisterMap::new(),
            register_defs: Vec::new(),
        }
    }

    /// Create a device with default registers pre-filled.
    /// Adds FC1/FC2/FC3/FC4 registers for addresses 0..=max_address,
    /// and initializes the corresponding RegisterMap values.
    pub fn with_default_registers(slave_id: u8, name: impl Into<String>, max_address: u16) -> Self {
        use crate::register::{DataType, Endian, RegisterDef, RegisterType};

        let mut device = Self::new(slave_id, name);
        let mut defs = Vec::with_capacity((max_address as usize + 1) * 4);

        for addr in 0..=max_address {
            // FC1 Coil
            defs.push(RegisterDef {
                address: addr,
                register_type: RegisterType::Coil,
                data_type: DataType::Bool,
                endian: Endian::Big,
                name: String::new(),
                comment: String::new(),
            });
            device.register_map.write_coil(addr, false);

            // FC2 Discrete Input
            defs.push(RegisterDef {
                address: addr,
                register_type: RegisterType::DiscreteInput,
                data_type: DataType::Bool,
                endian: Endian::Big,
                name: String::new(),
                comment: String::new(),
            });
            device.register_map.discrete_inputs.insert(addr, false);

            // FC3 Holding Register
            defs.push(RegisterDef {
                address: addr,
                register_type: RegisterType::HoldingRegister,
                data_type: DataType::UInt16,
                endian: Endian::Big,
                name: String::new(),
                comment: String::new(),
            });
            device.register_map.write_holding_register(addr, 0);

            // FC4 Input Register
            defs.push(RegisterDef {
                address: addr,
                register_type: RegisterType::InputRegister,
                data_type: DataType::UInt16,
                endian: Endian::Big,
                name: String::new(),
                comment: String::new(),
            });
            device.register_map.input_registers.insert(addr, 0);
        }

        device.register_defs = defs;
        device
    }

    /// Create a device with random register values pre-filled.
    /// Same structure as `with_default_registers` but values are randomized:
    /// - Coil/DiscreteInput: random bool
    /// - HoldingRegister/InputRegister: random u16
    pub fn with_random_registers(slave_id: u8, name: impl Into<String>, max_address: u16) -> Self {
        use crate::register::{DataType, Endian, RegisterDef, RegisterType};
        use rand::Rng;

        let mut device = Self::new(slave_id, name);
        let mut defs = Vec::with_capacity((max_address as usize + 1) * 4);
        let mut rng = rand::thread_rng();

        for addr in 0..=max_address {
            // FC1 Coil
            defs.push(RegisterDef {
                address: addr,
                register_type: RegisterType::Coil,
                data_type: DataType::Bool,
                endian: Endian::Big,
                name: String::new(),
                comment: String::new(),
            });
            device.register_map.write_coil(addr, rng.gen::<bool>());

            // FC2 Discrete Input
            defs.push(RegisterDef {
                address: addr,
                register_type: RegisterType::DiscreteInput,
                data_type: DataType::Bool,
                endian: Endian::Big,
                name: String::new(),
                comment: String::new(),
            });
            device.register_map.discrete_inputs.insert(addr, rng.gen::<bool>());

            // FC3 Holding Register
            defs.push(RegisterDef {
                address: addr,
                register_type: RegisterType::HoldingRegister,
                data_type: DataType::UInt16,
                endian: Endian::Big,
                name: String::new(),
                comment: String::new(),
            });
            device.register_map.write_holding_register(addr, rng.gen::<u16>());

            // FC4 Input Register
            defs.push(RegisterDef {
                address: addr,
                register_type: RegisterType::InputRegister,
                data_type: DataType::UInt16,
                endian: Endian::Big,
                name: String::new(),
                comment: String::new(),
            });
            device.register_map.input_registers.insert(addr, rng.gen::<u16>());
        }

        device.register_defs = defs;
        device
    }
}

/// Running state of a slave connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Stopped,
    Running,
}

/// Transport configuration for a slave connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    pub bind_address: String,
    pub port: u16,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 502,
        }
    }
}

/// Shared state accessible by all connections on a SlaveConnection.
pub type SharedDevices = Arc<RwLock<HashMap<u8, SlaveDevice>>>;

/// Shared log collector for all connections on a SlaveConnection.
pub type SharedLogCollector = Option<Arc<LogCollector>>;

/// A slave connection manages multiple SlaveDevices on a single TCP listener.
pub struct SlaveConnection {
    pub transport: TransportConfig,
    pub devices: SharedDevices,
    pub log_collector: SharedLogCollector,
    state: ConnectionState,
    shutdown_tx: Option<oneshot::Sender<()>>,
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

impl SlaveConnection {
    pub fn new(transport: TransportConfig) -> Self {
        Self {
            transport,
            devices: Arc::new(RwLock::new(HashMap::new())),
            log_collector: None,
            state: ConnectionState::Stopped,
            shutdown_tx: None,
            server_handle: None,
        }
    }

    /// Set the log collector for this connection.
    pub fn with_log_collector(mut self, collector: Arc<LogCollector>) -> Self {
        self.log_collector = Some(collector);
        self
    }

    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Add a slave device. Returns error if the slave_id already exists.
    pub async fn add_device(&self, device: SlaveDevice) -> Result<(), SlaveError> {
        let mut devices = self.devices.write().await;
        if devices.contains_key(&device.slave_id) {
            return Err(SlaveError::DuplicateSlaveId(device.slave_id));
        }
        devices.insert(device.slave_id, device);
        Ok(())
    }

    /// Remove a slave device by ID. Returns error if not found.
    pub async fn remove_device(&self, slave_id: u8) -> Result<SlaveDevice, SlaveError> {
        let mut devices = self.devices.write().await;
        devices
            .remove(&slave_id)
            .ok_or(SlaveError::SlaveNotFound(slave_id))
    }

    /// Start the TCP server. Returns error if already running or bind fails.
    pub async fn start(&mut self) -> Result<(), SlaveError> {
        if self.state == ConnectionState::Running {
            return Err(SlaveError::AlreadyRunning);
        }

        let addr: SocketAddr = format!("{}:{}", self.transport.bind_address, self.transport.port)
            .parse()
            .map_err(|e| SlaveError::BindError(format!("Invalid address: {e}")))?;

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| SlaveError::BindError(format!("Failed to bind {addr}: {e}")))?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let devices = self.devices.clone();
        let log_collector = self.log_collector.clone();

        let handle = tokio::spawn(async move {
            let server = Server::new(listener);
            let on_connected = {
                let devices = devices.clone();
                let log_collector = log_collector.clone();
                move |stream, socket_addr| {
                    let devices = devices.clone();
                    let log_collector = log_collector.clone();
                    let new_service =
                        move |_socket_addr| Ok(Some(SlaveService::new(devices.clone(), log_collector.clone())));
                    async move { accept_tcp_connection(stream, socket_addr, new_service) }
                }
            };
            let on_process_error = |err| {
                log::error!("Slave server process error: {err}");
            };
            let abort_signal = Box::pin(async {
                let _ = shutdown_rx.await;
            });
            let _ = server
                .serve_until(&on_connected, on_process_error, abort_signal)
                .await;
        });

        self.shutdown_tx = Some(shutdown_tx);
        self.server_handle = Some(handle);
        self.state = ConnectionState::Running;
        Ok(())
    }

    /// Stop the TCP server gracefully.
    pub async fn stop(&mut self) -> Result<(), SlaveError> {
        if self.state == ConnectionState::Stopped {
            return Err(SlaveError::NotRunning);
        }

        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.server_handle.take() {
            let _ = handle.await;
        }
        self.state = ConnectionState::Stopped;
        Ok(())
    }
}

/// The Modbus service that handles requests for a slave connection.
/// Shared across all client connections via the `new_service` closure.
struct SlaveService {
    devices: SharedDevices,
    log_collector: SharedLogCollector,
}

impl SlaveService {
    fn new(devices: SharedDevices, log_collector: SharedLogCollector) -> Self {
        Self { devices, log_collector }
    }

    fn get_function_code(request: &Request<'_>) -> Option<FunctionCode> {
        match request {
            Request::ReadCoils(..) => Some(FunctionCode::ReadCoils),
            Request::ReadDiscreteInputs(..) => Some(FunctionCode::ReadDiscreteInputs),
            Request::ReadHoldingRegisters(..) => Some(FunctionCode::ReadHoldingRegisters),
            Request::ReadInputRegisters(..) => Some(FunctionCode::ReadInputRegisters),
            Request::WriteSingleCoil(..) => Some(FunctionCode::WriteSingleCoil),
            Request::WriteSingleRegister(..) => Some(FunctionCode::WriteSingleRegister),
            Request::WriteMultipleCoils(..) => Some(FunctionCode::WriteMultipleCoils),
            Request::WriteMultipleRegisters(..) => Some(FunctionCode::WriteMultipleRegisters),
            _ => None,
        }
    }

    fn format_request_detail(request: &Request<'_>) -> String {
        match request {
            Request::ReadCoils(addr, qty) => format!("R {} x{}", addr, qty),
            Request::ReadDiscreteInputs(addr, qty) => format!("R {} x{}", addr, qty),
            Request::ReadHoldingRegisters(addr, qty) => format!("R {} x{}", addr, qty),
            Request::ReadInputRegisters(addr, qty) => format!("R {} x{}", addr, qty),
            Request::WriteSingleCoil(addr, val) => format!("W {} = {}", addr, val),
            Request::WriteSingleRegister(addr, val) => format!("W {} = {:#06x}", addr, val),
            Request::WriteMultipleCoils(addr, vals) => format!("W {} x{}", addr, vals.len()),
            Request::WriteMultipleRegisters(addr, vals) => format!("W {} x{}", addr, vals.len()),
            _ => "?".to_string(),
        }
    }

    fn log_if_enabled(&self, direction: Direction, fc: FunctionCode, detail: &str) {
        if let Some(collector) = &self.log_collector {
            let entry = LogEntry::new(direction, fc, detail);
            collector.add_blocking(entry);
        }
    }
}

impl Service for SlaveService {
    type Request = SlaveRequest<'static>;
    type Response = Option<Response>;
    type Exception = ExceptionCode;
    type Future = future::Ready<Result<Option<Response>, ExceptionCode>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let SlaveRequest { slave, request } = req;
        let devices = self.devices.clone();

        // Log inbound request
        if let Some(fc) = Self::get_function_code(&request) {
            let detail = Self::format_request_detail(&request);
            self.log_if_enabled(Direction::Rx, fc, &detail);
        }

        let is_write = matches!(
            request,
            Request::WriteSingleCoil(..)
                | Request::WriteSingleRegister(..)
                | Request::WriteMultipleCoils(..)
                | Request::WriteMultipleRegisters(..)
        );

        let result = if is_write {
            match devices.try_write() {
                Ok(mut devices) => match devices.get_mut(&slave) {
                    Some(device) => Some(handle_write(&mut device.register_map, request)),
                    None => None,
                },
                Err(_) => Some(Err(ExceptionCode::ServerDeviceBusy)),
            }
        } else {
            match devices.try_read() {
                Ok(devices) => match devices.get(&slave) {
                    Some(device) => Some(handle_read(&device.register_map, request)),
                    None => None,
                },
                Err(_) => Some(Err(ExceptionCode::ServerDeviceBusy)),
            }
        };

        match result {
            Some(Ok(response)) => future::ready(Ok(Some(response))),
            Some(Err(exception)) => future::ready(Err(exception)),
            None => future::ready(Ok(None)), // Unknown slave ID: silent drop
        }
    }
}

/// Handle read-only Modbus requests.
fn handle_read(
    register_map: &RegisterMap,
    request: Request<'static>,
) -> Result<Response, ExceptionCode> {
    match request {
        // FC01: Read Coils
        Request::ReadCoils(addr, quantity) => {
            Ok(Response::ReadCoils(register_map.read_coils(addr, quantity)))
        }
        // FC02: Read Discrete Inputs
        Request::ReadDiscreteInputs(addr, quantity) => Ok(Response::ReadDiscreteInputs(
            register_map.read_discrete_inputs(addr, quantity),
        )),
        // FC03: Read Holding Registers
        Request::ReadHoldingRegisters(addr, quantity) => Ok(Response::ReadHoldingRegisters(
            register_map.read_holding_registers(addr, quantity),
        )),
        // FC04: Read Input Registers
        Request::ReadInputRegisters(addr, quantity) => Ok(Response::ReadInputRegisters(
            register_map.read_input_registers(addr, quantity),
        )),
        // Unsupported function codes
        _ => Err(ExceptionCode::IllegalFunction),
    }
}

/// Handle write Modbus requests (requires mutable access to register map).
fn handle_write(
    register_map: &mut RegisterMap,
    request: Request<'static>,
) -> Result<Response, ExceptionCode> {
    match request {
        // FC05: Write Single Coil
        Request::WriteSingleCoil(addr, value) => {
            register_map.write_coil(addr, value);
            Ok(Response::WriteSingleCoil(addr, value))
        }
        // FC06: Write Single Register
        Request::WriteSingleRegister(addr, value) => {
            register_map.write_holding_register(addr, value);
            Ok(Response::WriteSingleRegister(addr, value))
        }
        // FC15: Write Multiple Coils
        Request::WriteMultipleCoils(addr, values) => {
            let quantity = values.len() as u16;
            register_map.write_coils(addr, &values);
            Ok(Response::WriteMultipleCoils(addr, quantity))
        }
        // FC16: Write Multiple Registers
        Request::WriteMultipleRegisters(addr, values) => {
            let quantity = values.len() as u16;
            register_map.write_holding_registers(addr, &values);
            Ok(Response::WriteMultipleRegisters(addr, quantity))
        }
        _ => Err(ExceptionCode::IllegalFunction),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SlaveError {
    #[error("slave ID {0} already exists")]
    DuplicateSlaveId(u8),
    #[error("slave ID {0} not found")]
    SlaveNotFound(u8),
    #[error("server is already running")]
    AlreadyRunning,
    #[error("server is not running")]
    NotRunning,
    #[error("bind error: {0}")]
    BindError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slave_device_creation() {
        let device = SlaveDevice::new(1, "Test Device");
        assert_eq!(device.slave_id, 1);
        assert_eq!(device.name, "Test Device");
        assert!(device.register_map.holding_registers.is_empty());
        assert!(device.register_defs.is_empty());
    }

    #[test]
    fn test_handle_read_holding_registers() {
        let mut map = RegisterMap::new();
        map.write_holding_register(0, 1234);
        map.write_holding_register(1, 5678);

        let response = handle_read(&map, Request::ReadHoldingRegisters(0, 2)).unwrap();
        match response {
            Response::ReadHoldingRegisters(values) => {
                assert_eq!(values, vec![1234, 5678]);
            }
            _ => panic!("unexpected response"),
        }
    }

    #[test]
    fn test_handle_read_coils() {
        let mut map = RegisterMap::new();
        map.write_coil(0, true);
        map.write_coil(1, false);
        map.write_coil(2, true);

        let response = handle_read(&map, Request::ReadCoils(0, 3)).unwrap();
        match response {
            Response::ReadCoils(values) => {
                assert_eq!(values, vec![true, false, true]);
            }
            _ => panic!("unexpected response"),
        }
    }

    #[test]
    fn test_handle_read_discrete_inputs() {
        let mut map = RegisterMap::new();
        map.discrete_inputs.insert(0, true);
        map.discrete_inputs.insert(1, false);

        let response = handle_read(&map, Request::ReadDiscreteInputs(0, 2)).unwrap();
        match response {
            Response::ReadDiscreteInputs(values) => {
                assert_eq!(values, vec![true, false]);
            }
            _ => panic!("unexpected response"),
        }
    }

    #[test]
    fn test_handle_read_input_registers() {
        let mut map = RegisterMap::new();
        map.input_registers.insert(0, 100);
        map.input_registers.insert(1, 200);

        let response = handle_read(&map, Request::ReadInputRegisters(0, 2)).unwrap();
        match response {
            Response::ReadInputRegisters(values) => {
                assert_eq!(values, vec![100, 200]);
            }
            _ => panic!("unexpected response"),
        }
    }

    #[test]
    fn test_handle_write_single_coil() {
        let mut map = RegisterMap::new();
        let response = handle_write(&mut map, Request::WriteSingleCoil(5, true)).unwrap();
        assert!(matches!(response, Response::WriteSingleCoil(5, true)));
        assert_eq!(map.read_coils(5, 1), vec![true]);
    }

    #[test]
    fn test_handle_write_single_register() {
        let mut map = RegisterMap::new();
        let response =
            handle_write(&mut map, Request::WriteSingleRegister(10, 0xABCD)).unwrap();
        assert!(matches!(response, Response::WriteSingleRegister(10, 0xABCD)));
        assert_eq!(map.read_holding_registers(10, 1), vec![0xABCD]);
    }

    #[test]
    fn test_handle_write_multiple_coils() {
        let mut map = RegisterMap::new();
        let values = vec![true, false, true];
        let response = handle_write(
            &mut map,
            Request::WriteMultipleCoils(0, std::borrow::Cow::Owned(values)),
        )
        .unwrap();
        assert!(matches!(response, Response::WriteMultipleCoils(0, 3)));
        assert_eq!(map.read_coils(0, 3), vec![true, false, true]);
    }

    #[test]
    fn test_handle_write_multiple_registers() {
        let mut map = RegisterMap::new();
        let values = vec![100, 200, 300];
        let response = handle_write(
            &mut map,
            Request::WriteMultipleRegisters(0, std::borrow::Cow::Owned(values)),
        )
        .unwrap();
        assert!(matches!(response, Response::WriteMultipleRegisters(0, 3)));
        assert_eq!(map.read_holding_registers(0, 3), vec![100, 200, 300]);
    }

    #[test]
    fn test_handle_unsupported_function() {
        let map = RegisterMap::new();
        let result = handle_read(&map, Request::ReportServerId);
        assert_eq!(result.unwrap_err(), ExceptionCode::IllegalFunction);
    }

    #[tokio::test]
    async fn test_slave_connection_add_remove_device() {
        let conn = SlaveConnection::new(TransportConfig::default());
        let device = SlaveDevice::new(1, "Test");
        conn.add_device(device).await.unwrap();

        // Duplicate should fail
        let dup = SlaveDevice::new(1, "Dup");
        assert!(conn.add_device(dup).await.is_err());

        // Remove should work
        let removed = conn.remove_device(1).await.unwrap();
        assert_eq!(removed.slave_id, 1);

        // Remove again should fail
        assert!(conn.remove_device(1).await.is_err());
    }

    #[test]
    fn test_with_default_registers() {
        let device = SlaveDevice::with_default_registers(1, "从站 1", 100);
        assert_eq!(device.slave_id, 1);
        assert_eq!(device.name, "从站 1");

        // 4 types x 101 addresses = 404 register defs
        assert_eq!(device.register_defs.len(), 404);

        // Verify register map values initialized
        assert_eq!(device.register_map.coils.len(), 101);
        assert_eq!(device.register_map.discrete_inputs.len(), 101);
        assert_eq!(device.register_map.holding_registers.len(), 101);
        assert_eq!(device.register_map.input_registers.len(), 101);

        // All coils should be false
        for addr in 0..=100u16 {
            assert_eq!(device.register_map.coils.get(&addr), Some(&false));
        }

        // All holding registers should be 0
        for addr in 0..=100u16 {
            assert_eq!(device.register_map.holding_registers.get(&addr), Some(&0));
        }
    }

    #[tokio::test]
    async fn test_connection_with_default_device() {
        let conn = SlaveConnection::new(TransportConfig::default());
        let device = SlaveDevice::with_default_registers(1, "从站 1", 100);
        conn.add_device(device).await.unwrap();

        let devices = conn.devices.read().await;
        assert_eq!(devices.len(), 1);

        let dev = devices.get(&1).unwrap();
        assert_eq!(dev.register_defs.len(), 404);

        // Verify FC03 ReadHoldingRegisters works with default values
        let values = dev.register_map.read_holding_registers(0, 10);
        assert_eq!(values, vec![0; 10]);
    }

    #[test]
    fn test_with_random_registers() {
        let device = SlaveDevice::with_random_registers(1, "随机从站", 100);
        assert_eq!(device.slave_id, 1);
        assert_eq!(device.name, "随机从站");

        // 4 types x 101 addresses = 404 register defs
        assert_eq!(device.register_defs.len(), 404);

        // Verify register map values initialized
        assert_eq!(device.register_map.coils.len(), 101);
        assert_eq!(device.register_map.discrete_inputs.len(), 101);
        assert_eq!(device.register_map.holding_registers.len(), 101);
        assert_eq!(device.register_map.input_registers.len(), 101);

        // At least some values should be non-zero/true (statistically near-certain with 101 entries)
        let has_true_coil = (0..=100u16).any(|addr| *device.register_map.coils.get(&addr).unwrap());
        let has_nonzero_hr = (0..=100u16).any(|addr| *device.register_map.holding_registers.get(&addr).unwrap() != 0);
        assert!(has_true_coil, "expected at least one true coil with random init");
        assert!(has_nonzero_hr, "expected at least one non-zero holding register with random init");
    }
}
