//! Settings sources — env vars, .env files, secrets directories
//!
//! Each source collects raw key-value string pairs. The SettingsBuilder
//! then merges them in priority order and performs type coercion.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::settings::SettingsConfig;
use crate::settings::SettingsError;

// ============================================================================
// Source Trait
// ============================================================================

/// A source of raw key-value settings.
pub trait SettingsSource {
    /// Load raw string key-value pairs from this source.
    ///
    /// Keys should be returned in their original casing; the builder
    /// handles case normalization.
    fn load(&self, config: &SettingsConfig) -> Result<HashMap<String, String>, SettingsError>;
}

// ============================================================================
// Environment Variable Source
// ============================================================================

/// Loads settings from OS environment variables.
///
/// All current environment variables are returned. The builder filters
/// them by prefix and field name.
pub struct EnvSource;

impl EnvSource {
    pub fn new() -> Self {
        Self
    }
}

impl SettingsSource for EnvSource {
    fn load(&self, _config: &SettingsConfig) -> Result<HashMap<String, String>, SettingsError> {
        let vars: HashMap<String, String> = std::env::vars().collect();
        Ok(vars)
    }
}

// ============================================================================
// Dotenv File Source
// ============================================================================

/// Loads settings from a .env file using the `dotenvy` crate.
///
/// Supports standard .env format:
/// ```text
/// KEY=value
/// # comments
/// QUOTED="value with spaces"
/// SINGLE_QUOTED='value'
/// EXPORT=exported  # inline comments
/// ```
pub struct DotenvSource {
    path: PathBuf,
}

impl DotenvSource {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl SettingsSource for DotenvSource {
    fn load(&self, _config: &SettingsConfig) -> Result<HashMap<String, String>, SettingsError> {
        if !self.path.exists() {
            return Ok(HashMap::new());
        }

        let mut result = HashMap::new();
        let iter = dotenvy::from_path_iter(&self.path)
            .map_err(|e| SettingsError::DotenvError(self.path.clone(), e.to_string()))?;

        for item in iter {
            match item {
                Ok((key, value)) => {
                    result.insert(key, value);
                }
                Err(e) => {
                    return Err(SettingsError::DotenvError(self.path.clone(), e.to_string()));
                }
            }
        }

        Ok(result)
    }
}

// ============================================================================
// Secrets File Source
// ============================================================================

/// Loads settings from a secrets directory (e.g., /run/secrets/).
///
/// Each file in the directory represents a setting:
/// - Filename = key (e.g., `db_password`)
/// - File content = value (trimmed)
///
/// This is the standard pattern for Docker/Kubernetes secrets.
pub struct SecretsSource {
    dir: PathBuf,
}

impl SecretsSource {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl SettingsSource for SecretsSource {
    fn load(&self, _config: &SettingsConfig) -> Result<HashMap<String, String>, SettingsError> {
        if !self.dir.exists() {
            return Ok(HashMap::new());
        }

        let mut result = HashMap::new();

        let entries = fs::read_dir(&self.dir)
            .map_err(|e| SettingsError::SecretsError(self.dir.clone(), e.to_string()))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| SettingsError::SecretsError(self.dir.clone(), e.to_string()))?;

            let path = entry.path();

            // Skip directories and hidden files
            if path.is_dir() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }

            // Read file content
            let key = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if key.is_empty() {
                continue;
            }

            let value = fs::read_to_string(&path)
                .map_err(|e| SettingsError::SecretsError(path.clone(), e.to_string()))?;

            result.insert(key, value.trim().to_string());
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_source_loads() {
        // Set a test env var
        std::env::set_var("CCLAB_TEST_SETTING_XYZ", "test_value");
        let source = EnvSource::new();
        let config = SettingsConfig::default();
        let values = source.load(&config).unwrap();
        assert_eq!(
            values.get("CCLAB_TEST_SETTING_XYZ"),
            Some(&"test_value".to_string())
        );
        std::env::remove_var("CCLAB_TEST_SETTING_XYZ");
    }

    #[test]
    fn test_dotenv_source_missing_file() {
        let source = DotenvSource::new(PathBuf::from("/nonexistent/.env"));
        let config = SettingsConfig::default();
        let values = source.load(&config).unwrap();
        assert!(values.is_empty());
    }

    #[test]
    fn test_secrets_source_missing_dir() {
        let source = SecretsSource::new(PathBuf::from("/nonexistent/secrets"));
        let config = SettingsConfig::default();
        let values = source.load(&config).unwrap();
        assert!(values.is_empty());
    }

    #[test]
    fn test_secrets_source_reads_files() {
        let dir = tempfile::tempdir().unwrap();
        let secret_path = dir.path().join("db_password");
        fs::write(&secret_path, "  super_secret  \n").unwrap();

        let source = SecretsSource::new(dir.path().to_path_buf());
        let config = SettingsConfig::default();
        let values = source.load(&config).unwrap();
        assert_eq!(values.get("db_password"), Some(&"super_secret".to_string()));
    }

    #[test]
    fn test_dotenv_source_parses_file() {
        let dir = tempfile::tempdir().unwrap();
        let env_path = dir.path().join(".env");
        fs::write(&env_path, "DB_HOST=localhost\nDB_PORT=5432\n# comment\n").unwrap();

        let source = DotenvSource::new(env_path);
        let config = SettingsConfig::default();
        let values = source.load(&config).unwrap();
        assert_eq!(values.get("DB_HOST"), Some(&"localhost".to_string()));
        assert_eq!(values.get("DB_PORT"), Some(&"5432".to_string()));
    }
}
