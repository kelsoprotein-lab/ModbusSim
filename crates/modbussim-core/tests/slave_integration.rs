use modbussim_core::slave::{SlaveConnection, SlaveDevice, TransportConfig};
use std::net::SocketAddr;
use tokio_modbus::prelude::*;

/// Helper: start a slave connection on the given port with pre-configured devices.
async fn start_slave(port: u16) -> SlaveConnection {
    let transport = TransportConfig {
        bind_address: "127.0.0.1".to_string(),
        port,
    };
    let mut conn = SlaveConnection::new(transport);

    // Add slave device 1 with some preset values
    let mut device1 = SlaveDevice::new(1, "Device 1");
    device1.register_map.write_holding_register(0, 1000);
    device1.register_map.write_holding_register(1, 2000);
    device1.register_map.write_holding_register(2, 3000);
    device1.register_map.write_coil(0, true);
    device1.register_map.write_coil(1, false);
    device1.register_map.write_coil(2, true);
    device1
        .register_map
        .input_registers
        .insert(0, 100);
    device1
        .register_map
        .input_registers
        .insert(1, 200);
    device1
        .register_map
        .discrete_inputs
        .insert(0, true);
    device1
        .register_map
        .discrete_inputs
        .insert(1, false);
    conn.add_device(device1).await.unwrap();

    // Add slave device 2
    let mut device2 = SlaveDevice::new(2, "Device 2");
    device2.register_map.write_holding_register(0, 9999);
    conn.add_device(device2).await.unwrap();

    conn.start().await.unwrap();

    // Give the server a moment to start listening
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    conn
}

#[tokio::test]
async fn test_read_holding_registers() {
    let mut conn = start_slave(15001).await;
    let addr: SocketAddr = "127.0.0.1:15001".parse().unwrap();

    let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();
    let values = ctx.read_holding_registers(0, 3).await.unwrap().unwrap();
    assert_eq!(values, vec![1000, 2000, 3000]);

    ctx.disconnect().await.unwrap();
    conn.stop().await.unwrap();
}

#[tokio::test]
async fn test_read_coils() {
    let mut conn = start_slave(15002).await;
    let addr: SocketAddr = "127.0.0.1:15002".parse().unwrap();

    let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();
    let values = ctx.read_coils(0, 3).await.unwrap().unwrap();
    assert_eq!(values, vec![true, false, true]);

    ctx.disconnect().await.unwrap();
    conn.stop().await.unwrap();
}

#[tokio::test]
async fn test_read_discrete_inputs() {
    let mut conn = start_slave(15003).await;
    let addr: SocketAddr = "127.0.0.1:15003".parse().unwrap();

    let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();
    let values = ctx.read_discrete_inputs(0, 2).await.unwrap().unwrap();
    assert_eq!(values, vec![true, false]);

    ctx.disconnect().await.unwrap();
    conn.stop().await.unwrap();
}

#[tokio::test]
async fn test_read_input_registers() {
    let mut conn = start_slave(15004).await;
    let addr: SocketAddr = "127.0.0.1:15004".parse().unwrap();

    let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();
    let values = ctx.read_input_registers(0, 2).await.unwrap().unwrap();
    assert_eq!(values, vec![100, 200]);

    ctx.disconnect().await.unwrap();
    conn.stop().await.unwrap();
}

#[tokio::test]
async fn test_write_single_coil() {
    let mut conn = start_slave(15005).await;
    let addr: SocketAddr = "127.0.0.1:15005".parse().unwrap();

    let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();

    // Write a coil then read it back
    ctx.write_single_coil(10, true).await.unwrap().unwrap();
    let values = ctx.read_coils(10, 1).await.unwrap().unwrap();
    assert_eq!(values, vec![true]);

    ctx.disconnect().await.unwrap();
    conn.stop().await.unwrap();
}

#[tokio::test]
async fn test_write_single_register() {
    let mut conn = start_slave(15006).await;
    let addr: SocketAddr = "127.0.0.1:15006".parse().unwrap();

    let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();

    ctx.write_single_register(10, 42).await.unwrap().unwrap();
    let values = ctx.read_holding_registers(10, 1).await.unwrap().unwrap();
    assert_eq!(values, vec![42]);

    ctx.disconnect().await.unwrap();
    conn.stop().await.unwrap();
}

