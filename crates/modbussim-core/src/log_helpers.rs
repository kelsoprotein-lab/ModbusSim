use crate::log_collector::LogCollector;
use crate::log_entry::LogEntry;

/// Get all log entries from a collector.
pub async fn get_all_logs(collector: &LogCollector) -> Vec<LogEntry> {
    collector.get_all().await
}

/// Get a paginated slice of log entries.
pub async fn get_logs_paginated(
    collector: &LogCollector,
    offset: usize,
    limit: usize,
) -> Vec<LogEntry> {
    let all = collector.get_all().await;
    all.into_iter().skip(offset).take(limit).collect()
}

/// Export all logs as CSV string.
pub async fn export_csv(collector: &LogCollector) -> String {
    collector.export_csv().await
}

/// Export all logs as plain text string.
pub async fn export_text(collector: &LogCollector) -> String {
    collector.export_text().await
}

/// Clear all log entries.
pub async fn clear_logs(collector: &LogCollector) {
    collector.clear().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log_entry::{Direction, FunctionCode};
    use chrono::Utc;

    fn make_entry(detail: &str) -> LogEntry {
        LogEntry {
            timestamp: Utc::now(),
            direction: Direction::Tx,
            function_code: FunctionCode::ReadHoldingRegisters,
            detail: detail.to_string(),
            raw_bytes: None,
        }
    }

    #[tokio::test]
    async fn test_get_all_logs_empty() {
        let collector = LogCollector::new();
        let logs = get_all_logs(&collector).await;
        assert!(logs.is_empty());
    }

    #[tokio::test]
    async fn test_get_all_logs_with_entries() {
        let collector = LogCollector::new();
        collector.add(make_entry("entry-0")).await;
        collector.add(make_entry("entry-1")).await;

        let logs = get_all_logs(&collector).await;
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].detail, "entry-0");
        assert_eq!(logs[1].detail, "entry-1");
    }

    #[tokio::test]
    async fn test_get_logs_paginated() {
        let collector = LogCollector::new();
        for i in 0..10 {
            collector.add(make_entry(&format!("entry-{}", i))).await;
        }

        let page = get_logs_paginated(&collector, 3, 4).await;
        assert_eq!(page.len(), 4);
        assert_eq!(page[0].detail, "entry-3");
        assert_eq!(page[3].detail, "entry-6");
    }

    #[tokio::test]
    async fn test_get_logs_paginated_beyond_end() {
        let collector = LogCollector::new();
        collector.add(make_entry("only-entry")).await;

        let page = get_logs_paginated(&collector, 5, 10).await;
        assert!(page.is_empty());
    }

    #[tokio::test]
    async fn test_export_csv_contains_header() {
        let collector = LogCollector::new();
        collector.add(make_entry("csv-detail")).await;

        let csv = export_csv(&collector).await;
        assert!(csv.starts_with("Timestamp,"));
        assert!(csv.contains("csv-detail"));
    }

    #[tokio::test]
    async fn test_export_text_format() {
        let collector = LogCollector::new();
        collector.add(make_entry("text-detail")).await;

        let text = export_text(&collector).await;
        assert!(text.contains("TX"));
        assert!(text.contains("FC03"));
        assert!(text.contains("text-detail"));
    }

    #[tokio::test]
    async fn test_clear_logs() {
        let collector = LogCollector::new();
        collector.add(make_entry("to-clear")).await;
        assert_eq!(get_all_logs(&collector).await.len(), 1);

        clear_logs(&collector).await;
        assert!(get_all_logs(&collector).await.is_empty());
    }
}
