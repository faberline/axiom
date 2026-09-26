use std::env;

/// Server configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Server host address
    pub host: String,
    /// Server port
    pub port: u16,
    /// Database storage path (for cclab-kv)
    pub database_path: String,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()?;
        // Changed from DATABASE_URL to DATABASE_PATH for file-based storage
        let database_path =
            env::var("DATABASE_PATH").unwrap_or_else(|_| "./data/rusheet.db".to_string());

        Ok(Self {
            host,
            port,
            database_path,
        })
    }
}
