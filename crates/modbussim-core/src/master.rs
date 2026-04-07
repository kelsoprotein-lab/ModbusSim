use crate::ascii_master::AsciiMasterTransport;
use crate::log_collector::LogCollector;
use crate::log_entry::{Direction, FunctionCode, LogEntry};
use crate::reconnect::ReconnectPolicy;
use crate::rtu_master::RtuMasterTransport;
use crate::rtu_tcp_master::RtuTcpMasterTransport;
use crate::transport::Transport;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_modbus::prelude::*;
use tokio_modbus::ExceptionCode;

/// Configuration for a master connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterConfig {
    pub target_address: String,
    pub port: u16,
    pub slave_id: u8,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    3000
}

impl Default for MasterConfig {
    fn default() -> Self {
        Self {
            target_address: "127.0.0.1".to_string(),
            port: 502,
            slave_id: 1,
            timeout_ms: default_timeout_ms(),
        }
    }
}

/// Connection state of the master.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MasterState {
    Disconnected,
    Connected,
    Reconnecting,
    Error,
}

/// Which read function code to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadFunction {
    /// FC01
    ReadCoils,
    /// FC02
    ReadDiscreteInputs,
    /// FC03
    ReadHoldingRegisters,
    /// FC04
    ReadInputRegisters,
}

/// Result of a read operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ReadResult {
    Coils(Vec<bool>),
    DiscreteInputs(Vec<bool>),
    HoldingRegisters(Vec<u16>),
    InputRegisters(Vec<u16>),
}

/// Events emitted by the polling task.
#[derive(Debug, Clone)]
pub enum PollEvent {
    Data(ReadResult),
    Error(String),
}

/// Configuration for a polling task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollConfig {
    pub function: ReadFunction,
    pub start_address: u16,
    pub quantity: u16,
    pub interval_ms: u64,
}

/// A named group of registers to scan periodically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanGroup {
    pub id: String,
    pub name: String,
    pub function: ReadFunction,
    pub start_address: u16,
    pub quantity: u16,
    pub interval_ms: u64,
    pub enabled: bool,
    /// Optional slave ID override. If None, uses the connection's default slave_id.
    #[serde(default)]
    pub slave_id: Option<u8>,
}

/// Handle for a running poll task.
struct PollTaskHandle {
    shutdown_tx: oneshot::Sender<()>,
    join_handle: tokio::task::JoinHandle<()>,
}

/// Active transport context — holds the connected transport handle.
#[derive(Clone)]
enum TransportCtx {
    Tcp(Arc<Mutex<client::Context>>),
    Rtu(Arc<RtuMasterTransport>),
    Ascii(Arc<AsciiMasterTransport>),
    RtuTcp(Arc<RtuTcpMasterTransport>),
}

/// A master connection that connects to a Modbus slave via any transport.
pub struct MasterConnection {
    pub config: MasterConfig,
    pub transport: Transport,
    pub reconnect_policy: ReconnectPolicy,
    reconnect_handle: Option<tokio::task::JoinHandle<()>>,
    state: MasterState,
    transport_ctx: Option<TransportCtx>,
    poll_tasks: HashMap<String, PollTaskHandle>,
    log_collector: Option<Arc<LogCollector>>,
}

impl MasterConnection {
    pub fn new(config: MasterConfig, transport: Transport) -> Self {
        Self {
            config,
            transport,
            reconnect_policy: ReconnectPolicy::default(),
            reconnect_handle: None,
            state: MasterState::Disconnected,
            transport_ctx: None,
            poll_tasks: HashMap::new(),
            log_collector: None,
        }
    }

    /// Set the log collector for this connection.
    pub fn with_log_collector(mut self, collector: Arc<LogCollector>) -> Self {
        self.log_collector = Some(collector);
        self
    }

    pub fn state(&self) -> MasterState {
        self.state
    }

