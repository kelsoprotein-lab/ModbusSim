use crate::log_collector::LogCollector;
use crate::log_entry::{Direction, FunctionCode, LogEntry};
use serde::{Deserialize, Serialize};
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

/// A master connection that connects to a Modbus TCP slave.
pub struct MasterConnection {
    pub config: MasterConfig,
    state: MasterState,
    ctx: Option<Arc<Mutex<client::Context>>>,
    poll_shutdown: Option<oneshot::Sender<()>>,
    poll_handle: Option<tokio::task::JoinHandle<()>>,
    log_collector: Option<Arc<LogCollector>>,
}

impl MasterConnection {
    pub fn new(config: MasterConfig) -> Self {
        Self {
            config,
            state: MasterState::Disconnected,
            ctx: None,
            poll_shutdown: None,
            poll_handle: None,
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

    /// Connect to the target Modbus TCP slave.
    pub async fn connect(&mut self) -> Result<(), MasterError> {
        if self.state == MasterState::Connected {
            return Err(MasterError::AlreadyConnected);
        }

        let addr: SocketAddr =
            format!("{}:{}", self.config.target_address, self.config.port)
                .parse()
                .map_err(|e| MasterError::ConnectionFailed(format!("Invalid address: {e}")))?;

        let ctx = tokio::time::timeout(
            Duration::from_millis(self.config.timeout_ms),
            tcp::connect_slave(addr, Slave(self.config.slave_id)),
        )
        .await
        .map_err(|_| MasterError::Timeout("Connection timed out".to_string()))?
        .map_err(|e| MasterError::ConnectionFailed(format!("{e}")))?;

        self.ctx = Some(Arc::new(Mutex::new(ctx)));
        self.state = MasterState::Connected;
        Ok(())
    }

    /// Disconnect from the target.
    pub async fn disconnect(&mut self) -> Result<(), MasterError> {
        if self.state == MasterState::Disconnected {
            return Err(MasterError::NotConnected);
        }

        // Stop polling first if active
        self.stop_poll().await.ok();

        if let Some(ctx) = self.ctx.take() {
            let mut ctx = ctx.lock().await;
            let _ = ctx.disconnect().await;
        }
        self.state = MasterState::Disconnected;
        Ok(())
    }

    fn get_ctx(&self) -> Result<Arc<Mutex<client::Context>>, MasterError> {
        self.ctx.clone().ok_or(MasterError::NotConnected)
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

    fn log_tx(&self, fc: FunctionCode, detail: &str) {
        if let Some(collector) = &self.log_collector {
            let entry = LogEntry::new(Direction::Tx, fc, detail);
            collector.add_blocking(entry);
        }
    }

    fn log_rx(&self, fc: FunctionCode, detail: &str) {
        if let Some(collector) = &self.log_collector {
            let entry = LogEntry::new(Direction::Rx, fc, detail);
            collector.add_blocking(entry);
        }
    }

    /// Read registers using the specified function code.
    pub async fn read(
        &self,
        function: ReadFunction,
        start_address: u16,
        quantity: u16,
    ) -> Result<ReadResult, MasterError> {
        let ctx = self.get_ctx()?;
        let mut ctx = ctx.lock().await;
        let timeout = self.timeout_duration();
        let fc = Self::to_function_code(function);

        // Log TX
        self.log_tx(fc, &format!("R {} x{}", start_address, quantity));

        let result = match function {
            ReadFunction::ReadCoils => {
                let data = tokio::time::timeout(timeout, ctx.read_coils(start_address, quantity))
                    .await
                    .map_err(|_| MasterError::Timeout("Read coils timed out".into()))?
                    .map_err(|e| MasterError::Transport(format!("{e}")))?
                    .map_err(|e| MasterError::Exception(e))?;
                ReadResult::Coils(data)
            }
            ReadFunction::ReadDiscreteInputs => {
                let data = tokio::time::timeout(
                    timeout,
                    ctx.read_discrete_inputs(start_address, quantity),
                )
                .await
                .map_err(|_| MasterError::Timeout("Read discrete inputs timed out".into()))?
                .map_err(|e| MasterError::Transport(format!("{e}")))?
                .map_err(|e| MasterError::Exception(e))?;
                ReadResult::DiscreteInputs(data)
            }
            ReadFunction::ReadHoldingRegisters => {
                let data = tokio::time::timeout(
                    timeout,
                    ctx.read_holding_registers(start_address, quantity),
                )
                .await
                .map_err(|_| MasterError::Timeout("Read holding registers timed out".into()))?
                .map_err(|e| MasterError::Transport(format!("{e}")))?
                .map_err(|e| MasterError::Exception(e))?;
                ReadResult::HoldingRegisters(data)
            }
            ReadFunction::ReadInputRegisters => {
                let data = tokio::time::timeout(
                    timeout,
                    ctx.read_input_registers(start_address, quantity),
                )
                .await
                .map_err(|_| MasterError::Timeout("Read input registers timed out".into()))?
                .map_err(|e| MasterError::Transport(format!("{e}")))?
                .map_err(|e| MasterError::Exception(e))?;
                ReadResult::InputRegisters(data)
            }
        };

        // Log RX with result data
        let detail = match &result {
            ReadResult::Coils(vals) => format!("{:?}", vals),
            ReadResult::DiscreteInputs(vals) => format!("{:?}", vals),
            ReadResult::HoldingRegisters(vals) => format!("{:?}", vals),
            ReadResult::InputRegisters(vals) => format!("{:?}", vals),
        };
        self.log_rx(fc, &detail);

        Ok(result)
    }

    /// Write a single coil (FC05).
    pub async fn write_single_coil(
        &self,
        address: u16,
        value: bool,
    ) -> Result<(), MasterError> {
        let ctx = self.get_ctx()?;
        let mut ctx = ctx.lock().await;
        self.log_tx(FunctionCode::WriteSingleCoil, &format!("W {} = {}", address, value));
        tokio::time::timeout(self.timeout_duration(), ctx.write_single_coil(address, value))
            .await
            .map_err(|_| MasterError::Timeout("Write single coil timed out".into()))?
            .map_err(|e| MasterError::Transport(format!("{e}")))?
            .map_err(|e| MasterError::Exception(e))?;
        Ok(())
    }

    /// Write a single holding register (FC06).
    pub async fn write_single_register(
        &self,
        address: u16,
        value: u16,
    ) -> Result<(), MasterError> {
        let ctx = self.get_ctx()?;
        let mut ctx = ctx.lock().await;
        self.log_tx(FunctionCode::WriteSingleRegister, &format!("W {} = {:#06x}", address, value));
        tokio::time::timeout(
            self.timeout_duration(),
            ctx.write_single_register(address, value),
        )
        .await
        .map_err(|_| MasterError::Timeout("Write single register timed out".into()))?
        .map_err(|e| MasterError::Transport(format!("{e}")))?
        .map_err(|e| MasterError::Exception(e))?;
        Ok(())
    }

    /// Write multiple coils (FC15).
    pub async fn write_multiple_coils(
        &self,
        address: u16,
        values: &[bool],
    ) -> Result<(), MasterError> {
        let ctx = self.get_ctx()?;
        let mut ctx = ctx.lock().await;
        self.log_tx(FunctionCode::WriteMultipleCoils, &format!("W {} x{}", address, values.len()));
        tokio::time::timeout(
            self.timeout_duration(),
            ctx.write_multiple_coils(address, values),
        )
        .await
        .map_err(|_| MasterError::Timeout("Write multiple coils timed out".into()))?
        .map_err(|e| MasterError::Transport(format!("{e}")))?
        .map_err(|e| MasterError::Exception(e))?;
        Ok(())
    }

    /// Write multiple holding registers (FC16).
    pub async fn write_multiple_registers(
        &self,
        address: u16,
        values: &[u16],
    ) -> Result<(), MasterError> {
        let ctx = self.get_ctx()?;
        let mut ctx = ctx.lock().await;
        self.log_tx(FunctionCode::WriteMultipleRegisters, &format!("W {} x{}", address, values.len()));
        tokio::time::timeout(
            self.timeout_duration(),
            ctx.write_multiple_registers(address, values),
        )
        .await
        .map_err(|_| MasterError::Timeout("Write multiple registers timed out".into()))?
        .map_err(|e| MasterError::Transport(format!("{e}")))?
        .map_err(|e| MasterError::Exception(e))?;
        Ok(())
    }

    /// Start polling with the given configuration.
    /// Returns a receiver for poll events.
    pub async fn start_poll(
        &mut self,
        poll_config: PollConfig,
    ) -> Result<mpsc::Receiver<PollEvent>, MasterError> {
        if self.poll_handle.is_some() {
            self.stop_poll().await?;
        }

        let ctx = self.get_ctx()?;
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        let (event_tx, event_rx) = mpsc::channel::<PollEvent>(100);
        let timeout = self.timeout_duration();

        let handle = tokio::spawn(async move {
            let interval = Duration::from_millis(poll_config.interval_ms);
            loop {
                // Check for shutdown
                if shutdown_rx.try_recv().is_ok() {
                    break;
                }

                let result = {
                    let mut ctx = ctx.lock().await;
                    execute_read(&mut ctx, poll_config.function, poll_config.start_address, poll_config.quantity, timeout).await
                };

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

        self.poll_shutdown = Some(shutdown_tx);
        self.poll_handle = Some(handle);
        Ok(event_rx)
    }

    /// Stop the active polling task.
    pub async fn stop_poll(&mut self) -> Result<(), MasterError> {
        if let Some(tx) = self.poll_shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.poll_handle.take() {
            let _ = handle.await;
        }
        Ok(())
    }

    /// Whether polling is currently active.
    pub fn is_polling(&self) -> bool {
        self.poll_handle.is_some()
    }
}

/// Execute a single read operation on a client context.
async fn execute_read(
    ctx: &mut client::Context,
    function: ReadFunction,
    start_address: u16,
    quantity: u16,
    timeout: Duration,
) -> Result<ReadResult, MasterError> {
    match function {
        ReadFunction::ReadCoils => {
            let data =
                tokio::time::timeout(timeout, ctx.read_coils(start_address, quantity))
                    .await
                    .map_err(|_| MasterError::Timeout("Poll read timed out".into()))?
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
            .map_err(|_| MasterError::Timeout("Poll read timed out".into()))?
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
            .map_err(|_| MasterError::Timeout("Poll read timed out".into()))?
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
            .map_err(|_| MasterError::Timeout("Poll read timed out".into()))?
            .map_err(|e| MasterError::Transport(format!("{e}")))?
            .map_err(|e| MasterError::Exception(e))?;
            Ok(ReadResult::InputRegisters(data))
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
        let conn = MasterConnection::new(MasterConfig::default());
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
}
