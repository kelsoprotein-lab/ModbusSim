//! 端到端联动测试：同进程内启动 core slave + master，跑完整交互流程。
//!
//! - 完全非交互：`cargo test --test e2e_flow` 直接跑通/失败。
//! - 每个关键节点打 `[E2E <ms>] [<stage>] ...` 日志，`-- --nocapture` 可直接读。
//! - assert 失败时 stderr 日志也会打出来，便于定位在哪一步坏。
//!
//! 运行：
//! ```bash
//! cargo test --test e2e_flow -- --nocapture
//! ```

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use modbussim_core::master::{
    MasterConfig, MasterConnection, MasterState, PollEvent, ReadFunction, ReadResult, ScanGroup,
};
use modbussim_core::slave::{SlaveConnection, SlaveDevice};
use modbussim_core::transport::Transport;

/// 统一日志：`[E2E 12345ms] [stage       ] msg`。
/// 使用 stderr，test runner 不 capture stderr 时也能看到；`-- --nocapture` 两者都会打。
macro_rules! step {
    ($stage:expr, $($arg:tt)*) => {{
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        eprintln!("[E2E {:>12}ms] [{:<14}] {}", ts, $stage, format!($($arg)*));
    }}
}

fn now() -> Instant {
    Instant::now()
}

fn make_master(port: u16, timeout_ms: u64) -> MasterConnection {
    let config = MasterConfig {
        target_address: "127.0.0.1".to_string(),
        port,
        slave_id: 1,
        timeout_ms,
        ..Default::default()
    };
    let transport = Transport::Tcp {
        host: config.target_address.clone(),
        port: config.port,
    };
    MasterConnection::new(config, transport)
}

/// 起一个本地 slave：slave_id=1，预填 0..=50 的四种寄存器。
async fn make_slave(port: u16) -> SlaveConnection {
    let transport = Transport::Tcp {
        host: "127.0.0.1".to_string(),
        port,
    };
    let mut conn = SlaveConnection::new(transport);

    let mut device = SlaveDevice::new(1, "E2E-Device");
    for addr in 0..=50u16 {
        device.register_map.holding_registers.insert(addr, 0);
        device.register_map.input_registers.insert(addr, 0);
        device.register_map.coils.insert(addr, false);
        device.register_map.discrete_inputs.insert(addr, false);
    }
    // 固定的种子数据
    device.register_map.write_holding_register(0, 1111);
    device.register_map.write_holding_register(1, 2222);
    device.register_map.write_holding_register(2, 3333);
    device.register_map.write_coil(0, true);
    device.register_map.write_coil(1, false);
    device.register_map.write_coil(2, true);
    device.register_map.discrete_inputs.insert(0, true);
    device.register_map.input_registers.insert(5, 9999);

    conn.add_device(device).await.unwrap();
    conn.start().await.unwrap();
    // 给 server 一点点时间起来。
    tokio::time::sleep(Duration::from_millis(80)).await;
    conn
}