    /// Connect to the target Modbus slave using the configured transport.
    pub async fn connect(&mut self) -> Result<(), MasterError> {
        if self.state == MasterState::Connected {
            return Err(MasterError::AlreadyConnected);
        }

        let timeout = Duration::from_millis(self.config.timeout_ms);

        let ctx = match &self.transport {
            Transport::Tcp { host, port } => {
                let addr: SocketAddr = format!("{}:{}", host, port)
                    .parse()
                    .map_err(|e| MasterError::ConnectionFailed(format!("Invalid address: {e}")))?;
                let tcp_ctx = tokio::time::timeout(
                    timeout,
                    tcp::connect_slave(addr, Slave(self.config.slave_id)),
                )
                .await
                .map_err(|_| MasterError::Timeout("Connection timed out".to_string()))?
                .map_err(|e| MasterError::ConnectionFailed(format!("{e}")))?;
                TransportCtx::Tcp(Arc::new(Mutex::new(tcp_ctx)))
            }
            Transport::Rtu(serial_config) => {
                let rtu = RtuMasterTransport::connect(serial_config)
                    .await
                    .map_err(|e| MasterError::ConnectionFailed(e))?;
                TransportCtx::Rtu(Arc::new(rtu))
            }
            Transport::Ascii(serial_config) => {
                let ascii = AsciiMasterTransport::connect(serial_config)
                    .await
                    .map_err(|e| MasterError::ConnectionFailed(e))?;
                TransportCtx::Ascii(Arc::new(ascii))
            }
            Transport::RtuOverTcp { host, port } => {
                let rtu_tcp = RtuTcpMasterTransport::connect(host, *port, timeout)
                    .await
                    .map_err(|e| MasterError::ConnectionFailed(e))?;
                TransportCtx::RtuTcp(Arc::new(rtu_tcp))
            }
            Transport::TcpTls { .. } => {
                return Err(MasterError::ConnectionFailed("TLS not yet implemented".to_string()));
            }
        };

        self.transport_ctx = Some(ctx);
        self.state = MasterState::Connected;
        Ok(())
    }

    /// Disconnect from the target.
    pub async fn disconnect(&mut self) -> Result<(), MasterError> {
        if self.state == MasterState::Disconnected {
            return Err(MasterError::NotConnected);
        }

        // Stop all polling first
        self.stop_all_scans().await;

        if let Some(ctx) = self.transport_ctx.take() {
            if let TransportCtx::Tcp(tcp_ctx) = ctx {
                let mut ctx = tcp_ctx.lock().await;
                let _ = ctx.disconnect().await;
            }
            // RTU/ASCII/RtuTcp: transport is dropped, which closes the port/stream.
        }
        self.state = MasterState::Disconnected;
        Ok(())
    }

    /// Reconnect: disconnect then connect again.
    pub async fn reconnect(&mut self) -> Result<(), MasterError> {
        let _ = self.disconnect().await;
        self.connect().await
    }

    fn get_transport_ctx(&self) -> Result<TransportCtx, MasterError> {
        self.transport_ctx.clone().ok_or(MasterError::NotConnected)
    }

    fn get_tcp_ctx(&self) -> Result<Arc<Mutex<client::Context>>, MasterError> {
        match &self.transport_ctx {
            Some(TransportCtx::Tcp(ctx)) => Ok(ctx.clone()),
            Some(_) => Err(MasterError::Transport("operation requires TCP transport".into())),
            None => Err(MasterError::NotConnected),
        }
    }

    /// Get a handle to the TCP context for external use (e.g., scanning).
    /// Returns an error for non-TCP transports.
    pub fn get_ctx_handle(&self) -> Result<Arc<Mutex<client::Context>>, MasterError> {
        self.get_tcp_ctx()
    }

    fn timeout_duration(&self) -> Duration {
        Duration::from_millis(self.config.timeout_ms)
    }

    fn to_function_code(function: ReadFunction) -> FunctionCode {
        match function {
            ReadFunction::ReadCoils => FunctionCode::ReadCoils,
            ReadFunction::ReadDiscreteInputs => FunctionCode::ReadDiscreteInputs,
            ReadFunction::ReadHoldingRegisters => FunctionCode::ReadHoldingRegisters,
            ReadFunction::ReadInputRegisters => FunctionCode::ReadInputRegisters,
        }
    }

    async fn log_tx(&self, fc: FunctionCode, detail: &str) {
        if let Some(collector) = &self.log_collector {
            let entry = LogEntry::new(Direction::Tx, fc, detail);
            collector.add(entry).await;
        }
    }

    async fn log_rx(&self, fc: FunctionCode, detail: &str) {
        if let Some(collector) = &self.log_collector {
            let entry = LogEntry::new(Direction::Rx, fc, detail);
            collector.add(entry).await;
        }
    }

