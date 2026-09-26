//! Logger implementation with multiple sinks and context binding.

use crate::sink::{LogLevel, LogRecord, Sink};
use parking_lot::RwLock;
use std::collections::HashMap;

/// The main logger.
pub struct Logger {
    sinks: RwLock<Vec<Box<dyn Sink>>>,
    context: RwLock<HashMap<String, String>>,
    min_level: RwLock<LogLevel>,
}

impl Logger {
    /// Create a new logger with no sinks.
    pub fn new() -> Self {
        Self {
            sinks: RwLock::new(vec![]),
            context: RwLock::new(HashMap::new()),
            min_level: RwLock::new(LogLevel::Debug),
        }
    }

    /// Add a sink.
    pub fn add_sink(&self, sink: Box<dyn Sink>) {
        self.sinks.write().push(sink);
    }

    /// Set global minimum log level.
    pub fn set_level(&self, level: LogLevel) {
        *self.min_level.write() = level;
    }

    /// Bind context key-value pairs.
    pub fn bind(&self, key: &str, value: &str) {
        self.context
            .write()
            .insert(key.to_string(), value.to_string());
    }

    /// Create a child logger with additional context.
    pub fn with_context<'a>(&'a self, extra: HashMap<String, String>) -> BoundLogger<'a> {
        let mut ctx = self.context.read().clone();
        ctx.extend(extra);
        BoundLogger {
            parent: self,
            context: ctx,
        }
    }

    /// Log a message at the given level.
    pub fn log(&self, level: LogLevel, message: &str) {
        let min_level = *self.min_level.read();
        if level < min_level {
            return;
        }
        let record = LogRecord {
            level,
            message: message.to_string(),
            timestamp: chrono::Utc::now(),
            context: self.context.read().clone(),
            module: None,
            file: None,
            line: None,
        };
        let sinks = self.sinks.read();
        for sink in sinks.iter() {
            let _ = sink.write(&record);
        }
    }

    pub fn trace(&self, msg: &str) {
        self.log(LogLevel::Trace, msg);
    }
    pub fn debug(&self, msg: &str) {
        self.log(LogLevel::Debug, msg);
    }
    pub fn info(&self, msg: &str) {
        self.log(LogLevel::Info, msg);
    }
    pub fn warning(&self, msg: &str) {
        self.log(LogLevel::Warning, msg);
    }
    pub fn error(&self, msg: &str) {
        self.log(LogLevel::Error, msg);
    }
    pub fn critical(&self, msg: &str) {
        self.log(LogLevel::Critical, msg);
    }

    /// Get the number of registered sinks.
    pub fn sink_count(&self) -> usize {
        self.sinks.read().len()
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

/// A logger with bound context (returned by logger.bind()).
pub struct BoundLogger<'a> {
    parent: &'a Logger,
    context: HashMap<String, String>,
}

impl<'a> BoundLogger<'a> {
    pub fn log(&self, level: LogLevel, message: &str) {
        let min_level = *self.parent.min_level.read();
        if level < min_level {
            return;
        }
        let record = LogRecord {
            level,
            message: message.to_string(),
            timestamp: chrono::Utc::now(),
            context: self.context.clone(),
            module: None,
            file: None,
            line: None,
        };
        let sinks = self.parent.sinks.read();
        for sink in sinks.iter() {
            let _ = sink.write(&record);
        }
    }

    pub fn info(&self, msg: &str) {
        self.log(LogLevel::Info, msg);
    }
    pub fn debug(&self, msg: &str) {
        self.log(LogLevel::Debug, msg);
    }
    pub fn warning(&self, msg: &str) {
        self.log(LogLevel::Warning, msg);
    }
    pub fn error(&self, msg: &str) {
        self.log(LogLevel::Error, msg);
    }
    pub fn critical(&self, msg: &str) {
        self.log(LogLevel::Critical, msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use parking_lot::Mutex;
    use std::sync::Arc;

    #[derive(Clone, Default)]
    struct RecordingSink {
        records: Arc<Mutex<Vec<LogRecord>>>,
    }

    impl Sink for RecordingSink {
        fn write(&self, record: &LogRecord) -> Result<()> {
            self.records.lock().push(record.clone());
            Ok(())
        }

        fn flush(&self) -> Result<()> {
            Ok(())
        }
    }

    fn recording_sink() -> (Box<dyn Sink>, Arc<Mutex<Vec<LogRecord>>>) {
        let sink = RecordingSink::default();
        let records = sink.records.clone();
        (Box::new(sink), records)
    }

    #[test]
    fn logger_filters_by_level_and_fans_out_context_to_sinks() {
        let logger = Logger::new();
        let (sink_a, records_a) = recording_sink();
        let (sink_b, records_b) = recording_sink();

        logger.add_sink(sink_a);
        logger.add_sink(sink_b);
        logger.bind("request_id", "req-42");
        logger.set_level(LogLevel::Info);

        logger.debug("hidden");
        logger.info("visible");

        for records in [records_a, records_b] {
            let records = records.lock();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].level, LogLevel::Info);
            assert_eq!(records[0].message, "visible");
            assert_eq!(
                records[0].context.get("request_id"),
                Some(&"req-42".to_string())
            );
        }
    }

    #[test]
    fn bound_logger_merges_context_and_reuses_parent_sinks() {
        let logger = Logger::new();
        let (sink, records) = recording_sink();
        logger.add_sink(sink);
        logger.bind("service", "checkout");

        let extra = HashMap::from([
            ("request_id".to_string(), "req-7".to_string()),
            ("user_id".to_string(), "user-1".to_string()),
        ]);
        let bound = logger.with_context(extra);

        logger.bind("late_key", "not-in-bound-snapshot");
        bound.warning("bound message");

        let records = records.lock();
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.level, LogLevel::Warning);
        assert_eq!(record.message, "bound message");
        assert_eq!(record.context.get("service"), Some(&"checkout".to_string()));
        assert_eq!(record.context.get("request_id"), Some(&"req-7".to_string()));
        assert_eq!(record.context.get("user_id"), Some(&"user-1".to_string()));
        assert!(!record.context.contains_key("late_key"));
    }
}
