//! 主站 UI 侧的一个扫描组（映射到 core::ScanGroup）。

use std::time::Instant;

use modbussim_core::master::{ReadFunction, ReadResult};

#[derive(Clone)]
pub struct ScanGroupUi {
    pub id: String,
    pub name: String,
    pub fc: ReadFunction,
    pub addr: u16,
    pub qty: u16,
    pub interval_ms: u64,
    pub enabled: bool,
    pub latest: Option<ReadResult>,
    pub last_update: Option<Instant>,
    pub last_error: Option<String>,
}

impl ScanGroupUi {
    pub fn new_with_id(id: String) -> Self {
        Self {
            name: id.clone(),
            id,
            fc: ReadFunction::ReadHoldingRegisters,
            addr: 0,
            qty: 10,
            interval_ms: 500,
            enabled: false,
            latest: None,
            last_update: None,
            last_error: None,
        }
    }
}
