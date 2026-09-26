//! MongoDB connection management with pool configuration and health checking

use bson::{doc, Document as BsonDocument};
use mamba_core::Result;
use mongodb::{
    options::{ClientOptions, ServerApi, ServerApiVersion},
    Client, Collection, Database,
};
use std::time::Duration;

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Minimum number of connections in the pool (default: 0)
    pub min_pool_size: Option<u32>,
    /// Maximum number of connections in the pool (default: 10)
    pub max_pool_size: Option<u32>,
    /// Maximum time a connection can remain idle before being closed (default: none)
    pub max_idle_time: Option<Duration>,
    /// Connection timeout (default: 10s)
    pub connect_timeout: Option<Duration>,
    /// Server selection timeout (default: 30s)
    pub server_selection_timeout: Option<Duration>,
    /// Application name for server logs
    pub app_name: Option<String>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            // Optimization: Keep 5 connections warm to reduce establishment overhead
            min_pool_size: Some(5),
            max_pool_size: Some(20),
            max_idle_time: None,
            connect_timeout: Some(Duration::from_secs(10)),
            server_selection_timeout: Some(Duration::from_secs(30)),
            app_name: Some("cclab".to_string()),
        }
    }
}

/// MongoDB connection manager with pooling support
pub struct Connection {
    client: Client,
    database: Database,
    database_name: String,
}

impl Connection {
    /// Create a new MongoDB connection with default pool settings
    pub async fn new(connection_string: &str) -> Result<Self> {
        Self::with_config(connection_string, PoolConfig::default()).await
    }

    /// Create a new MongoDB connection with custom pool configuration
    pub async fn with_config(connection_string: &str, config: PoolConfig) -> Result<Self> {
        let mut client_options = ClientOptions::parse(connection_string).await?;

        // Apply pool configuration
        if let Some(min) = config.min_pool_size {
            client_options.min_pool_size = Some(min);
        }
        if let Some(max) = config.max_pool_size {
            client_options.max_pool_size = Some(max);
        }
        if let Some(idle) = config.max_idle_time {
            client_options.max_idle_time = Some(idle);
        }
        if let Some(connect) = config.connect_timeout {
            client_options.connect_timeout = Some(connect);
        }
        if let Some(server_sel) = config.server_selection_timeout {
            client_options.server_selection_timeout = Some(server_sel);
        }
        if let Some(app) = config.app_name {
            client_options.app_name = Some(app);
        }

        // Set stable API version for compatibility
        let server_api = ServerApi::builder().version(ServerApiVersion::V1).build();
        client_options.server_api = Some(server_api);

        let client = Client::with_options(client_options)?;

        let database = client.default_database().ok_or_else(|| {
            mamba_core::DataBridgeError::Connection(
                "No default database specified in connection string".to_string(),
            )
        })?;

        let database_name = database.name().to_string();

        Ok(Self {
            client,
            database,
            database_name,
        })
    }

    /// Get a reference to the database
    pub fn database(&self) -> &Database {
        &self.database
    }

    /// Get the database name
    pub fn database_name(&self) -> &str {
        &self.database_name
    }

    /// Get a reference to the client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get a collection by name (returns untyped BsonDocument collection)
    pub fn get_collection(&self, name: &str) -> Collection<BsonDocument> {
        self.database.collection(name)
    }

    /// Get a typed collection
    pub fn get_typed_collection<T: Send + Sync>(&self, name: &str) -> Collection<T> {
        self.database.collection(name)
    }

    /// Switch to a different database
    pub fn use_database(&self, name: &str) -> Database {
        self.client.database(name)
    }

    /// Check if the connection is healthy by pinging the server
    pub async fn ping(&self) -> Result<bool> {
        match self.database.run_command(doc! { "ping": 1 }).await {
            Ok(_) => Ok(true),
            Err(e) => Err(mamba_core::DataBridgeError::Connection(format!(
                "Ping failed: {}",
                e
            ))),
        }
    }

    /// Get server status information
    pub async fn server_status(&self) -> Result<BsonDocument> {
        let result = self
            .database
            .run_command(doc! { "serverStatus": 1 })
            .await?;
        Ok(result)
    }

    /// List all collection names in the current database
    pub async fn list_collection_names(&self) -> Result<Vec<String>> {
        let names = self.database.list_collection_names().await?;
        Ok(names)
    }

    /// List all database names on the server
    pub async fn list_database_names(&self) -> Result<Vec<String>> {
        let names = self.client.list_database_names().await?;
        Ok(names)
    }

    /// Drop the current database (use with caution!)
    pub async fn drop_database(&self) -> Result<()> {
        self.database.drop().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_pool_config() {
        let config = PoolConfig::default();
        assert_eq!(config.min_pool_size, Some(5)); // Optimized: warm connections
        assert_eq!(config.max_pool_size, Some(20)); // Optimized: larger pool
        assert_eq!(config.app_name, Some("cclab".to_string()));
    }

    #[test]
    fn test_custom_pool_config() {
        let config = PoolConfig {
            min_pool_size: Some(5),
            max_pool_size: Some(50),
            max_idle_time: Some(Duration::from_secs(300)),
            connect_timeout: Some(Duration::from_secs(5)),
            server_selection_timeout: Some(Duration::from_secs(10)),
            app_name: Some("my-app".to_string()),
        };
        assert_eq!(config.min_pool_size, Some(5));
        assert_eq!(config.max_pool_size, Some(50));
    }
}
