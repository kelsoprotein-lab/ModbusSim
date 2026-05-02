use crate::log_entry::{Direction, LogEntry};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

/// Maximum number of log entries to keep in memory.
const MAX_LOG_ENTRIES: usize = 10000;

/// Callback invoked whenever an entry is appended. Receives a reference so
/// the closure can clone only the fields it needs (for Tauri emit, etc.).
/// The callback runs synchronously while the write lock is held — keep it
/// fast and non-blocking.
pub type LogAppendCallback = Arc<dyn Fn(&LogEntry) + Send + Sync>;

/// A thread-safe communication log collector.
///
/// Collects Modbus communication events from slave and master engines,
/// maintaining a ring buffer of up to 10000 entries.
#[derive(Clone)]
pub struct LogCollector {
    entries: Arc<RwLock<VecDeque<LogEntry>>>,
    on_append: Arc<Mutex<Option<LogAppendCallback>>>,
}

impl std::fmt::Debug for LogCollector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogCollector")
            .field("entries", &self.entries)
            .field("on_append", &"<callback>")
            .finish()
    }
}

impl Default for LogCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl LogCollector {
    /// Create a new empty log collector.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::new())),
            on_append: Arc::new(Mutex::new(None)),
        }
    }

    /// Install (or replace) the on-append callback. Safe to call any time.
    pub fn set_append_callback(&self, cb: LogAppendCallback) {
        if let Ok(mut slot) = self.on_append.lock() {
            *slot = Some(cb);
        }
    }

    /// Snapshot the current callback (cheap Arc clone) so we don't need to
    /// hold the on_append lock across the entries lock.
    fn snapshot_callback(&self) -> Option<LogAppendCallback> {
        self.on_append.lock().ok().and_then(|g| g.clone())
    }

    fn push_entry(entries: &mut VecDeque<LogEntry>, entry: LogEntry) {
        if entries.len() >= MAX_LOG_ENTRIES {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    /// Add a log entry.
    ///
    /// If the buffer exceeds MAX_LOG_ENTRIES, the oldest entry is removed.
    pub async fn add(&self, entry: LogEntry) {
        let cb = self.snapshot_callback();
        let mut entries = self.entries.write().await;
        if let Some(cb) = cb {
            let snap = entry.clone();
            Self::push_entry(&mut entries, entry);
            drop(entries);
            cb(&snap);
        } else {
            Self::push_entry(&mut entries, entry);
        }
    }

    /// Add a log entry (blocking version).
    /// WARNING: panics if called from within an async tokio runtime.
    /// Use `try_add` for sync contexts within an async runtime.
    pub fn add_blocking(&self, entry: LogEntry) {
        let cb = self.snapshot_callback();
        let mut entries = self.entries.blocking_write();
        if let Some(cb) = cb {
            let snap = entry.clone();
            Self::push_entry(&mut entries, entry);
            drop(entries);
            cb(&snap);
        } else {
            Self::push_entry(&mut entries, entry);
        }
    }

    /// Add a log entry (non-blocking, safe to call from sync code within async runtime).
    /// Silently drops the entry if the lock cannot be acquired immediately.
    pub fn try_add(&self, entry: LogEntry) {
        let cb = self.snapshot_callback();
        if let Ok(mut entries) = self.entries.try_write() {
            if let Some(cb) = cb {
                let snap = entry.clone();
                Self::push_entry(&mut entries, entry);
                drop(entries);
                cb(&snap);
            } else {
                Self::push_entry(&mut entries, entry);
            }
        }
    }

    /// Get all log entries.
    pub async fn get_all(&self) -> Vec<LogEntry> {
        self.entries.read().await.iter().cloned().collect()
    }

    /// Non-blocking snapshot for sync callers (e.g. the egui render loop).
    /// Returns None if a writer currently holds the lock.
    pub fn try_get_all(&self) -> Option<Vec<LogEntry>> {
        self.entries
            .try_read()
            .ok()
            .map(|g| g.iter().cloned().collect())
    }

    /// Non-blocking count of entries whose timestamp is within the last `window`.
    /// Returns `None` if a writer holds the lock.
    ///
    /// Uses the fact that entries are pushed in time order: scans from the tail
    /// and stops at the first entry older than the cutoff. O(k) where k is the
    /// number of recent entries (typically small even if the buffer is large).
    pub fn try_count_within(&self, window: std::time::Duration) -> Option<usize> {
        let chrono_window = chrono::Duration::from_std(window).ok()?;
        let cutoff = chrono::Utc::now() - chrono_window;
        self.entries.try_read().ok().map(|guard| {
            guard
                .iter()
                .rev()
                .take_while(|e| e.timestamp >= cutoff)
                .count()
        })
    }

    /// Get all log entries (blocking version).
    pub fn get_all_blocking(&self) -> Vec<LogEntry> {
        self.entries.blocking_read().iter().cloned().collect()
    }

    /// Get the most recent `n` entries.
    pub async fn get_recent(&self, n: usize) -> Vec<LogEntry> {
        let entries = self.entries.read().await;
        let start = entries.len().saturating_sub(n);
        entries.iter().skip(start).cloned().collect()
    }

    /// Clear all log entries.
    pub async fn clear(&self) {
        self.entries.write().await.clear();
    }

    /// Clear all log entries (blocking version).
    pub fn clear_blocking(&self) {
        self.entries.blocking_write().clear();
    }

    /// Export all entries to CSV format.
    pub async fn export_csv(&self) -> String {
        let entries = self.entries.read().await;
        let mut output = String::new();
        output.push_str(LogEntry::csv_header());
        output.push('\n');
        for entry in entries.iter() {
            output.push_str(&entry.to_csv_row());
            output.push('\n');
        }
        output
    }

    /// Export all entries to CSV format (blocking version).
    pub fn export_csv_blocking(&self) -> String {
        let entries = self.entries.blocking_read();
        let mut output = String::new();
        output.push_str(LogEntry::csv_header());
        output.push('\n');
        for entry in entries.iter() {
            output.push_str(&entry.to_csv_row());
            output.push('\n');
        }
        output
    }

    /// Export all entries to plain text format.
    pub async fn export_text(&self) -> String {
        let entries = self.entries.read().await;
        let mut output = String::new();
        for entry in entries.iter() {
            let timestamp = entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f");
            let dir = match entry.direction {
                Direction::Rx => "RX",
                Direction::Tx => "TX",
            };
            output.push_str(&format!(
                "[{}] {} {} - {}\n",
                timestamp,
                dir,
                entry.function_code.name(),
                entry.detail
            ));
        }
        output
    }

    /// Export all entries to plain text format (blocking version).
    pub fn export_text_blocking(&self) -> String {
        let entries = self.entries.blocking_read();
        let mut output = String::new();
        for entry in entries.iter() {
            let timestamp = entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f");
            let dir = match entry.direction {
                Direction::Rx => "RX",
                Direction::Tx => "TX",
            };
            output.push_str(&format!(
                "[{}] {} {} - {}\n",
                timestamp,
                dir,
                entry.function_code.name(),
                entry.detail
            ));
        }
        output
    }

    /// Get the current number of entries.
    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Check if the collector is empty.
    pub async fn is_empty(&self) -> bool {
        self.entries.read().await.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log_entry::FunctionCode;

    #[tokio::test]
    async fn test_log_collector_basic() {
        let collector = LogCollector::new();
        assert!(collector.is_empty().await);

        let entry = LogEntry::new(Direction::Rx, FunctionCode::ReadHoldingRegisters, "R 0 x2");
        collector.add(entry).await;

        assert_eq!(collector.len().await, 1);
    }

    #[tokio::test]
    async fn test_log_collector_max_entries() {
        let collector = LogCollector::new();
        let max = MAX_LOG_ENTRIES;

        // Add more than MAX_LOG_ENTRIES
        for i in 0..(max + 100) {
            let entry = LogEntry::new(
                Direction::Rx,
                FunctionCode::ReadHoldingRegisters,
                format!("R {} x1", i),
            );
            collector.add(entry).await;
        }

        // Should be capped at MAX_LOG_ENTRIES
        assert_eq!(collector.len().await, max);

        // First entry should have been removed
        let entries = collector.get_all().await;
        assert!(!entries[0].detail.contains("R 0 x1"));
        assert!(entries[0].detail.contains("R 100 x1"));
    }

    #[tokio::test]
    async fn test_clear() {
        let collector = LogCollector::new();
        let entry = LogEntry::new(Direction::Rx, FunctionCode::ReadHoldingRegisters, "R 0 x2");
        collector.add(entry).await;
        assert_eq!(collector.len().await, 1);

        collector.clear().await;
        assert!(collector.is_empty().await);
    }

    #[tokio::test]
    async fn test_export_csv() {
        let collector = LogCollector::new();
        let entry = LogEntry::new(Direction::Rx, FunctionCode::ReadHoldingRegisters, "R 0 x2");
        collector.add(entry).await;

        let csv = collector.export_csv().await;
        assert!(csv.contains("Timestamp,Direction,Function,Detail,RawBytes"));
        assert!(csv.contains("RX"));
        assert!(csv.contains("FC03"));
        assert!(csv.contains("R 0 x2"));
    }

    #[tokio::test]
    async fn test_export_text() {
        let collector = LogCollector::new();
        let entry = LogEntry::new(
            Direction::Tx,
            FunctionCode::WriteSingleRegister,
            "W 10 = 42",
        );
        collector.add(entry).await;

        let text = collector.export_text().await;
        assert!(text.contains("TX"));
        assert!(text.contains("FC06"));
        assert!(text.contains("W 10 = 42"));
    }

    #[tokio::test]
    async fn test_get_recent() {
        let collector = LogCollector::new();

        for i in 0..10 {
            let entry = LogEntry::new(
                Direction::Rx,
                FunctionCode::ReadHoldingRegisters,
                format!("R {} x1", i),
            );
            collector.add(entry).await;
        }

        let recent = collector.get_recent(3).await;
        assert_eq!(recent.len(), 3);
        // Should be the last 3 entries
        assert!(recent[0].detail.contains("R 7 x1"));
        assert!(recent[2].detail.contains("R 9 x1"));
    }

    #[tokio::test]
    async fn test_try_count_within_recent_only() {
        let collector = LogCollector::new();
        let mut old = LogEntry::new(Direction::Rx, FunctionCode::ReadHoldingRegisters, "old");
        old.timestamp = chrono::Utc::now() - chrono::Duration::seconds(10);
        collector.add(old).await;
        for i in 0..3 {
            collector
                .add(LogEntry::new(
                    Direction::Rx,
                    FunctionCode::ReadHoldingRegisters,
                    format!("new {}", i),
                ))
                .await;
        }
        let count = collector.try_count_within(std::time::Duration::from_secs(1));
        assert_eq!(count, Some(3));
    }

    #[tokio::test]
    async fn test_try_count_within_empty() {
        let collector = LogCollector::new();
        assert_eq!(
            collector.try_count_within(std::time::Duration::from_secs(1)),
            Some(0),
        );
    }
}
