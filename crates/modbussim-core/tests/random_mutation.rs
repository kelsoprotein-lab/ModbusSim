//! Coverage for SlaveDevice::apply_random_mutation across all four register
//! types. Earlier reports indicated that mutating function codes 3 (holding)
//! and 4 (input) was a no-op while 1 (coil) and 2 (discrete) worked; these
//! tests pin the algorithm so that regression cannot recur silently.

use modbussim_core::register::RegisterType;
use modbussim_core::slave::SlaveDevice;

fn snapshot(device: &SlaveDevice) -> (Vec<bool>, Vec<bool>, Vec<u16>, Vec<u16>) {
    let coils: Vec<bool> = (0..32)
        .map(|a| device.register_map.coils.get(&a).copied().unwrap_or(false))
        .collect();
    let di: Vec<bool> = (0..32)
        .map(|a| {
            device
                .register_map
                .discrete_inputs
                .get(&a)
                .copied()
                .unwrap_or(false)
        })
        .collect();
    let hr: Vec<u16> = (0..32)
        .map(|a| {
            device
                .register_map
                .holding_registers
                .get(&a)
                .copied()
                .unwrap_or(0)
        })
        .collect();
    let ir: Vec<u16> = (0..32)
        .map(|a| {
            device
                .register_map
                .input_registers
                .get(&a)
                .copied()
                .unwrap_or(0)
        })
        .collect();
    (coils, di, hr, ir)
}

#[test]
fn holding_register_actually_changes_after_iterations() {
    let mut device = SlaveDevice::with_default_registers(1, "t", 31);
    let (_c0, _d0, hr0, _i0) = snapshot(&device);
    assert!(hr0.iter().all(|&v| v == 0), "preconditions: all zeros");

    // Run several rounds; cur=0 + delta in [-100,100] yields >0 about half the
    // time, so 10 rounds * >=3 picks practically guarantees a non-zero somewhere.
    let mut total = 0u32;
    for _ in 0..10 {
        total += device.apply_random_mutation_thread(&[RegisterType::HoldingRegister]);
    }
    let (_c, _d, hr, _i) = snapshot(&device);

    assert!(total > 0, "mutation count must be > 0");
    assert!(
        hr.iter().any(|&v| v != 0),
        "at least one holding register must change from 0 across 10 rounds"
    );
}

#[test]
fn input_register_actually_changes_after_iterations() {
    let mut device = SlaveDevice::with_default_registers(1, "t", 31);
    for _ in 0..10 {
        device.apply_random_mutation_thread(&[RegisterType::InputRegister]);
    }
    let (_c, _d, _h, ir) = snapshot(&device);
    assert!(
        ir.iter().any(|&v| v != 0),
        "at least one input register must change from 0 across 10 rounds"
    );
}

#[test]
fn coil_actually_flips() {
    let mut device = SlaveDevice::with_default_registers(1, "t", 31);
    device.apply_random_mutation_thread(&[RegisterType::Coil]);
    let (c, _d, _h, _i) = snapshot(&device);
    assert!(
        c.iter().any(|&b| b),
        "at least one coil should flip to true"
    );
}

#[test]
fn discrete_input_actually_flips() {
    let mut device = SlaveDevice::with_default_registers(1, "t", 31);
    device.apply_random_mutation_thread(&[RegisterType::DiscreteInput]);
    let (_c, d, _h, _i) = snapshot(&device);
    assert!(
        d.iter().any(|&b| b),
        "at least one discrete input should flip"
    );
}

#[test]
fn empty_defs_returns_zero_no_panic() {
    let mut device = SlaveDevice::new(1, "empty");
    let n =
        device.apply_random_mutation_thread(&[RegisterType::Coil, RegisterType::HoldingRegister]);
    assert_eq!(n, 0);
}

#[test]
fn mixed_types_all_change() {
    let mut device = SlaveDevice::with_default_registers(1, "t", 31);
    for _ in 0..10 {
        device.apply_random_mutation_thread(&[
            RegisterType::Coil,
            RegisterType::DiscreteInput,
            RegisterType::HoldingRegister,
            RegisterType::InputRegister,
        ]);
    }
    let (c, d, h, i) = snapshot(&device);
    assert!(c.iter().any(|&v| v), "coil must change");
    assert!(d.iter().any(|&v| v), "discrete input must change");
    assert!(h.iter().any(|&v| v != 0), "holding register must change");
    assert!(i.iter().any(|&v| v != 0), "input register must change");
}
