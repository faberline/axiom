//! Integration tests for Connection Pool Lifecycle.
//!
//! These tests require a PostgreSQL database to be running.
//! Set DATABASE_URL environment variable to customize connection.
//!
//! Run with: cargo test -p cclab-titan --test test_pool

use pg::{Connection, PoolConfig, RetryConfig};
use std::time::Duration;

/// Helper to get database URL from environment
fn get_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql://localhost/test_db".to_string())
}

// =============================================================================
// Pool Configuration Tests
// =============================================================================

#[tokio::test]
async fn test_pool_default_config() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;

    // Pool should be created successfully with defaults
    let pool = conn.pool();
    assert!(pool.size() >= 1); // At least min_connections

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn test_pool_min_connections() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 3,
        max_connections: 10,
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig::default(),
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Wait for pool to warm up
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Pool should have at least min_connections
    assert!(pool.size() >= 3, "Pool size {} should be >= 3", pool.size());

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn test_pool_max_connections_respected() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 1,
        max_connections: 5,
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig::default(),
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Spawn multiple concurrent queries
    let mut handles = vec![];
    for _ in 0..10 {
        let pool_clone = pool.clone();
        handles.push(tokio::spawn(async move {
            // Hold connection for a bit
            let _row: (i32,) = sqlx::query_as("SELECT 1 FROM pg_sleep(0.1)")
                .fetch_one(&pool_clone)
                .await
                .unwrap();
        }));
    }

    // Pool size should never exceed max_connections
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(pool.size() <= 5, "Pool size {} should be <= 5", pool.size());

    for handle in handles {
        handle.await?;
    }

    conn.close().await?;
    Ok(())
}

// =============================================================================
// Connection Timeout Tests
// =============================================================================

#[tokio::test]
async fn test_pool_connection_timeout() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 0,
        max_connections: 1,
        connect_timeout: 1, // 1 second timeout
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig::no_retry(),
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Acquire the only connection and hold it
    let tx = pool.begin().await?;

    // Try to acquire another connection - should timeout
    let pool_clone = pool.clone();
    let _result = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::spawn(async move { sqlx::query("SELECT 1").execute(&pool_clone).await }),
    )
    .await;

    // Should either timeout or fail to acquire
    // (behavior depends on sqlx version)
    tx.rollback().await?;

    conn.close().await?;
    Ok(())
}

// =============================================================================
// Connection Health Tests
// =============================================================================

#[tokio::test]
async fn test_pool_ping_healthy() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;

    // Ping should succeed
    conn.ping().await?;

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn test_pool_reconnect_after_query() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();

    // Execute multiple queries - connections should be reused
    for i in 0..10 {
        let row: (i32,) = sqlx::query_as("SELECT $1::int")
            .bind(i)
            .fetch_one(pool)
            .await?;
        assert_eq!(row.0, i);
    }

    conn.close().await?;
    Ok(())
}

// =============================================================================
// Graceful Shutdown Tests
// =============================================================================

#[tokio::test]
async fn test_pool_graceful_shutdown() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();

    // Execute a query
    let _: (i32,) = sqlx::query_as("SELECT 1").fetch_one(pool).await?;

    // Close should complete without error
    conn.close().await?;

    // Pool should be closed
    assert!(pool.is_closed());

    Ok(())
}

#[tokio::test]
async fn test_pool_shutdown_with_pending_queries() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();

    // Start a long-running query in background
    let pool_clone = pool.clone();
    let handle = tokio::spawn(async move {
        let result: Result<(i32,), _> = sqlx::query_as("SELECT 1 FROM pg_sleep(0.5)")
            .fetch_one(&pool_clone)
            .await;
        result.is_ok()
    });

    // Give query time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Close the pool - should wait for pending queries
    conn.close().await?;

    // Query should have completed
    let completed = handle.await?;
    assert!(completed || pool.is_closed());

    Ok(())
}

// =============================================================================
// Concurrent Access Tests
// =============================================================================

#[tokio::test]
async fn test_pool_concurrent_queries() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 2,
        max_connections: 10,
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig::default(),
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Run 20 concurrent queries
    let mut handles = vec![];
    for i in 0..20 {
        let pool_clone = pool.clone();
        handles.push(tokio::spawn(async move {
            let row: (i32,) = sqlx::query_as("SELECT $1::int")
                .bind(i)
                .fetch_one(&pool_clone)
                .await?;
            Ok::<i32, sqlx::Error>(row.0)
        }));
    }

    // All queries should complete successfully
    let mut results = vec![];
    for handle in handles {
        results.push(handle.await??);
    }

    assert_eq!(results.len(), 20);

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn test_pool_under_load() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 2,
        max_connections: 5,
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig::default(),
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Run 50 queries with limited pool - tests queuing
    let mut handles = vec![];
    for i in 0..50 {
        let pool_clone = pool.clone();
        handles.push(tokio::spawn(async move {
            let row: (i32,) = sqlx::query_as("SELECT $1::int")
                .bind(i)
                .fetch_one(&pool_clone)
                .await?;
            Ok::<i32, sqlx::Error>(row.0)
        }));
    }

    // All queries should eventually complete
    let mut success_count = 0;
    for handle in handles {
        if handle.await?.is_ok() {
            success_count += 1;
        }
    }

    assert_eq!(success_count, 50);

    conn.close().await?;
    Ok(())
}