    /// Read registers using the specified function code.
    pub async fn read(
        &self,
        function: ReadFunction,
        start_address: u16,
        quantity: u16,
    ) -> Result<ReadResult, MasterError> {
        let transport_ctx = self.get_transport_ctx()?;
        let timeout = self.timeout_duration();
        let fc = Self::to_function_code(function);

        // Log TX
        self.log_tx(fc, &format!("R {} x{}", start_address, quantity)).await;

        let result = execute_read_any(
            &transport_ctx,
            self.config.slave_id,
            function,
            start_address,
            quantity,
            timeout,
        )
        .await?;

        // Log RX with result data
        let detail = match &result {
            ReadResult::Coils(vals) => format!("{:?}", vals),
            ReadResult::DiscreteInputs(vals) => format!("{:?}", vals),
            ReadResult::HoldingRegisters(vals) => format!("{:?}", vals),
            ReadResult::InputRegisters(vals) => format!("{:?}", vals),
        };
        self.log_rx(fc, &detail).await;

        Ok(result)
    }

    /// Write a single coil (FC05).
    pub async fn write_single_coil(
        &self,
        address: u16,
        value: bool,
    ) -> Result<(), MasterError> {
        let transport_ctx = self.get_transport_ctx()?;
        let timeout = self.timeout_duration();
        self.log_tx(FunctionCode::WriteSingleCoil, &format!("W {} = {}", address, value)).await;
        match &transport_ctx {
            TransportCtx::Tcp(ctx) => {
                let mut ctx = ctx.lock().await;
                tokio::time::timeout(timeout, ctx.write_single_coil(address, value))
                    .await
                    .map_err(|_| MasterError::Timeout("Write single coil timed out".into()))?
                    .map_err(|e| MasterError::Transport(format!("{e}")))?
                    .map_err(|e| MasterError::Exception(e))?;
            }
            other => {
                let coil_value: u16 = if value { 0xFF00 } else { 0x0000 };
                let mut pdu = vec![0x05];
                pdu.extend_from_slice(&address.to_be_bytes());
                pdu.extend_from_slice(&coil_value.to_be_bytes());
                let resp = send_pdu_via_transport(other, self.config.slave_id, &pdu, timeout).await?;
                check_write_response(&resp, 0x05)?;
            }
        }
        Ok(())
    }

    /// Write a single holding register (FC06).
    pub async fn write_single_register(
        &self,
        address: u16,
        value: u16,
    ) -> Result<(), MasterError> {
        let transport_ctx = self.get_transport_ctx()?;
        let timeout = self.timeout_duration();
        self.log_tx(FunctionCode::WriteSingleRegister, &format!("W {} = {:#06x}", address, value)).await;
        match &transport_ctx {
            TransportCtx::Tcp(ctx) => {
                let mut ctx = ctx.lock().await;
                tokio::time::timeout(timeout, ctx.write_single_register(address, value))
                    .await
                    .map_err(|_| MasterError::Timeout("Write single register timed out".into()))?
                    .map_err(|e| MasterError::Transport(format!("{e}")))?
                    .map_err(|e| MasterError::Exception(e))?;
            }
            other => {
                let mut pdu = vec![0x06];
                pdu.extend_from_slice(&address.to_be_bytes());
                pdu.extend_from_slice(&value.to_be_bytes());
                let resp = send_pdu_via_transport(other, self.config.slave_id, &pdu, timeout).await?;
                check_write_response(&resp, 0x06)?;
            }
        }
        Ok(())
    }

    /// Write multiple coils (FC15).
    pub async fn write_multiple_coils(
        &self,
        address: u16,
        values: &[bool],
    ) -> Result<(), MasterError> {
        let transport_ctx = self.get_transport_ctx()?;
        let timeout = self.timeout_duration();
        self.log_tx(FunctionCode::WriteMultipleCoils, &format!("W {} x{}", address, values.len())).await;
        match &transport_ctx {
            TransportCtx::Tcp(ctx) => {
                let mut ctx = ctx.lock().await;
                tokio::time::timeout(timeout, ctx.write_multiple_coils(address, values))
                    .await
                    .map_err(|_| MasterError::Timeout("Write multiple coils timed out".into()))?
                    .map_err(|e| MasterError::Transport(format!("{e}")))?
                    .map_err(|e| MasterError::Exception(e))?;
            }
            other => {
                let quantity = values.len() as u16;
                let byte_count = (values.len() + 7) / 8;
                let mut coil_bytes = vec![0u8; byte_count];
                for (i, &v) in values.iter().enumerate() {
                    if v {
                        coil_bytes[i / 8] |= 1 << (i % 8);
                    }
                }
                let mut pdu = vec![0x0F];
                pdu.extend_from_slice(&address.to_be_bytes());
                pdu.extend_from_slice(&quantity.to_be_bytes());
                pdu.push(byte_count as u8);
                pdu.extend_from_slice(&coil_bytes);
                let resp = send_pdu_via_transport(other, self.config.slave_id, &pdu, timeout).await?;
                check_write_response(&resp, 0x0F)?;
            }
        }
        Ok(())
    }

