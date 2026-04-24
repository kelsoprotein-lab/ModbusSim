//! 后台任务 → UI 线程的消息枚举。

use modbussim_core::master::{MasterState, ReadFunction, ReadResult};

pub enum UiEvent {
    ConnectionCreated {
        id: String,
        label: String,
        slave_id: u8,
    },
    ConnectionStateChanged {
        id: String,
        state: MasterState,
    },
    ConnectionRemoved(String),
    ReadDone {
        id: String,
        result: ReadResult,
    },
    PollStarted {
        id: String,
        group_id: String,
    },
    PollStopped {
        id: String,
        group_id: String,
    },
    PollUpdate {
        id: String,
        group_id: String,
        result: ReadResult,
    },
    PollError {
        id: String,
        group_id: String,
        msg: String,
    },
    PollConfigLoaded {
        id: String,
        group_id: String,
        fc: ReadFunction,
        addr: u16,
        qty: u16,
        interval_ms: u64,
    },
    Info(String),
    Error(String),
}