/// 场景 1：完整读/写/轮询链路。
///
/// 流程：启动 slave → master connect → FC03/01/02/04 全读 → FC06/05/16/15 全写
/// → 读回校验 → 启动 scan group 跑 3 轮 → stop → disconnect → slave.stop。
#[tokio::test]
async fn e2e_full_master_slave_flow() {
    let port = 17701;
    step!("SETUP", "spawn slave @127.0.0.1:{}", port);
    let mut slave = make_slave(port).await;
    step!("SETUP", "slave up, state={:?}", slave.state());

    let mut master = make_master(port, 3000);
    step!("MASTER", "pre-connect state={:?}", master.state());

    let t0 = now();
    master.connect().await.expect("connect");
    step!(
        "MASTER",
        "connected in {}ms, state={:?}",
        t0.elapsed().as_millis(),
        master.state()
    );
    assert_eq!(master.state(), MasterState::Connected);

    // ---- 读路径 --------------------------------------------------------
    let r = master
        .read(ReadFunction::ReadHoldingRegisters, 0, 3)
        .await
        .expect("read FC03");
    step!("READ FC03", "addr=0 qty=3 -> {:?}", r);
    match r {
        ReadResult::HoldingRegisters(vs) => assert_eq!(vs, vec![1111, 2222, 3333]),
        other => panic!("FC03 unexpected: {other:?}"),
    }

    let r = master
        .read(ReadFunction::ReadCoils, 0, 3)
        .await
        .expect("read FC01");
    step!("READ FC01", "addr=0 qty=3 -> {:?}", r);
    match r {
        ReadResult::Coils(vs) => assert_eq!(vs, vec![true, false, true]),
        other => panic!("FC01 unexpected: {other:?}"),
    }

    let r = master
        .read(ReadFunction::ReadDiscreteInputs, 0, 1)
        .await
        .expect("read FC02");
    step!("READ FC02", "addr=0 qty=1 -> {:?}", r);
    match r {
        ReadResult::DiscreteInputs(vs) => assert_eq!(vs, vec![true]),
        other => panic!("FC02 unexpected: {other:?}"),
    }

    let r = master
        .read(ReadFunction::ReadInputRegisters, 5, 1)
        .await
        .expect("read FC04");
    step!("READ FC04", "addr=5 qty=1 -> {:?}", r);
    match r {
        ReadResult::InputRegisters(vs) => assert_eq!(vs, vec![9999]),
        other => panic!("FC04 unexpected: {other:?}"),
    }

    // ---- 写路径 --------------------------------------------------------
    master
        .write_single_register(10, 4242)
        .await
        .expect("write FC06");
    step!("WRITE FC06", "addr=10 val=4242 ok");

    master
        .write_single_coil(10, true)
        .await
        .expect("write FC05");
    step!("WRITE FC05", "addr=10 val=true ok");

    master
        .write_multiple_registers(20, &[7, 8, 9])
        .await
        .expect("write FC16");
    step!("WRITE FC16", "addr=20..22 vals=[7,8,9] ok");

    master
        .write_multiple_coils(20, &[true, false, true])
        .await
        .expect("write FC15");
    step!("WRITE FC15", "addr=20..22 vals=[T,F,T] ok");

    // ---- 回读校验 ------------------------------------------------------
    let r = master
        .read(ReadFunction::ReadHoldingRegisters, 10, 1)
        .await
        .unwrap();
    match r {
        ReadResult::HoldingRegisters(vs) => {
            step!("VERIFY FC06", "HR[10] = {:?}", vs);
            assert_eq!(vs, vec![4242]);
        }
        _ => unreachable!(),
    }
    let r = master.read(ReadFunction::ReadCoils, 10, 1).await.unwrap();
    match r {
        ReadResult::Coils(vs) => {
            step!("VERIFY FC05", "COIL[10] = {:?}", vs);
            assert_eq!(vs, vec![true]);
        }
        _ => unreachable!(),
    }
    let r = master
        .read(ReadFunction::ReadHoldingRegisters, 20, 3)
        .await
        .unwrap();
    match r {
        ReadResult::HoldingRegisters(vs) => {
            step!("VERIFY FC16", "HR[20..22] = {:?}", vs);
            assert_eq!(vs, vec![7, 8, 9]);
        }
        _ => unreachable!(),
    }
    let r = master.read(ReadFunction::ReadCoils, 20, 3).await.unwrap();
    match r {
        ReadResult::Coils(vs) => {
            step!("VERIFY FC15", "COIL[20..22] = {:?}", vs);
            assert_eq!(vs, vec![true, false, true]);
        }
        _ => unreachable!(),
    }

    // ---- Scan group 轮询 ----------------------------------------------
    let group = ScanGroup {
        id: "g1".to_string(),
        name: "g1".to_string(),
        function: ReadFunction::ReadHoldingRegisters,
        start_address: 0,
        quantity: 3,
        interval_ms: 100,
        enabled: true,
        slave_id: None,
    };
    step!(
        "SCAN START",
        "group={} intv={}ms",
        group.id,
        group.interval_ms
    );
    let mut rx = master.start_scan_group(&group).await.expect("start_scan");
    assert!(master.is_scan_active("g1"));

    let mut data_count = 0u32;
    let deadline = Instant::now() + Duration::from_secs(2);
    while data_count < 3 && Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(800), rx.recv()).await {
            Ok(Some(PollEvent::Data(ReadResult::HoldingRegisters(vs)))) => {
                data_count += 1;
                step!("SCAN DATA", "#{} values={:?}", data_count, vs);
                assert_eq!(vs, vec![1111, 2222, 3333]);
            }
            Ok(Some(PollEvent::Error(e))) => panic!("scan error: {e}"),
            Ok(Some(other)) => panic!("unexpected scan event: {other:?}"),
            Ok(None) => panic!("scan channel closed early"),
            Err(_) => panic!("scan timeout waiting for PollEvent::Data"),
        }
    }
    assert!(
        data_count >= 3,
        "expected >=3 scan updates, got {data_count}"
    );

    master.stop_scan_group("g1").await.expect("stop_scan");
    step!("SCAN STOP", "g1 stopped");

    // ---- Teardown ------------------------------------------------------
    master.disconnect().await.expect("disconnect");
    step!("MASTER", "disconnected state={:?}", master.state());
    slave.stop().await.expect("slave stop");
    step!("SLAVE", "stopped state={:?}", slave.state());
    step!("DONE", "full flow OK — {} scan updates", data_count);
}