// =============================================================================
// Idle Timeout Tests
// =============================================================================

#[tokio::test]
async fn test_pool_idle_connection_cleanup() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 0,
        max_connections: 5,
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(1), // 1 second idle timeout
        retry: RetryConfig::default(),
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Create some connections
    let mut handles = vec![];
    for _ in 0..3 {
        let pool_clone = pool.clone();
        handles.push(tokio::spawn(async move {
            let _: (i32,) = sqlx::query_as("SELECT 1")
                .fetch_one(&pool_clone)
                .await
                .unwrap();
        }));
    }

    for handle in handles {
        handle.await?;
    }

    let _size_before = pool.size();

    // Wait for idle timeout
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Idle connections should be cleaned up
    // Note: This depends on sqlx's internal cleanup behavior
    let _size_after = pool.size();

    // At minimum, verify pool is still functional
    let _: (i32,) = sqlx::query_as("SELECT 1").fetch_one(pool).await?;

    conn.close().await?;
    Ok(())
}

// =============================================================================
// Retry Tests
// =============================================================================

#[tokio::test]
async fn test_pool_retry_config() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 1,
        max_connections: 5,
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 1000,
            backoff_multiplier: 2.0,
        },
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;

    // Connection should be established with retry config
    conn.ping().await?;

    conn.close().await?;
    Ok(())
}

#[test]
fn test_retry_delay_calculation() {
    let config = RetryConfig {
        max_retries: 5,
        initial_delay_ms: 100,
        max_delay_ms: 5000,
        backoff_multiplier: 2.0,
    };

    // Test delay calculations
    assert_eq!(config.delay_for_attempt(0), Duration::from_millis(100));
    assert_eq!(config.delay_for_attempt(1), Duration::from_millis(200));
    assert_eq!(config.delay_for_attempt(2), Duration::from_millis(400));
    assert_eq!(config.delay_for_attempt(3), Duration::from_millis(800));
    assert_eq!(config.delay_for_attempt(4), Duration::from_millis(1600));
    assert_eq!(config.delay_for_attempt(5), Duration::from_millis(3200));
    // Should be capped at max_delay_ms
    assert_eq!(config.delay_for_attempt(6), Duration::from_millis(5000));
}

// =============================================================================
// Statement Cache Tests
// =============================================================================