    /// Write multiple holding registers (FC16).
    pub async fn write_multiple_registers(
        &self,
        address: u16,
        values: &[u16],
    ) -> Result<(), MasterError> {
        let transport_ctx = self.get_transport_ctx()?;
        let timeout = self.timeout_duration();
        self.log_tx(FunctionCode::WriteMultipleRegisters, &format!("W {} x{}", address, values.len())).await;
        match &transport_ctx {
            TransportCtx::Tcp(ctx) => {
                let mut ctx = ctx.lock().await;
                tokio::time::timeout(timeout, ctx.write_multiple_registers(address, values))
                    .await
                    .map_err(|_| MasterError::Timeout("Write multiple registers timed out".into()))?
                    .map_err(|e| MasterError::Transport(format!("{e}")))?
                    .map_err(|e| MasterError::Exception(e))?;
            }
            other => {
                let quantity = values.len() as u16;
                let byte_count = (values.len() * 2) as u8;
                let mut pdu = vec![0x10];
                pdu.extend_from_slice(&address.to_be_bytes());
                pdu.extend_from_slice(&quantity.to_be_bytes());
                pdu.push(byte_count);
                for v in values {
                    pdu.extend_from_slice(&v.to_be_bytes());
                }
                let resp = send_pdu_via_transport(other, self.config.slave_id, &pdu, timeout).await?;
                check_write_response(&resp, 0x10)?;
            }
        }
        Ok(())
    }

    /// Start polling with the given configuration (legacy single-poll API).
    /// Returns a receiver for poll events.
    pub async fn start_poll(
        &mut self,
        poll_config: PollConfig,
    ) -> Result<mpsc::Receiver<PollEvent>, MasterError> {
        let group = ScanGroup {
            id: "__legacy_poll__".to_string(),
            name: "Legacy Poll".to_string(),
            function: poll_config.function,
            start_address: poll_config.start_address,
            quantity: poll_config.quantity,
            interval_ms: poll_config.interval_ms,
            enabled: true,
            slave_id: None,
        };
        self.start_scan_group(&group).await
    }

    /// Stop the legacy single polling task.
    pub async fn stop_poll(&mut self) -> Result<(), MasterError> {
        self.stop_scan_group("__legacy_poll__").await
    }

    /// Whether any polling is currently active.
    pub fn is_polling(&self) -> bool {
        !self.poll_tasks.is_empty()
    }

    /// Whether a specific scan group is actively polling.
    pub fn is_scan_active(&self, group_id: &str) -> bool {
        self.poll_tasks.contains_key(group_id)
    }

