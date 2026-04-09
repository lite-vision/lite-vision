use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

impl From<tracing::Level> for LogLevel {
    fn from(level: tracing::Level) -> Self {
        match level {
            tracing::Level::DEBUG => LogLevel::Debug,
            tracing::Level::INFO => LogLevel::Info,
            tracing::Level::WARN => LogLevel::Warn,
            tracing::Level::ERROR => LogLevel::Error,
            _ => LogLevel::Info,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp_ms: u64,
    pub level: LogLevel,
    pub component: String,
    pub event: String,
    pub block_height: Option<u64>,
    pub job_id: Option<[u8; 32]>,
    pub correlation_id: Option<[u8; 32]>,
    pub message: String,
    pub metadata: HashMap<String, Value>,
}

impl LogEntry {
    pub fn new(level: LogLevel, component: &str, event: &str, message: &str) -> Self {
        Self {
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            level,
            component: component.to_string(),
            event: event.to_string(),
            block_height: None,
            job_id: None,
            correlation_id: None,
            message: message.to_string(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_block_height(mut self, height: u64) -> Self {
        self.block_height = Some(height);
        self
    }

    pub fn with_job_id(mut self, job_id: [u8; 32]) -> Self {
        self.job_id = Some(job_id);
        self
    }

    pub fn with_correlation_id(mut self, correlation_id: [u8; 32]) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    pub fn with_metadata(mut self, key: &str, value: Value) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

pub struct Logger {
    component: String,
    min_level: LogLevel,
}

impl Logger {
    pub fn new(component: &str) -> Self {
        Self {
            component: component.to_string(),
            min_level: LogLevel::Info,
        }
    }

    pub fn with_min_level(mut self, level: LogLevel) -> Self {
        self.min_level = level;
        self
    }

    pub fn debug(&self, event: &str, message: &str) -> LogEntry {
        let entry = LogEntry::new(LogLevel::Debug, &self.component, event, message);
        if self.should_log(&entry) {
            self.emit(&entry);
        }
        entry
    }

    pub fn info(&self, event: &str, message: &str) -> LogEntry {
        let entry = LogEntry::new(LogLevel::Info, &self.component, event, message);
        if self.should_log(&entry) {
            self.emit(&entry);
        }
        entry
    }

    pub fn warn(&self, event: &str, message: &str) -> LogEntry {
        let entry = LogEntry::new(LogLevel::Warn, &self.component, event, message);
        if self.should_log(&entry) {
            self.emit(&entry);
        }
        entry
    }

    pub fn error(&self, event: &str, message: &str) -> LogEntry {
        let entry = LogEntry::new(LogLevel::Error, &self.component, event, message);
        if self.should_log(&entry) {
            self.emit(&entry);
        }
        entry
    }

    fn should_log(&self, entry: &LogEntry) -> bool {
        let level_order = |l: &LogLevel| match l {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Warn => 2,
            LogLevel::Error => 3,
        };
        level_order(&entry.level) >= level_order(&self.min_level)
    }

    fn emit(&self, entry: &LogEntry) {
        println!("{}", entry.to_json());
    }
}

pub fn init_logging() {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(fmt::layer().with_ansi(true))
        .init();
}

pub fn init_structured_logging() {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_ansi(true))
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::new(
            LogLevel::Info,
            "test_component",
            "test_event",
            "Test message",
        );

        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.component, "test_component");
        assert_eq!(entry.event, "test_event");
        assert_eq!(entry.message, "Test message");
    }

    #[test]
    fn test_log_entry_with_options() {
        let entry = LogEntry::new(LogLevel::Info, "test", "event", "message")
            .with_block_height(100)
            .with_job_id([1u8; 32])
            .with_correlation_id([2u8; 32])
            .with_metadata("key", Value::String("value".to_string()));

        assert_eq!(entry.block_height, Some(100));
        assert_eq!(entry.job_id, Some([1u8; 32]));
        assert_eq!(entry.correlation_id, Some([2u8; 32]));
        assert!(entry.metadata.contains_key("key"));
    }

    #[test]
    fn test_log_entry_json_serialization() {
        let entry = LogEntry::new(LogLevel::Info, "test", "event", "message");
        let json = entry.to_json();

        assert!(json.contains("Info"));
        assert!(json.contains("test"));
        assert!(json.contains("event"));
    }

    #[test]
    fn test_logger_level_filter() {
        let logger = Logger::new("test").with_min_level(LogLevel::Warn);

        assert!(logger.should_log(&LogEntry::new(LogLevel::Warn, "test", "e", "m")));
        assert!(logger.should_log(&LogEntry::new(LogLevel::Error, "test", "e", "m")));
        assert!(!logger.should_log(&LogEntry::new(LogLevel::Debug, "test", "e", "m")));
        assert!(!logger.should_log(&LogEntry::new(LogLevel::Info, "test", "e", "m")));
    }
}