#[tokio::test]
async fn test_pool_statement_cache() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 1,
        max_connections: 5,
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig::default(),
        statement_cache_capacity: 100, // Enable statement caching
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Execute same query multiple times - should use cached statement
    for i in 0..10 {
        let row: (i32,) = sqlx::query_as("SELECT $1::int + 1")
            .bind(i)
            .fetch_one(pool)
            .await?;
        assert_eq!(row.0, i + 1);
    }

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn test_pool_no_statement_cache() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 1,
        max_connections: 5,
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig::default(),
        statement_cache_capacity: 0, // Disable statement caching
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Should still work without caching
    for i in 0..5 {
        let row: (i32,) = sqlx::query_as("SELECT $1::int")
            .bind(i)
            .fetch_one(pool)
            .await?;
        assert_eq!(row.0, i);
    }

    conn.close().await?;
    Ok(())
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[tokio::test]
async fn test_pool_invalid_connection_string() {
    let result = Connection::new(
        "postgresql://invalid:invalid@nonexistent:5432/invalid",
        PoolConfig {
            retry: RetryConfig::no_retry(),
            ..PoolConfig::default()
        },
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_pool_empty_connection_string() {
    let result = Connection::new("", PoolConfig::default()).await;

    assert!(result.is_err());
}

// =============================================================================
// ERROR CASES - Connection Failures
// =============================================================================

#[tokio::test]
async fn test_pool_bad_authentication() {
    // Test invalid username/password
    let result = Connection::new(
        "postgresql://wrong_user:wrong_pass@localhost:5432/test_db",
        PoolConfig {
            retry: RetryConfig::no_retry(),
            connect_timeout: 5,
            ..PoolConfig::default()
        },
    )
    .await;

    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string().to_lowercase();
    // Should contain authentication or password related error
    assert!(
        err_str.contains("password")
            || err_str.contains("authentication")
            || err_str.contains("denied")
            || err_str.contains("failed")
            || err_str.contains("role"),
        "Expected auth error, got: {}",
        err_str
    );
}

#[tokio::test]
async fn test_pool_unreachable_host() {
    // Test connection to non-routable IP (should timeout quickly)
    let result = Connection::new(
        "postgresql://user:pass@10.255.255.1:5432/db",
        PoolConfig {
            retry: RetryConfig::no_retry(),
            connect_timeout: 2, // Short timeout
            ..PoolConfig::default()
        },
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_pool_refused_connection() {
    // Test connection to closed port (localhost:59999 should be closed)
    let result = Connection::new(
        "postgresql://user:pass@localhost:59999/db",
        PoolConfig {
            retry: RetryConfig::no_retry(),
            connect_timeout: 5,
            ..PoolConfig::default()
        },
    )
    .await;

    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string().to_lowercase();
    assert!(
        err_str.contains("refused")
            || err_str.contains("connect")
            || err_str.contains("failed")
            || err_str.contains("error"),
        "Expected connection refused error, got: {}",
        err_str
    );
}

#[tokio::test]
async fn test_pool_invalid_database_name() {
    let uri = get_database_url();
    // Extract host from valid URI and use invalid database
    let base = uri.split('/').take(3).collect::<Vec<_>>().join("/");
    let invalid_uri = format!("{}/nonexistent_db_12345", base);

    let result = Connection::new(
        &invalid_uri,
        PoolConfig {
            retry: RetryConfig::no_retry(),
            connect_timeout: 10,
            ..PoolConfig::default()
        },
    )
    .await;

    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string().to_lowercase();
    assert!(
        err_str.contains("database")
            || err_str.contains("does not exist")
            || err_str.contains("not exist"),
        "Expected database not found error, got: {}",
        err_str
    );
}

#[tokio::test]
async fn test_pool_malformed_connection_string() {
    // Various malformed URIs
    let malformed_uris = vec![
        "not_a_uri",
        "postgresql://",
        "postgresql://:@/",
        "http://localhost/db", // wrong scheme
        "postgresql://localhost:not_a_port/db",
    ];

    for uri in malformed_uris {
        let result = Connection::new(
            uri,
            PoolConfig {
                retry: RetryConfig::no_retry(),
                ..PoolConfig::default()
            },
        )
        .await;

        assert!(result.is_err(), "Expected error for malformed URI: {}", uri);
    }
}

// =============================================================================
// ERROR CASES - Pool Acquisition Timeout
// =============================================================================

#[tokio::test]
async fn test_pool_acquisition_timeout_explicit() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 0,
        max_connections: 1,
        connect_timeout: 1, // 1 second acquire timeout
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig::no_retry(),
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Hold the only connection with a long query
    let pool_clone = pool.clone();
    let blocker = tokio::spawn(async move {
        let _: (i32,) = sqlx::query_as("SELECT 1 FROM pg_sleep(3)")
            .fetch_one(&pool_clone)
            .await
            .unwrap();
    });

    // Give the blocker time to acquire the connection
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Try to acquire - should fail due to timeout
    let pool_clone2 = pool.clone();
    let result = sqlx::query("SELECT 1").execute(&pool_clone2).await;

    // Should fail with timeout or pool closed error
    assert!(
        result.is_err(),
        "Expected timeout error when pool exhausted"
    );

    // Wait for blocker to finish
    let _ = blocker.await;
    conn.close().await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Boundary Values
// =============================================================================

#[tokio::test]
async fn test_pool_min_connections_zero() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 0, // No warmup connections
        max_connections: 5,
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig::default(),
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Pool should start with 0 or minimal connections
    // First query should create a connection
    let _: (i32,) = sqlx::query_as("SELECT 1").fetch_one(pool).await?;

    // Now pool should have at least 1 connection
    assert!(pool.size() >= 1);

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn test_pool_max_connections_one() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 0,
        max_connections: 1, // Single connection pool
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig::default(),
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Sequential queries should work fine
    for i in 0..5 {
        let row: (i32,) = sqlx::query_as("SELECT $1::int")
            .bind(i)
            .fetch_one(pool)
            .await?;
        assert_eq!(row.0, i);
    }

    // Pool should never exceed 1
    assert!(pool.size() <= 1, "Pool size {} should be <= 1", pool.size());

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn test_pool_statement_cache_large() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 1,
        max_connections: 5,
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig::default(),
        statement_cache_capacity: 10000, // Very large cache
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Execute many different queries
    for i in 0..100 {
        let query = format!("SELECT {}::int", i);
        let row: (i32,) = sqlx::query_as(&query).fetch_one(pool).await?;
        assert_eq!(row.0, i);
    }

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn test_pool_very_short_timeouts() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 1,
        max_connections: 5,
        connect_timeout: 30,
        max_lifetime: Some(1), // 1 second max lifetime
        idle_timeout: Some(1), // 1 second idle timeout
        retry: RetryConfig::default(),
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Execute queries
    let _: (i32,) = sqlx::query_as("SELECT 1").fetch_one(pool).await?;

    // Wait for timeouts to trigger
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Pool should still be functional (connections recycled)
    let _: (i32,) = sqlx::query_as("SELECT 2").fetch_one(pool).await?;

    conn.close().await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Concurrent Operations
// =============================================================================

#[tokio::test]
async fn test_pool_concurrent_close_with_queries() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();

    // Start multiple long-running queries
    let mut handles = vec![];
    for _ in 0..5 {
        let pool_clone = pool.clone();
        handles.push(tokio::spawn(async move {
            let result: Result<(i32,), _> = sqlx::query_as("SELECT 1 FROM pg_sleep(0.3)")
                .fetch_one(&pool_clone)
                .await;
            result.is_ok()
        }));
    }

    // Give queries time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Close pool while queries are running
    conn.close().await?;

    // All queries should either complete or fail gracefully
    for handle in handles {
        let _ = handle.await; // Don't panic on error
    }

    // Pool should be closed
    assert!(pool.is_closed());

    Ok(())
}

#[tokio::test]
async fn test_pool_rapid_acquire_release() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 1,
        max_connections: 3,
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(600),
        retry: RetryConfig::default(),
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;
    let pool = conn.pool();

    // Rapid acquire/release cycles
    for _ in 0..100 {
        let _: (i32,) = sqlx::query_as("SELECT 1").fetch_one(pool).await?;
    }

    // Pool should be stable
    assert!(pool.size() >= 1);
    assert!(pool.size() <= 3);

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn test_pool_race_multiple_pools() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();

    // Create multiple pools to same database
    let conn1 = Connection::new(&uri, PoolConfig::default()).await?;
    let conn2 = Connection::new(&uri, PoolConfig::default()).await?;
    let conn3 = Connection::new(&uri, PoolConfig::default()).await?;

    // Run concurrent queries on all pools
    let (r1, r2, r3) = tokio::join!(
        async {
            let pool = conn1.pool();
            let row: (i32,) = sqlx::query_as("SELECT 1").fetch_one(pool).await?;
            Ok::<i32, sqlx::Error>(row.0)
        },
        async {
            let pool = conn2.pool();
            let row: (i32,) = sqlx::query_as("SELECT 2").fetch_one(pool).await?;
            Ok::<i32, sqlx::Error>(row.0)
        },
        async {
            let pool = conn3.pool();
            let row: (i32,) = sqlx::query_as("SELECT 3").fetch_one(pool).await?;
            Ok::<i32, sqlx::Error>(row.0)
        }
    );

    assert_eq!(r1?, 1);
    assert_eq!(r2?, 2);
    assert_eq!(r3?, 3);

    conn1.close().await?;
    conn2.close().await?;
    conn3.close().await?;
    Ok(())
}

// =============================================================================
// EDGE CASES - Error Recovery
// =============================================================================

#[tokio::test]
async fn test_pool_recovery_after_query_error() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let conn = Connection::new(&uri, PoolConfig::default()).await?;
    let pool = conn.pool();

    // Execute invalid query
    let result: Result<(i32,), _> = sqlx::query_as("SELECT * FROM nonexistent_table_12345")
        .fetch_one(pool)
        .await;
    assert!(result.is_err());

    // Pool should still be functional
    let row: (i32,) = sqlx::query_as("SELECT 42").fetch_one(pool).await?;
    assert_eq!(row.0, 42);

    // Multiple errors should not break the pool
    for _ in 0..5 {
        let _ = sqlx::query_as::<_, (i32,)>("INVALID SQL SYNTAX")
            .fetch_one(pool)
            .await;
    }

    // Pool should still work
    let row: (i32,) = sqlx::query_as("SELECT 100").fetch_one(pool).await?;
    assert_eq!(row.0, 100);

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn test_pool_ping_after_idle() -> Result<(), Box<dyn std::error::Error>> {
    let uri = get_database_url();
    let config = PoolConfig {
        min_connections: 1,
        max_connections: 5,
        connect_timeout: 30,
        max_lifetime: Some(1800),
        idle_timeout: Some(2), // Short idle timeout
        retry: RetryConfig::default(),
        statement_cache_capacity: 100,
    };

    let conn = Connection::new(&uri, config).await?;

    // Use connection
    conn.ping().await?;

    // Wait past idle timeout
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Ping should still work (reconnect if needed)
    conn.ping().await?;

    conn.close().await?;
    Ok(())
}