#[tokio::test]
async fn test_write_multiple_coils() {
    let mut conn = start_slave(15007).await;
    let addr: SocketAddr = "127.0.0.1:15007".parse().unwrap();

    let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();

    ctx.write_multiple_coils(20, &[true, false, true, true])
        .await
        .unwrap()
        .unwrap();
    let values = ctx.read_coils(20, 4).await.unwrap().unwrap();
    assert_eq!(values, vec![true, false, true, true]);

    ctx.disconnect().await.unwrap();
    conn.stop().await.unwrap();
}

#[tokio::test]
async fn test_write_multiple_registers() {
    let mut conn = start_slave(15008).await;
    let addr: SocketAddr = "127.0.0.1:15008".parse().unwrap();

    let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();

    ctx.write_multiple_registers(20, &[111, 222, 333])
        .await
        .unwrap()
        .unwrap();
    let values = ctx.read_holding_registers(20, 3).await.unwrap().unwrap();
    assert_eq!(values, vec![111, 222, 333]);

    ctx.disconnect().await.unwrap();
    conn.stop().await.unwrap();
}

#[tokio::test]
async fn test_slave_id_routing() {
    let mut conn = start_slave(15009).await;
    let addr: SocketAddr = "127.0.0.1:15009".parse().unwrap();

    // Read from slave 1
    let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();
    let v1 = ctx.read_holding_registers(0, 1).await.unwrap().unwrap();
    assert_eq!(v1, vec![1000]);

    // Switch to slave 2 and read different values
    ctx.set_slave(Slave(2));
    let v2 = ctx.read_holding_registers(0, 1).await.unwrap().unwrap();
    assert_eq!(v2, vec![9999]);

    ctx.disconnect().await.unwrap();
    conn.stop().await.unwrap();
}

#[tokio::test]
async fn test_unknown_slave_id_timeout() {
    let mut conn = start_slave(15010).await;
    let addr: SocketAddr = "127.0.0.1:15010".parse().unwrap();

    // Connect as slave 99 (does not exist) — server should silently drop
    let mut ctx = tcp::connect_slave(addr, Slave(99)).await.unwrap();

    // This should time out because the server won't respond
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        ctx.read_holding_registers(0, 1),
    )
    .await;

    // Should be a timeout (Err from tokio::time::timeout)
    assert!(result.is_err(), "Expected timeout for unknown slave ID");

    ctx.disconnect().await.unwrap();
    conn.stop().await.unwrap();
}

#[tokio::test]
async fn test_multiple_concurrent_clients() {
    let mut conn = start_slave(15011).await;
    let addr: SocketAddr = "127.0.0.1:15011".parse().unwrap();

    // Spawn two clients concurrently
    let h1 = tokio::spawn(async move {
        let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();
        let values = ctx.read_holding_registers(0, 3).await.unwrap().unwrap();
        assert_eq!(values, vec![1000, 2000, 3000]);
        ctx.disconnect().await.unwrap();
    });

    let h2 = tokio::spawn(async move {
        let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();
        let values = ctx.read_holding_registers(0, 3).await.unwrap().unwrap();
        assert_eq!(values, vec![1000, 2000, 3000]);
        ctx.disconnect().await.unwrap();
    });

    h1.await.unwrap();
    h2.await.unwrap();

    conn.stop().await.unwrap();
}

#[tokio::test]
async fn test_write_persists_across_clients() {
    let mut conn = start_slave(15012).await;
    let addr: SocketAddr = "127.0.0.1:15012".parse().unwrap();

    // Client 1 writes
    {
        let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();
        ctx.write_single_register(50, 7777).await.unwrap().unwrap();
        ctx.disconnect().await.unwrap();
    }

    // Client 2 reads the written value
    {
        let mut ctx = tcp::connect_slave(addr, Slave(1)).await.unwrap();
        let values = ctx.read_holding_registers(50, 1).await.unwrap().unwrap();
        assert_eq!(values, vec![7777]);
        ctx.disconnect().await.unwrap();
    }

    conn.stop().await.unwrap();
}
