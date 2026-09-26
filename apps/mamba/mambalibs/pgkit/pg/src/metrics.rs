//! Connection pool metrics and monitoring.
//!
//! This module provides metrics collection, health checks, and export
//! functionality for monitoring PostgreSQL connection pool performance.
//!
//! # Example
//!
//! ```rust,ignore
//! use pg::{Connection, PoolConfig, PoolMetrics, HealthCheck};
//!
//! let conn = Connection::new(&uri, PoolConfig::default()).await?;
//! let metrics = PoolMetrics::from_connection(&conn);
//!
//! // Check health
//! let health = HealthCheck::check(&conn).await?;
//! println!("Pool healthy: {}", health.is_healthy);
//!
//! // Export as Prometheus format
//! println!("{}", metrics.to_prometheus("myapp"));
//! ```

use crate::{Connection, PoolConfig, Result};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::instrument;

/// Snapshot of connection pool metrics at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMetrics {
    /// Current number of connections in the pool.
    pub pool_size: u32,
    /// Number of idle connections available.
    pub num_idle: u32,
    /// Number of active connections in use.
    pub num_active: u32,
    /// Maximum allowed connections (from config).
    pub max_connections: u32,
    /// Minimum connections to maintain (from config).
    pub min_connections: u32,
    /// Pool utilization percentage (0.0 - 1.0).
    pub utilization: f64,
    /// Timestamp when metrics were collected (Unix timestamp).
    pub timestamp: u64,
}

impl PoolMetrics {
    /// Collect current metrics from a connection pool.
    pub fn from_connection(conn: &Connection) -> Self {
        Self::from_connection_with_config(conn, &PoolConfig::default())
    }

    /// Collect current metrics from a connection pool with config.
    pub fn from_connection_with_config(conn: &Connection, config: &PoolConfig) -> Self {
        let pool = conn.pool();
        let pool_size = pool.size();
        let num_idle = pool.num_idle() as u32;
        let num_active = pool_size.saturating_sub(num_idle);
        let max_connections = config.max_connections;

        let utilization = if max_connections > 0 {
            (num_active as f64) / (max_connections as f64)
        } else {
            0.0
        };

        Self {
            pool_size,
            num_idle,
            num_active,
            max_connections,
            min_connections: config.min_connections,
            utilization,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }

    /// Check if the pool is approaching saturation (>80% utilization).
    pub fn is_near_saturation(&self) -> bool {
        self.utilization > 0.8
    }

    /// Check if the pool is saturated (100% utilization).
    pub fn is_saturated(&self) -> bool {
        self.num_active >= self.max_connections
    }

    /// Export metrics in Prometheus text format.
    ///
    /// # Arguments
    ///
    /// * `prefix` - Metric name prefix (e.g., "myapp" produces "myapp_pool_size")
    pub fn to_prometheus(&self, prefix: &str) -> String {
        let mut output = String::new();

        // Pool size
        output.push_str(&format!(
            "# HELP {}_pool_size Current number of connections in the pool\n",
            prefix
        ));
        output.push_str(&format!("# TYPE {}_pool_size gauge\n", prefix));
        output.push_str(&format!("{}_pool_size {}\n", prefix, self.pool_size));

        // Idle connections
        output.push_str(&format!(
            "# HELP {}_pool_idle Number of idle connections\n",
            prefix
        ));
        output.push_str(&format!("# TYPE {}_pool_idle gauge\n", prefix));
        output.push_str(&format!("{}_pool_idle {}\n", prefix, self.num_idle));

        // Active connections
        output.push_str(&format!(
            "# HELP {}_pool_active Number of active connections\n",
            prefix
        ));
        output.push_str(&format!("# TYPE {}_pool_active gauge\n", prefix));
        output.push_str(&format!("{}_pool_active {}\n", prefix, self.num_active));

        // Max connections
        output.push_str(&format!(
            "# HELP {}_pool_max Maximum allowed connections\n",
            prefix
        ));
        output.push_str(&format!("# TYPE {}_pool_max gauge\n", prefix));
        output.push_str(&format!("{}_pool_max {}\n", prefix, self.max_connections));

        // Utilization
        output.push_str(&format!(
            "# HELP {}_pool_utilization Pool utilization ratio (0-1)\n",
            prefix
        ));
        output.push_str(&format!("# TYPE {}_pool_utilization gauge\n", prefix));
        output.push_str(&format!(
            "{}_pool_utilization {:.4}\n",
            prefix, self.utilization
        ));

        output
    }

    /// Export metrics as JSON string.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| crate::DataBridgeError::Query(format!("JSON serialization failed: {}", e)))
    }
}