    /// Start polling for a scan group.
    /// Returns a receiver for poll events from this group.
    pub async fn start_scan_group(
        &mut self,
        group: &ScanGroup,
    ) -> Result<mpsc::Receiver<PollEvent>, MasterError> {
        // Stop existing poll for this group if any
        self.stop_scan_group(&group.id).await.ok();

        let transport_ctx = self.get_transport_ctx()?;
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        let (event_tx, event_rx) = mpsc::channel::<PollEvent>(100);
        let timeout = self.timeout_duration();
        let function = group.function;
        let start_address = group.start_address;
        let quantity = group.quantity;
        let interval_ms = group.interval_ms;
        let log_collector = self.log_collector.clone();
        let group_slave_id = group.slave_id;
        let default_slave_id = self.config.slave_id;

        let handle = tokio::spawn(async move {
            let interval = Duration::from_millis(interval_ms);
            let fc = match function {
                ReadFunction::ReadCoils => FunctionCode::ReadCoils,
                ReadFunction::ReadDiscreteInputs => FunctionCode::ReadDiscreteInputs,
                ReadFunction::ReadHoldingRegisters => FunctionCode::ReadHoldingRegisters,
                ReadFunction::ReadInputRegisters => FunctionCode::ReadInputRegisters,
            };
            let slave_id = group_slave_id.unwrap_or(default_slave_id);
            loop {
                // Check for shutdown
                if shutdown_rx.try_recv().is_ok() {
                    break;
                }

                // Log TX
                if let Some(ref collector) = log_collector {
                    let entry = LogEntry::new(Direction::Tx, fc, format!("R {} x{}", start_address, quantity));
                    collector.add(entry).await;
                }

                let result = {
                    // For TCP with slave_id override, set it before reading
                    if let TransportCtx::Tcp(ref ctx) = transport_ctx {
                        if group_slave_id.is_some() {
                            let mut ctx = ctx.lock().await;
                            ctx.set_slave(Slave(slave_id));
                        }
                    }
                    execute_read_any(
                        &transport_ctx,
                        slave_id,
                        function,
                        start_address,
                        quantity,
                        timeout,
                    )
                    .await
                };

                // Log RX
                if let Some(ref collector) = log_collector {
                    let detail = match &result {
                        Ok(ReadResult::Coils(v)) => format!("{} coils", v.len()),
                        Ok(ReadResult::DiscreteInputs(v)) => format!("{} inputs", v.len()),
                        Ok(ReadResult::HoldingRegisters(v)) => format!("{} regs", v.len()),
                        Ok(ReadResult::InputRegisters(v)) => format!("{} regs", v.len()),
                        Err(e) => format!("ERR: {}", e),
                    };
                    let entry = LogEntry::new(Direction::Rx, fc, detail);
                    collector.add(entry).await;
                }

                let event = match result {
                    Ok(data) => PollEvent::Data(data),
                    Err(e) => PollEvent::Error(format!("{e}")),
                };

                if event_tx.send(event).await.is_err() {
                    break; // Receiver dropped
                }

                tokio::time::sleep(interval).await;
            }
        });

        self.poll_tasks.insert(
            group.id.clone(),
            PollTaskHandle {
                shutdown_tx,
                join_handle: handle,
            },
        );

        Ok(event_rx)
    }

    /// Stop a specific scan group's polling task.
    pub async fn stop_scan_group(&mut self, group_id: &str) -> Result<(), MasterError> {
        if let Some(handle) = self.poll_tasks.remove(group_id) {
            let _ = handle.shutdown_tx.send(());
            let _ = handle.join_handle.await;
        }
        Ok(())
    }

    /// Stop all active scan groups.
    pub async fn stop_all_scans(&mut self) {
        let tasks: Vec<(String, PollTaskHandle)> = self.poll_tasks.drain().collect();
        for (_, handle) in tasks {
            let _ = handle.shutdown_tx.send(());
            let _ = handle.join_handle.await;
        }
    }
}

// ---------------------------------------------------------------------------
// PDU helpers for non-TCP transports
// ---------------------------------------------------------------------------

fn build_read_pdu(function: ReadFunction, start_address: u16, quantity: u16) -> Vec<u8> {
    let fc: u8 = match function {
        ReadFunction::ReadCoils => 0x01,
        ReadFunction::ReadDiscreteInputs => 0x02,
        ReadFunction::ReadHoldingRegisters => 0x03,
        ReadFunction::ReadInputRegisters => 0x04,
    };
    let mut pdu = vec![fc];
    pdu.extend_from_slice(&start_address.to_be_bytes());
    pdu.extend_from_slice(&quantity.to_be_bytes());
    pdu
}

fn parse_read_response_pdu(
    function: ReadFunction,
    response_pdu: &[u8],
) -> Result<ReadResult, MasterError> {
    if response_pdu.is_empty() {
        return Err(MasterError::Transport("empty response".into()));
    }
    // Check for exception response (high bit set on function code)
    if response_pdu[0] & 0x80 != 0 {
        let exc = response_pdu.get(1).copied().unwrap_or(0);
        return Err(MasterError::Transport(format!(
            "Modbus exception: 0x{:02X}",
            exc
        )));
    }
    let byte_count = response_pdu.get(1).copied().unwrap_or(0) as usize;
    let data = if response_pdu.len() > 2 {
        &response_pdu[2..]
    } else {
        &[]
    };

    match function {
        ReadFunction::ReadCoils | ReadFunction::ReadDiscreteInputs => {
            let mut bits = Vec::new();
            for byte_idx in 0..byte_count {
                for bit_idx in 0..8 {
                    if byte_idx < data.len() {
                        bits.push((data[byte_idx] >> bit_idx) & 1 == 1);
                    }
                }
            }
            match function {
                ReadFunction::ReadCoils => Ok(ReadResult::Coils(bits)),
                _ => Ok(ReadResult::DiscreteInputs(bits)),
            }
        }
        ReadFunction::ReadHoldingRegisters | ReadFunction::ReadInputRegisters => {
            let mut regs = Vec::new();
            for chunk in data.chunks(2) {
                if chunk.len() == 2 {
                    regs.push(u16::from_be_bytes([chunk[0], chunk[1]]));
                }
            }
            match function {
                ReadFunction::ReadHoldingRegisters => Ok(ReadResult::HoldingRegisters(regs)),
                _ => Ok(ReadResult::InputRegisters(regs)),
            }
        }
    }
}

