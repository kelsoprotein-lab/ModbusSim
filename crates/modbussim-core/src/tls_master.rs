//! TLS-enabled Modbus TCP master client.
//!
//! Uses native-tls for the TLS layer and MBAP framing (same as plain TCP Modbus)
//! over the encrypted stream. All I/O is synchronous (native_tls::TlsStream) so
//! we use `spawn_blocking` to avoid blocking the Tokio runtime.

use crate::master::{MasterError, ReadFunction, ReadResult};
use crate::mbap;
use crate::transport::TlsConfig;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// A TLS-wrapped Modbus TCP master connection.
pub struct TlsMasterConnection {
    stream: Arc<Mutex<native_tls::TlsStream<std::net::TcpStream>>>,
    transaction_id: AtomicU16,
}

impl TlsMasterConnection {
    fn next_transaction_id(&self) -> u16 {
        self.transaction_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Core send/receive over TLS. Runs in spawn_blocking because native_tls is sync.
    async fn send_receive(
        &self,
        slave_id: u8,
        pdu: &[u8],
        timeout: Duration,
    ) -> Result<Vec<u8>, MasterError> {
        let stream = self.stream.clone();
        let tid = self.next_transaction_id();
        let pdu = pdu.to_vec();

        tokio::task::spawn_blocking(move || {
            let mut stream = stream.blocking_lock();

            // Set timeouts on the underlying TCP stream
            let tcp = stream.get_ref();
            tcp.set_read_timeout(Some(timeout))
                .map_err(|e| MasterError::Transport(format!("set read timeout: {e}")))?;
            tcp.set_write_timeout(Some(timeout))
                .map_err(|e| MasterError::Transport(format!("set write timeout: {e}")))?;

            // Write MBAP frame
            mbap::write_frame(&mut *stream, tid, slave_id, &pdu)
                .map_err(|e| MasterError::Transport(format!("TLS write: {e}")))?;

            // Read MBAP response frame
            let (_header, resp_pdu) = mbap::read_frame(&mut *stream)
                .map_err(|e| MasterError::Transport(format!("TLS read: {e}")))?;

            Ok(resp_pdu)
        })
        .await
        .map_err(|e| MasterError::Transport(format!("spawn_blocking: {e}")))?
    }

    /// Read coils, discrete inputs, holding registers, or input registers.
    pub async fn read(
        &self,
        slave_id: u8,
        function: ReadFunction,
        start_address: u16,
        quantity: u16,
        timeout: Duration,
    ) -> Result<ReadResult, MasterError> {
        let fc: u8 = match function {
            ReadFunction::ReadCoils => 0x01,
            ReadFunction::ReadDiscreteInputs => 0x02,
            ReadFunction::ReadHoldingRegisters => 0x03,
            ReadFunction::ReadInputRegisters => 0x04,
        };
        let mut pdu = vec![fc];
        pdu.extend_from_slice(&start_address.to_be_bytes());
        pdu.extend_from_slice(&quantity.to_be_bytes());

        let resp = self.send_receive(slave_id, &pdu, timeout).await?;
        parse_response(function, &resp)
    }

    /// Write a single coil (FC05).
    pub async fn write_single_coil(
        &self,
        slave_id: u8,
        address: u16,
        value: bool,
        timeout: Duration,
    ) -> Result<(), MasterError> {
        let coil_value: u16 = if value { 0xFF00 } else { 0x0000 };
        let mut pdu = vec![0x05];
        pdu.extend_from_slice(&address.to_be_bytes());
        pdu.extend_from_slice(&coil_value.to_be_bytes());
        let resp = self.send_receive(slave_id, &pdu, timeout).await?;
        check_write_response(&resp, 0x05)
    }

    /// Write a single holding register (FC06).
    pub async fn write_single_register(
        &self,
        slave_id: u8,
        address: u16,
        value: u16,
        timeout: Duration,
    ) -> Result<(), MasterError> {
        let mut pdu = vec![0x06];
        pdu.extend_from_slice(&address.to_be_bytes());
        pdu.extend_from_slice(&value.to_be_bytes());
        let resp = self.send_receive(slave_id, &pdu, timeout).await?;
        check_write_response(&resp, 0x06)
    }

    /// Write multiple coils (FC15 / 0x0F).
    pub async fn write_multiple_coils(
        &self,
        slave_id: u8,
        address: u16,
        values: &[bool],
        timeout: Duration,
    ) -> Result<(), MasterError> {
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
        let resp = self.send_receive(slave_id, &pdu, timeout).await?;
        check_write_response(&resp, 0x0F)
    }

    /// Write multiple holding registers (FC16 / 0x10).
    pub async fn write_multiple_registers(
        &self,
        slave_id: u8,
        address: u16,
        values: &[u16],
        timeout: Duration,
    ) -> Result<(), MasterError> {
        let quantity = values.len() as u16;
        let byte_count = (values.len() * 2) as u8;
        let mut pdu = vec![0x10];
        pdu.extend_from_slice(&address.to_be_bytes());
        pdu.extend_from_slice(&quantity.to_be_bytes());
        pdu.push(byte_count);
        for v in values {
            pdu.extend_from_slice(&v.to_be_bytes());
        }
        let resp = self.send_receive(slave_id, &pdu, timeout).await?;
        check_write_response(&resp, 0x10)
    }
}

// ---------------------------------------------------------------------------
// Response parsing helpers
// ---------------------------------------------------------------------------

fn parse_response(function: ReadFunction, resp: &[u8]) -> Result<ReadResult, MasterError> {
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
    let byte_count = resp.get(1).copied().unwrap_or(0) as usize;
    let data = if resp.len() > 2 { &resp[2..] } else { &[] };

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
            "unexpected function code: expected 0x{:02X}, got 0x{:02X}",
            expected_fc, resp[0]
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// TLS connector & connection factory
// ---------------------------------------------------------------------------

/// Build a `native_tls::TlsConnector` from the given config.
pub fn build_tls_connector(config: &TlsConfig) -> Result<native_tls::TlsConnector, MasterError> {
    let mut builder = native_tls::TlsConnector::builder();
    builder.min_protocol_version(Some(native_tls::Protocol::Tlsv12));

    // Load CA certificate if configured
    if !config.ca_file.is_empty() {
        let ca_pem = std::fs::read(&config.ca_file)
            .map_err(|e| MasterError::ConnectionFailed(format!("read CA file: {e}")))?;
        let cert = native_tls::Certificate::from_pem(&ca_pem)
            .map_err(|e| MasterError::ConnectionFailed(format!("parse CA cert: {e}")))?;
        builder.add_root_certificate(cert);
    }

    // Load client identity — PKCS#12 takes priority, then PEM cert+key
    if !config.pkcs12_file.is_empty() {
        let pkcs12_data = std::fs::read(&config.pkcs12_file)
            .map_err(|e| MasterError::ConnectionFailed(format!("read PKCS#12 file: {e}")))?;
        let identity =
            native_tls::Identity::from_pkcs12(&pkcs12_data, &config.pkcs12_password)
                .map_err(|e| MasterError::ConnectionFailed(format!("parse PKCS#12: {e}")))?;
        builder.identity(identity);
    } else if !config.cert_file.is_empty() && !config.key_file.is_empty() {
        let cert_pem = std::fs::read(&config.cert_file)
            .map_err(|e| MasterError::ConnectionFailed(format!("read cert file: {e}")))?;
        let key_pem = std::fs::read(&config.key_file)
            .map_err(|e| MasterError::ConnectionFailed(format!("read key file: {e}")))?;
        // Concatenate cert + key for PEM identity
        let mut pem = cert_pem;
        pem.extend_from_slice(&key_pem);
        let identity = native_tls::Identity::from_pkcs8(&pem, &key_pem)
            .map_err(|e| MasterError::ConnectionFailed(format!("parse PEM identity: {e}")))?;
        builder.identity(identity);
    }

    if config.accept_invalid_certs {
        builder.danger_accept_invalid_certs(true);
    }

    builder
        .build()
        .map_err(|e| MasterError::ConnectionFailed(format!("TLS connector build: {e}")))
}

/// Establish a TLS connection to the given address/port.
pub async fn connect_tls(
    host: &str,
    port: u16,
    tls_config: &TlsConfig,
    timeout: Duration,
) -> Result<TlsMasterConnection, MasterError> {
    let addr = format!("{}:{}", host, port);
    let host_owned = host.to_string();

    // TCP connect with timeout
    let tcp_stream = tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr))
        .await
        .map_err(|_| MasterError::Timeout("TLS TCP connect timed out".into()))?
        .map_err(|e| MasterError::ConnectionFailed(format!("TCP connect: {e}")))?;

    // Convert to std TcpStream for native_tls
    let std_stream = tcp_stream
        .into_std()
        .map_err(|e| MasterError::ConnectionFailed(format!("convert to std stream: {e}")))?;

    let connector = build_tls_connector(tls_config)?;

    // TLS handshake in spawn_blocking (native_tls is synchronous)
    let tls_stream = tokio::task::spawn_blocking(move || {
        connector
            .connect(&host_owned, std_stream)
            .map_err(|e| MasterError::ConnectionFailed(format!("TLS handshake: {e}")))
    })
    .await
    .map_err(|e| MasterError::ConnectionFailed(format!("spawn_blocking: {e}")))?
    ?;

    Ok(TlsMasterConnection {
        stream: Arc::new(Mutex::new(tls_stream)),
        transaction_id: AtomicU16::new(1),
    })
}