/// Health status of the connection pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Whether the pool is healthy and operational.
    pub is_healthy: bool,
    /// Whether the database is reachable.
    pub is_connected: bool,
    /// Whether the pool is near saturation (>80%).
    pub is_near_saturation: bool,
    /// Whether the pool is fully saturated.
    pub is_saturated: bool,
    /// Time taken to perform health check in milliseconds.
    pub check_latency_ms: u64,
    /// Error message if unhealthy.
    pub error: Option<String>,
    /// Current pool metrics snapshot.
    pub metrics: PoolMetrics,
}

impl HealthStatus {
    /// Check if all health indicators are good.
    pub fn all_ok(&self) -> bool {
        self.is_healthy && self.is_connected && !self.is_saturated
    }
}

/// Health check utilities for the connection pool.
pub struct HealthCheck;

impl HealthCheck {
    /// Perform a comprehensive health check on the connection pool.
    ///
    /// This checks:
    /// - Database connectivity (ping)
    /// - Pool saturation status
    /// - Connection latency
    #[instrument(skip(conn))]
    pub async fn check(conn: &Connection) -> Result<HealthStatus> {
        Self::check_with_config(conn, &PoolConfig::default()).await
    }

    /// Perform health check with specific config for accurate metrics.
    #[instrument(skip(conn, config))]
    pub async fn check_with_config(conn: &Connection, config: &PoolConfig) -> Result<HealthStatus> {
        let start = Instant::now();

        // Collect current metrics
        let metrics = PoolMetrics::from_connection_with_config(conn, config);

        // Check connectivity
        let ping_result = conn.ping().await;
        let is_connected = ping_result.is_ok();
        let error = ping_result.err().map(|e| e.to_string());

        let check_latency_ms = start.elapsed().as_millis() as u64;

        let is_near_saturation = metrics.is_near_saturation();
        let is_saturated = metrics.is_saturated();

        // Pool is healthy if connected and not saturated
        let is_healthy = is_connected && !is_saturated;

        Ok(HealthStatus {
            is_healthy,
            is_connected,
            is_near_saturation,
            is_saturated,
            check_latency_ms,
            error,
            metrics,
        })
    }

    /// Quick connectivity check (ping only).
    #[instrument(skip(conn))]
    pub async fn ping(conn: &Connection) -> Result<Duration> {
        let start = Instant::now();
        conn.ping().await?;
        Ok(start.elapsed())
    }
}

/// Latency statistics for connection operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyStats {
    /// Number of samples collected.
    pub count: u64,
    /// Minimum latency in microseconds.
    pub min_us: u64,
    /// Maximum latency in microseconds.
    pub max_us: u64,
    /// Sum of all latencies for average calculation.
    pub sum_us: u64,
    /// P50 (median) latency estimate.
    pub p50_us: u64,
    /// P95 latency estimate.
    pub p95_us: u64,
    /// P99 latency estimate.
    pub p99_us: u64,
}

impl LatencyStats {
    /// Create new empty stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate average latency.
    pub fn avg_us(&self) -> u64 {
        if self.count == 0 {
            0
        } else {
            self.sum_us / self.count
        }
    }

    /// Record a latency sample.
    pub fn record(&mut self, latency: Duration) {
        let us = latency.as_micros() as u64;
        self.count += 1;
        self.sum_us += us;

        if self.count == 1 {
            self.min_us = us;
            self.max_us = us;
            self.p50_us = us;
            self.p95_us = us;
            self.p99_us = us;
        } else {
            self.min_us = self.min_us.min(us);
            self.max_us = self.max_us.max(us);
            // Simple exponential moving average for percentile estimates
            self.p50_us = (self.p50_us * 9 + us) / 10;
            self.p95_us = (self.p95_us * 19 + us) / 20;
            self.p99_us = (self.p99_us * 99 + us) / 100;
        }
    }