/// Send a raw PDU via a non-TCP transport and return the response PDU.
async fn send_pdu_via_transport(
    ctx: &TransportCtx,
    slave_id: u8,
    pdu: &[u8],
    timeout: Duration,
) -> Result<Vec<u8>, MasterError> {
    match ctx {
        TransportCtx::Rtu(t) => t
            .request(slave_id, pdu, timeout)
            .await
            .map_err(|e| MasterError::Transport(e)),
        TransportCtx::Ascii(t) => t
            .request(slave_id, pdu, timeout)
            .await
            .map_err(|e| MasterError::Transport(e)),
        TransportCtx::RtuTcp(t) => t
            .request(slave_id, pdu, timeout)
            .await
            .map_err(|e| MasterError::Transport(e)),
        TransportCtx::Tcp(_) => Err(MasterError::Transport(
            "send_pdu_via_transport called for TCP".into(),
        )),
    }
}

/// Check a write response PDU for exception.
fn check_write_response(resp: &[u8], expected_fc: u8) -> Result<(), MasterError> {
    if resp.is_empty() {
        return Err(MasterError::Transport("empty response".into()));
    }
    if resp[0] & 0x80 != 0 {
        let exc = resp.get(1).copied().unwrap_or(0);
        return Err(MasterError::Transport(format!(
            "Modbus exception: 0x{:02X}",
            exc
        )));
    }
    if resp[0] != expected_fc {
        return Err(MasterError::Transport(format!(
            "unexpected function code in response: expected 0x{:02X}, got 0x{:02X}",
            expected_fc, resp[0]
        )));
    }
    Ok(())
}

/// Execute a read via TCP tokio_modbus context.
async fn execute_read_tcp(
    ctx: &Arc<Mutex<client::Context>>,
    function: ReadFunction,
    start_address: u16,
    quantity: u16,
    timeout: Duration,
) -> Result<ReadResult, MasterError> {
    let mut ctx = ctx.lock().await;
    match function {
        ReadFunction::ReadCoils => {
            let data =
                tokio::time::timeout(timeout, ctx.read_coils(start_address, quantity))
                    .await
                    .map_err(|_| MasterError::Timeout("Read timed out".into()))?
                    .map_err(|e| MasterError::Transport(format!("{e}")))?
                    .map_err(|e| MasterError::Exception(e))?;
            Ok(ReadResult::Coils(data))
        }
        ReadFunction::ReadDiscreteInputs => {
            let data = tokio::time::timeout(
                timeout,
                ctx.read_discrete_inputs(start_address, quantity),
            )
            .await
            .map_err(|_| MasterError::Timeout("Read timed out".into()))?
            .map_err(|e| MasterError::Transport(format!("{e}")))?
            .map_err(|e| MasterError::Exception(e))?;
            Ok(ReadResult::DiscreteInputs(data))
        }
        ReadFunction::ReadHoldingRegisters => {
            let data = tokio::time::timeout(
                timeout,
                ctx.read_holding_registers(start_address, quantity),
            )
            .await
            .map_err(|_| MasterError::Timeout("Read timed out".into()))?
            .map_err(|e| MasterError::Transport(format!("{e}")))?
            .map_err(|e| MasterError::Exception(e))?;
            Ok(ReadResult::HoldingRegisters(data))
        }
        ReadFunction::ReadInputRegisters => {
            let data = tokio::time::timeout(
                timeout,
                ctx.read_input_registers(start_address, quantity),
            )
            .await
            .map_err(|_| MasterError::Timeout("Read timed out".into()))?
            .map_err(|e| MasterError::Transport(format!("{e}")))?
            .map_err(|e| MasterError::Exception(e))?;
            Ok(ReadResult::InputRegisters(data))
        }
    }
}

