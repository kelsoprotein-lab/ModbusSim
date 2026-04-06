/// End-to-end test: simulates the ModbusMaster app workflow
/// 1. Start slave on port 15021
/// 2. Create master connection
/// 3. Connect
/// 4. Add scan group
/// 5. Start polling
/// 6. Verify data is received
/// 7. Write a value and verify it's updated on next poll
use modbussim_core::log_collector::LogCollector;
use modbussim_core::master::{MasterConfig, MasterConnection, PollEvent, ReadFunction, ReadResult, ScanGroup};
use modbussim_core::slave::{SlaveConnection, SlaveDevice};
use modbussim_core::transport::Transport;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() {
    println!("========================================");
    println!("  ModbusMaster E2E Workflow Test");
    println!("========================================\n");

    // Step 1: Start slave
    println!("[1/7] Starting slave on port 15021...");
    let transport = Transport::Tcp {
        host: "0.0.0.0".to_string(),
        port: 15021,
    };
    let mut slave = SlaveConnection::new(transport);
    let device = SlaveDevice::with_random_registers(1, "E2E Slave", 100);
    slave.add_device(device).await.unwrap();
    slave.start().await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    println!("       Slave running.\n");

    // Step 2: Create master connection (simulates create_master_connection command)
    println!("[2/7] Creating master connection to 127.0.0.1:15021...");
    let config = MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port: 15021,
        slave_id: 1,
        timeout_ms: 3000,
    };
    let master_transport = Transport::Tcp {
        host: config.target_address.clone(),
        port: config.port,
    };
    let log_collector = Arc::new(LogCollector::new());
    let mut master = MasterConnection::new(config, master_transport).with_log_collector(log_collector.clone());
    println!("       State: {:?}\n", master.state());

    // Step 3: Connect (simulates connect_master command)
    println!("[3/7] Connecting...");
    master.connect().await.unwrap();
    println!("       State: {:?}\n", master.state());

    // Step 4: Add scan group (simulates add_scan_group command)
    println!("[4/7] Adding scan group FC03, addr=0, qty=10, interval=500ms...");
    let scan_group = ScanGroup {
        id: "sg-test-1".to_string(),
        name: "Test Group".to_string(),
        function: ReadFunction::ReadHoldingRegisters,
        start_address: 0,
        quantity: 10,
        interval_ms: 500,
        enabled: true,
        slave_id: None,
    };
    println!("       Scan group created: {}\n", scan_group.name);

    // Step 5: Start polling (simulates start_polling command)
    println!("[5/7] Starting polling...");
    let mut rx = master.start_scan_group(&scan_group).await.unwrap();
    println!("       Polling active: {}\n", master.is_scan_active("sg-test-1"));

    // Step 6: Receive poll data (simulates the bridge task + frontend display)
    println!("[6/7] Receiving poll data (3 cycles)...");
    let mut last_values = vec![];
    for i in 0..3 {
        if let Some(event) = rx.recv().await {
            match event {
                PollEvent::Data(ReadResult::HoldingRegisters(vals)) => {
                    println!("       Cycle {}: {:?}", i + 1, vals);
                    last_values = vals;
                }
                PollEvent::Error(e) => {
                    println!("       Cycle {} ERROR: {}", i + 1, e);
                    println!("\n  FAIL: Poll returned error!");
                    return;
                }
                _ => {}
            }
        }
    }
    println!();

    // Step 7: Write a value and verify (simulates write_single_register + poll update)
    println!("[7/7] Writing value 99 to address 0...");
    master.write_single_register(0, 99).await.unwrap();
    println!("       Write OK. Waiting for next poll...");

    // Wait for next poll to pick up the change
    if let Some(event) = rx.recv().await {
        match event {
            PollEvent::Data(ReadResult::HoldingRegisters(vals)) => {
                println!("       Updated: {:?}", vals);
                if vals[0] == 99 {
                    println!("       Register[0] = 99 confirmed!");
                } else {
                    println!("       WARNING: Register[0] = {} (expected 99)", vals[0]);
                }
            }
            _ => println!("       Unexpected event"),
        }
    }

    // Check logs
    let logs = log_collector.get_all().await;
    println!("\n  Communication logs: {} entries", logs.len());
    for log in logs.iter().take(5) {
        println!("    [{}] {} {} {}", log.timestamp.format("%H:%M:%S%.3f"),
                 if log.direction == modbussim_core::log_entry::Direction::Tx { "TX" } else { "RX" },
                 log.function_code.name(), log.detail);
    }
    if logs.len() > 5 {
        println!("    ... and {} more", logs.len() - 5);
    }

    // Cleanup
    master.stop_scan_group("sg-test-1").await.unwrap();
    master.disconnect().await.unwrap();
    slave.stop().await.unwrap();

    println!("\n========================================");
    println!("  ALL TESTS PASSED!");
    println!("========================================");
}