    /// Reset all statistics.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Metrics collector that tracks pool statistics over time.
#[derive(Debug)]
pub struct MetricsCollector {
    /// Connection acquire latency stats.
    pub acquire_latency: LatencyStats,
    /// Query execution latency stats.
    pub query_latency: LatencyStats,
    /// Total successful connection acquires.
    pub acquire_success: u64,
    /// Total failed connection acquires.
    pub acquire_failures: u64,
    /// Total queries executed.
    pub queries_executed: u64,
    /// Total query failures.
    pub query_failures: u64,
    /// When collection started.
    pub start_time: Instant,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// Create a new metrics collector.
    pub fn new() -> Self {
        Self {
            acquire_latency: LatencyStats::new(),
            query_latency: LatencyStats::new(),
            acquire_success: 0,
            acquire_failures: 0,
            queries_executed: 0,
            query_failures: 0,
            start_time: Instant::now(),
        }
    }

    /// Record a successful connection acquire.
    pub fn record_acquire_success(&mut self, latency: Duration) {
        self.acquire_success += 1;
        self.acquire_latency.record(latency);
    }

    /// Record a failed connection acquire.
    pub fn record_acquire_failure(&mut self) {
        self.acquire_failures += 1;
    }

    /// Record a successful query execution.
    pub fn record_query_success(&mut self, latency: Duration) {
        self.queries_executed += 1;
        self.query_latency.record(latency);
    }

    /// Record a failed query execution.
    pub fn record_query_failure(&mut self) {
        self.query_failures += 1;
    }

    /// Get uptime since collection started.
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Calculate queries per second.
    pub fn queries_per_second(&self) -> f64 {
        let uptime_secs = self.uptime().as_secs_f64();
        if uptime_secs > 0.0 {
            self.queries_executed as f64 / uptime_secs
        } else {
            0.0
        }
    }

    /// Calculate acquire success rate.
    pub fn acquire_success_rate(&self) -> f64 {
        let total = self.acquire_success + self.acquire_failures;
        if total > 0 {
            self.acquire_success as f64 / total as f64
        } else {
            1.0 // No failures if no attempts
        }
    }

    /// Calculate query success rate.
    pub fn query_success_rate(&self) -> f64 {
        let total = self.queries_executed + self.query_failures;
        if total > 0 {
            self.queries_executed as f64 / total as f64
        } else {
            1.0 // No failures if no attempts
        }
    }