/// Execute a read operation on any transport type.
async fn execute_read_any(
    ctx: &TransportCtx,
    slave_id: u8,
    function: ReadFunction,
    start_address: u16,
    quantity: u16,
    timeout: Duration,
) -> Result<ReadResult, MasterError> {
    match ctx {
        TransportCtx::Tcp(tcp_ctx) => {
            execute_read_tcp(tcp_ctx, function, start_address, quantity, timeout).await
        }
        other => {
            let pdu = build_read_pdu(function, start_address, quantity);
            let resp = send_pdu_via_transport(other, slave_id, &pdu, timeout).await?;
            parse_read_response_pdu(function, &resp)
        }
    }
}

/// Format an ExceptionCode into a human-readable description.
pub fn exception_description(code: ExceptionCode) -> &'static str {
    match code {
        ExceptionCode::IllegalFunction => "Illegal Function",
        ExceptionCode::IllegalDataAddress => "Illegal Data Address",
        ExceptionCode::IllegalDataValue => "Illegal Data Value",
        ExceptionCode::ServerDeviceFailure => "Server Device Failure",
        ExceptionCode::Acknowledge => "Acknowledge",
        ExceptionCode::ServerDeviceBusy => "Server Device Busy",
        ExceptionCode::MemoryParityError => "Memory Parity Error",
        ExceptionCode::GatewayPathUnavailable => "Gateway Path Unavailable",
        ExceptionCode::GatewayTargetDevice => "Gateway Target Device Failed to Respond",
        _ => "Unknown Exception",
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MasterError {
    #[error("already connected")]
    AlreadyConnected,
    #[error("not connected")]
    NotConnected,
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("modbus exception: {0:?}")]
    Exception(ExceptionCode),
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

/// Progress report for slave ID scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaveIdScanProgress {
    pub current_id: u8,
    pub total: u16,
    pub found_ids: Vec<u8>,
    pub done: bool,
    pub cancelled: bool,
}

/// A register found during address scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoundRegister {
    pub address: u16,
    pub value: u16,
}

/// Progress report for register address scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterScanProgress {
    pub current_address: u16,
    pub end_address: u16,
    pub found_registers: Vec<FoundRegister>,
    pub done: bool,
    pub cancelled: bool,
}

/// Scan slave IDs 1-247 using an existing TCP context.
/// Changes slave ID via `set_slave()`, restores original when done.
pub async fn scan_slave_ids_with_ctx(
    ctx: Arc<Mutex<client::Context>>,
    original_slave_id: u8,
    start_id: u8,
    end_id: u8,
    scan_timeout: Duration,
    mut cancel_rx: oneshot::Receiver<()>,
    progress_tx: mpsc::Sender<SlaveIdScanProgress>,
) -> Vec<u8> {
    let mut found_ids: Vec<u8> = Vec::new();
    let total = end_id.saturating_sub(start_id) as u16 + 1;

    for id in start_id..=end_id {
        // Check cancellation
        if cancel_rx.try_recv().is_ok() {
            let _ = progress_tx.send(SlaveIdScanProgress {
                current_id: id,
                total,
                found_ids: found_ids.clone(),
                done: false,
                cancelled: true,
            }).await;
            break;
        }

        // Probe this slave ID
        let found = {
            let mut ctx = ctx.lock().await;
            ctx.set_slave(Slave(id));
            match tokio::time::timeout(scan_timeout, ctx.read_holding_registers(0, 1)).await {
                Ok(Ok(Ok(_))) => true,
                _ => false,
            }
        };

        if found {
            found_ids.push(id);
        }

        let _ = progress_tx.send(SlaveIdScanProgress {
            current_id: id,
            total,
            found_ids: found_ids.clone(),
            done: id == end_id,
            cancelled: false,
        }).await;
    }

    // Restore original slave ID
    {
        let mut ctx = ctx.lock().await;
        ctx.set_slave(Slave(original_slave_id));
    }

    // Send final done
    let _ = progress_tx.send(SlaveIdScanProgress {
        current_id: end_id,
        total,
        found_ids: found_ids.clone(),
        done: true,
        cancelled: false,
    }).await;

    found_ids
}