/// 场景 2：slave 端寄存器变动 → master scan group 下一轮能观察到新值。
///
/// 验证"主站轮询是真轮询",不是缓存。
#[tokio::test]
async fn e2e_scan_group_detects_slave_mutation() {
    let port = 17702;
    step!("SETUP", "spawn slave @127.0.0.1:{}", port);
    let slave = make_slave(port).await;
    let mut master = make_master(port, 3000);
    master.connect().await.expect("connect");
    step!("MASTER", "connected");

    let group = ScanGroup {
        id: "gm".to_string(),
        name: "gm".to_string(),
        function: ReadFunction::ReadHoldingRegisters,
        start_address: 0,
        quantity: 1,
        interval_ms: 80,
        enabled: true,
        slave_id: None,
    };
    let mut rx = master.start_scan_group(&group).await.unwrap();
    step!("SCAN START", "gm @addr=0 qty=1 intv=80ms");

    // 第一次:基线,应为 1111
    let first = tokio::time::timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("first poll timeout")
        .expect("rx closed");
    match first {
        PollEvent::Data(ReadResult::HoldingRegisters(vs)) => {
            step!("SCAN BASELINE", "HR[0]={:?}", vs);
            assert_eq!(vs, vec![1111]);
        }
        other => panic!("unexpected first event: {other:?}"),
    }

    // 外部直接改 slave device 的 HR[0]
    {
        let mut devices = slave.devices.write().await;
        let d = devices.get_mut(&1).unwrap();
        d.register_map.write_holding_register(0, 5555);
        step!("SLAVE MUTATE", "HR[0] 1111 -> 5555");
    }

    // 在接下来几轮里一定能看到 5555
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut saw_new = false;
    let mut rounds = 0u32;
    while Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Some(PollEvent::Data(ReadResult::HoldingRegisters(vs)))) => {
                rounds += 1;
                step!("SCAN ROUND", "#{} HR[0]={:?}", rounds, vs);
                if vs == vec![5555] {
                    saw_new = true;
                    break;
                }
            }
            Ok(Some(PollEvent::Error(e))) => panic!("scan error: {e}"),
            Ok(Some(_)) => {}
            Ok(None) => panic!("rx closed"),
            Err(_) => panic!("scan round timeout"),
        }
    }
    assert!(
        saw_new,
        "mutation was not observed within deadline (rounds={rounds})"
    );
    step!("VERIFY", "mutation observed after {} rounds", rounds);

    master.stop_scan_group("gm").await.unwrap();
    master.disconnect().await.unwrap();
    // slave 离开作用域即 drop，但显式 stop 更干净
    let mut slave = slave;
    slave.stop().await.ok();
    step!("DONE", "mutation-detection flow OK");
}

/// 场景 3：读不存在的地址应触发 Modbus IllegalDataAddress 异常（错误路径）。
#[tokio::test]
async fn e2e_illegal_address_surfaces_error() {
    let port = 17703;
    let mut slave = make_slave(port).await;
    let mut master = make_master(port, 2000);
    master.connect().await.unwrap();
    step!("MASTER", "connected");

    // 地址 999 没有预填 → slave 会返回 IllegalDataAddress
    let res = master
        .read(ReadFunction::ReadHoldingRegisters, 999, 1)
        .await;
    step!("READ OOB", "addr=999 -> {:?}", res);
    assert!(
        res.is_err(),
        "expected error on illegal address, got {res:?}"
    );

    master.disconnect().await.ok();
    slave.stop().await.ok();
    step!("DONE", "illegal-address flow OK");
}