    /// Export collector stats as Prometheus format.
    pub fn to_prometheus(&self, prefix: &str) -> String {
        let mut output = String::new();

        // Acquire stats
        output.push_str(&format!(
            "# HELP {}_acquire_total Total connection acquire attempts\n",
            prefix
        ));
        output.push_str(&format!("# TYPE {}_acquire_total counter\n", prefix));
        output.push_str(&format!(
            "{}_acquire_total{{status=\"success\"}} {}\n",
            prefix, self.acquire_success
        ));
        output.push_str(&format!(
            "{}_acquire_total{{status=\"failure\"}} {}\n",
            prefix, self.acquire_failures
        ));

        // Query stats
        output.push_str(&format!(
            "# HELP {}_queries_total Total queries executed\n",
            prefix
        ));
        output.push_str(&format!("# TYPE {}_queries_total counter\n", prefix));
        output.push_str(&format!(
            "{}_queries_total{{status=\"success\"}} {}\n",
            prefix, self.queries_executed
        ));
        output.push_str(&format!(
            "{}_queries_total{{status=\"failure\"}} {}\n",
            prefix, self.query_failures
        ));

        // Latency stats
        if self.acquire_latency.count > 0 {
            output.push_str(&format!(
                "# HELP {}_acquire_latency_us Acquire latency in microseconds\n",
                prefix
            ));
            output.push_str(&format!("# TYPE {}_acquire_latency_us summary\n", prefix));
            output.push_str(&format!(
                "{}_acquire_latency_us{{quantile=\"0.5\"}} {}\n",
                prefix, self.acquire_latency.p50_us
            ));
            output.push_str(&format!(
                "{}_acquire_latency_us{{quantile=\"0.95\"}} {}\n",
                prefix, self.acquire_latency.p95_us
            ));
            output.push_str(&format!(
                "{}_acquire_latency_us{{quantile=\"0.99\"}} {}\n",
                prefix, self.acquire_latency.p99_us
            ));
        }

        if self.query_latency.count > 0 {
            output.push_str(&format!(
                "# HELP {}_query_latency_us Query latency in microseconds\n",
                prefix
            ));
            output.push_str(&format!("# TYPE {}_query_latency_us summary\n", prefix));
            output.push_str(&format!(
                "{}_query_latency_us{{quantile=\"0.5\"}} {}\n",
                prefix, self.query_latency.p50_us
            ));
            output.push_str(&format!(
                "{}_query_latency_us{{quantile=\"0.95\"}} {}\n",
                prefix, self.query_latency.p95_us
            ));
            output.push_str(&format!(
                "{}_query_latency_us{{quantile=\"0.99\"}} {}\n",
                prefix, self.query_latency.p99_us
            ));
        }

        // Uptime
        output.push_str(&format!(
            "# HELP {}_uptime_seconds Seconds since metrics collection started\n",
            prefix
        ));
        output.push_str(&format!("# TYPE {}_uptime_seconds counter\n", prefix));
        output.push_str(&format!(
            "{}_uptime_seconds {}\n",
            prefix,
            self.uptime().as_secs()
        ));

        output
    }