/// Scan a register address range using an existing TCP context.
pub async fn scan_registers_with_ctx(
    ctx: Arc<Mutex<client::Context>>,
    function: ReadFunction,
    start_address: u16,
    end_address: u16,
    chunk_size: u16,
    scan_timeout: Duration,
    mut cancel_rx: oneshot::Receiver<()>,
    progress_tx: mpsc::Sender<RegisterScanProgress>,
) -> Vec<FoundRegister> {
    let mut found: Vec<FoundRegister> = Vec::new();
    let mut addr = start_address;

    while addr <= end_address {
        // Check cancellation
        if cancel_rx.try_recv().is_ok() {
            let _ = progress_tx.send(RegisterScanProgress {
                current_address: addr,
                end_address,
                found_registers: found.clone(),
                done: false,
                cancelled: true,
            }).await;
            break;
        }

        let qty = chunk_size.min(end_address - addr + 1);

        let result = {
            let mut ctx = ctx.lock().await;
            let read_fut = match function {
                ReadFunction::ReadCoils => {
                    let f = ctx.read_coils(addr, qty);
                    tokio::time::timeout(scan_timeout, f).await
                        .ok()
                        .and_then(|r| r.ok())
                        .and_then(|r| r.ok())
                        .map(|vals| vals.iter().map(|&b| if b { 1u16 } else { 0u16 }).collect::<Vec<_>>())
                }
                ReadFunction::ReadDiscreteInputs => {
                    let f = ctx.read_discrete_inputs(addr, qty);
                    tokio::time::timeout(scan_timeout, f).await
                        .ok()
                        .and_then(|r| r.ok())
                        .and_then(|r| r.ok())
                        .map(|vals| vals.iter().map(|&b| if b { 1u16 } else { 0u16 }).collect::<Vec<_>>())
                }
                ReadFunction::ReadHoldingRegisters => {
                    let f = ctx.read_holding_registers(addr, qty);
                    tokio::time::timeout(scan_timeout, f).await
                        .ok()
                        .and_then(|r| r.ok())
                        .and_then(|r| r.ok())
                }
                ReadFunction::ReadInputRegisters => {
                    let f = ctx.read_input_registers(addr, qty);
                    tokio::time::timeout(scan_timeout, f).await
                        .ok()
                        .and_then(|r| r.ok())
                        .and_then(|r| r.ok())
                }
            };
            read_fut
        };

        if let Some(values) = result {
            for (i, &val) in values.iter().enumerate() {
                found.push(FoundRegister {
                    address: addr + i as u16,
                    value: val,
                });
            }
        }

        let done = addr + qty > end_address;
        let _ = progress_tx.send(RegisterScanProgress {
            current_address: addr + qty - 1,
            end_address,
            found_registers: found.clone(),
            done,
            cancelled: false,
        }).await;

        addr = addr.saturating_add(qty);
        if addr == 0 && end_address == u16::MAX {
            break; // overflow protection
        }
    }

    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_master_config_default() {
        let config = MasterConfig::default();
        assert_eq!(config.target_address, "127.0.0.1");
        assert_eq!(config.port, 502);
        assert_eq!(config.slave_id, 1);
        assert_eq!(config.timeout_ms, 3000);
    }

    #[test]
    fn test_master_connection_initial_state() {
        let config = MasterConfig::default();
        let transport = Transport::Tcp {
            host: config.target_address.clone(),
            port: config.port,
        };
        let conn = MasterConnection::new(config, transport);
        assert_eq!(conn.state(), MasterState::Disconnected);
        assert!(!conn.is_polling());
    }

    #[test]
    fn test_exception_description() {
        assert_eq!(
            exception_description(ExceptionCode::IllegalFunction),
            "Illegal Function"
        );
        assert_eq!(
            exception_description(ExceptionCode::IllegalDataAddress),
            "Illegal Data Address"
        );
    }

    #[test]
    fn test_master_error_display() {
        let err = MasterError::Exception(ExceptionCode::IllegalDataAddress);
        let msg = err.to_string();
        assert!(msg.contains("IllegalDataAddress") || msg.contains("Illegal"));
    }

    #[test]
    fn test_scan_group_serde() {
        let group = ScanGroup {
            id: "sg1".to_string(),
            name: "Test Group".to_string(),
            function: ReadFunction::ReadHoldingRegisters,
            start_address: 0,
            quantity: 10,
            interval_ms: 1000,
            enabled: true,
            slave_id: None,
        };
        let json = serde_json::to_string(&group).unwrap();
        let parsed: ScanGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "sg1");
        assert_eq!(parsed.quantity, 10);
    }
}
