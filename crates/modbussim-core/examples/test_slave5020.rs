/// Test: connect to the user's slave at port 5020
use modbussim_core::master::{MasterConfig, MasterConnection, ReadFunction};
use modbussim_core::transport::Transport;
use std::time::Duration;

#[tokio::main]
async fn main() {
    println!("Testing connection to external slave at 127.0.0.1:5020...\n");

    let config = MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port: 5020,
        slave_id: 1,
        timeout_ms: 5000,
        ..Default::default()
    };

    let transport = Transport::Tcp {
        host: config.target_address.clone(),
        port: config.port,
    };
    // No log collector to avoid any logging issues
    let mut conn = MasterConnection::new(config, transport);

    println!("Connecting...");
    match conn.connect().await {
        Ok(_) => println!("Connected!"),
        Err(e) => {
            println!("FAILED to connect: {}", e);
            return;
        }
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    println!("Reading FC03 addr=0 qty=10...");
    match conn.read(ReadFunction::ReadHoldingRegisters, 0, 10).await {
        Ok(result) => println!("SUCCESS: {:?}", result),
        Err(e) => println!("FAILED: {}", e),
    }

    let _ = conn.disconnect().await;
}