    /// Reset all collected statistics.
    pub fn reset(&mut self) {
        self.acquire_latency.reset();
        self.query_latency.reset();
        self.acquire_success = 0;
        self.acquire_failures = 0;
        self.queries_executed = 0;
        self.query_failures = 0;
        self.start_time = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_metrics_utilization() {
        let metrics = PoolMetrics {
            pool_size: 10,
            num_idle: 1,
            num_active: 9,
            max_connections: 10,
            min_connections: 1,
            utilization: 0.9, // > 0.8 threshold
            timestamp: 0,
        };

        assert!(metrics.is_near_saturation());
        assert!(!metrics.is_saturated());
    }

    #[test]
    fn test_pool_metrics_saturated() {
        let metrics = PoolMetrics {
            pool_size: 10,
            num_idle: 0,
            num_active: 10,
            max_connections: 10,
            min_connections: 1,
            utilization: 1.0,
            timestamp: 0,
        };

        assert!(metrics.is_near_saturation());
        assert!(metrics.is_saturated());
    }

    #[test]
    fn test_pool_metrics_prometheus_format() {
        let metrics = PoolMetrics {
            pool_size: 5,
            num_idle: 3,
            num_active: 2,
            max_connections: 10,
            min_connections: 1,
            utilization: 0.2,
            timestamp: 0,
        };

        let prom = metrics.to_prometheus("test");
        assert!(prom.contains("test_pool_size 5"));
        assert!(prom.contains("test_pool_idle 3"));
        assert!(prom.contains("test_pool_active 2"));
        assert!(prom.contains("test_pool_max 10"));
        assert!(prom.contains("test_pool_utilization 0.2000"));
    }

    #[test]
    fn test_pool_metrics_json_export() {
        let metrics = PoolMetrics {
            pool_size: 5,
            num_idle: 3,
            num_active: 2,
            max_connections: 10,
            min_connections: 1,
            utilization: 0.2,
            timestamp: 12345,
        };

        let json = metrics.to_json().unwrap();
        assert!(json.contains("\"pool_size\": 5"));
        assert!(json.contains("\"num_idle\": 3"));
    }

    #[test]
    fn test_latency_stats_recording() {
        let mut stats = LatencyStats::new();

        stats.record(Duration::from_micros(100));
        assert_eq!(stats.count, 1);
        assert_eq!(stats.min_us, 100);
        assert_eq!(stats.max_us, 100);

        stats.record(Duration::from_micros(200));
        assert_eq!(stats.count, 2);
        assert_eq!(stats.min_us, 100);
        assert_eq!(stats.max_us, 200);

        stats.record(Duration::from_micros(50));
        assert_eq!(stats.count, 3);
        assert_eq!(stats.min_us, 50);
        assert_eq!(stats.max_us, 200);
    }

    #[test]
    fn test_latency_stats_average() {
        let mut stats = LatencyStats::new();

        stats.record(Duration::from_micros(100));
        stats.record(Duration::from_micros(200));
        stats.record(Duration::from_micros(300));

        // Sum = 600, Count = 3, Avg = 200
        assert_eq!(stats.avg_us(), 200);
    }

    #[test]
    fn test_latency_stats_reset() {
        let mut stats = LatencyStats::new();
        stats.record(Duration::from_micros(100));
        stats.record(Duration::from_micros(200));

        stats.reset();

        assert_eq!(stats.count, 0);
        assert_eq!(stats.sum_us, 0);
        assert_eq!(stats.min_us, 0);
        assert_eq!(stats.max_us, 0);
    }

    #[test]
    fn test_metrics_collector_acquire() {
        let mut collector = MetricsCollector::new();

        collector.record_acquire_success(Duration::from_millis(10));
        collector.record_acquire_success(Duration::from_millis(20));
        collector.record_acquire_failure();

        assert_eq!(collector.acquire_success, 2);
        assert_eq!(collector.acquire_failures, 1);
        assert_eq!(collector.acquire_latency.count, 2);

        // Success rate: 2/3 = 0.666...
        assert!((collector.acquire_success_rate() - 0.6666).abs() < 0.01);
    }

    #[test]
    fn test_metrics_collector_query() {
        let mut collector = MetricsCollector::new();

        collector.record_query_success(Duration::from_millis(5));
        collector.record_query_success(Duration::from_millis(10));
        collector.record_query_success(Duration::from_millis(15));
        collector.record_query_failure();

        assert_eq!(collector.queries_executed, 3);
        assert_eq!(collector.query_failures, 1);

        // Success rate: 3/4 = 0.75
        assert!((collector.query_success_rate() - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_metrics_collector_prometheus() {
        let mut collector = MetricsCollector::new();
        collector.record_acquire_success(Duration::from_millis(10));
        collector.record_query_success(Duration::from_millis(5));

        let prom = collector.to_prometheus("test");
        assert!(prom.contains("test_acquire_total"));
        assert!(prom.contains("test_queries_total"));
        assert!(prom.contains("test_acquire_latency_us"));
        assert!(prom.contains("test_query_latency_us"));
    }

    #[test]
    fn test_metrics_collector_reset() {
        let mut collector = MetricsCollector::new();
        collector.record_acquire_success(Duration::from_millis(10));
        collector.record_query_success(Duration::from_millis(5));

        collector.reset();

        assert_eq!(collector.acquire_success, 0);
        assert_eq!(collector.queries_executed, 0);
        assert_eq!(collector.acquire_latency.count, 0);
        assert_eq!(collector.query_latency.count, 0);
    }

    #[test]
    fn test_health_status_all_ok() {
        let metrics = PoolMetrics {
            pool_size: 5,
            num_idle: 3,
            num_active: 2,
            max_connections: 10,
            min_connections: 1,
            utilization: 0.2,
            timestamp: 0,
        };

        let status = HealthStatus {
            is_healthy: true,
            is_connected: true,
            is_near_saturation: false,
            is_saturated: false,
            check_latency_ms: 5,
            error: None,
            metrics,
        };

        assert!(status.all_ok());
    }

    #[test]
    fn test_health_status_not_ok_when_saturated() {
        let metrics = PoolMetrics {
            pool_size: 10,
            num_idle: 0,
            num_active: 10,
            max_connections: 10,
            min_connections: 1,
            utilization: 1.0,
            timestamp: 0,
        };

        let status = HealthStatus {
            is_healthy: false,
            is_connected: true,
            is_near_saturation: true,
            is_saturated: true,
            check_latency_ms: 5,
            error: None,
            metrics,
        };

        assert!(!status.all_ok());
    }
}
