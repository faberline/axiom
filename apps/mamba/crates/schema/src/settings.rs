//! Settings configuration and builder
//!
//! Core types for configuring how settings are loaded from environment
//! variables, .env files, and secrets directories.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::settings_source::{DotenvSource, EnvSource, SecretsSource, SettingsSource};

// ============================================================================
// Settings Error
// ============================================================================

/// Error type for settings loading and coercion.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("missing required field: {0}")]
    MissingField(String),
    #[error("coercion error for field '{0}': {1}")]
    CoercionError(String, String),
    #[error("dotenv error ({0:?}): {1}")]
    DotenvError(PathBuf, String),
    #[error("secrets error ({0:?}): {1}")]
    SecretsError(PathBuf, String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

/// Coerce a raw string value into a SettingsValue based on the target type.
pub fn coerce_str(s: &str, field_type: &FieldType) -> Result<SettingsValue, String> {
    match field_type {
        FieldType::String => Ok(SettingsValue::String(s.to_string())),
        FieldType::Int => s
            .parse::<i64>()
            .map(SettingsValue::Int)
            .map_err(|e| format!("cannot parse '{}' as int: {}", s, e)),
        FieldType::Float => s
            .parse::<f64>()
            .map(SettingsValue::Float)
            .map_err(|e| format!("cannot parse '{}' as float: {}", s, e)),
        FieldType::Bool => match s.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(SettingsValue::Bool(true)),
            "false" | "0" | "no" | "off" => Ok(SettingsValue::Bool(false)),
            _ => Err(format!("cannot parse '{}' as bool", s)),
        },
        FieldType::Json | FieldType::Dict | FieldType::List => {
            // Try parsing as JSON
            Ok(SettingsValue::String(s.to_string()))
        }
        FieldType::Nested => Ok(SettingsValue::String(s.to_string())),
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for settings loading behavior.
///
/// Controls how environment variables are matched, which files to load,
/// and how nested fields are delimited.
#[derive(Debug, Clone)]
pub struct SettingsConfig {
    /// Prefix for environment variable names (e.g., "MYAPP_").
    pub env_prefix: String,

    /// Path to .env file (None to disable).
    pub env_file: Option<PathBuf>,

    /// Encoding for .env file.
    pub env_file_encoding: String,

    /// Whether env var names are case-sensitive.
    pub case_sensitive: bool,

    /// Delimiter for nested field access (e.g., "__" for DB__HOST -> db.host).
    pub env_nested_delimiter: Option<String>,

    /// Directory containing secret files (e.g., /run/secrets/).
    pub secrets_dir: Option<PathBuf>,
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            env_prefix: String::new(),
            env_file: Some(PathBuf::from(".env")),
            env_file_encoding: "utf-8".to_string(),
            case_sensitive: false,
            env_nested_delimiter: None,
            secrets_dir: None,
        }
    }
}

// ============================================================================
// Field Definition
// ============================================================================

/// Describes a single field in a settings model.
#[derive(Debug, Clone)]
pub struct FieldDef {
    /// Field name as declared in the model.
    pub name: String,

    /// Expected type for coercion.
    pub field_type: FieldType,

    /// Default value (None means required).
    pub default: Option<SettingsValue>,

    /// Whether this is a nested settings model.
    pub is_nested: bool,

    /// Nested field definitions (only if is_nested).
    pub nested_fields: Vec<FieldDef>,
}

/// Supported field types for environment variable coercion.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    String,
    Int,
    Float,
    Bool,
    List,
    Dict,
    Json,
    Nested,
}

/// A resolved settings value after coercion.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    List(Vec<SettingsValue>),
    Dict(HashMap<String, SettingsValue>),
    Null,
}

impl std::fmt::Display for SettingsValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(s) => write!(f, "{}", s),
            Self::Int(i) => write!(f, "{}", i),
            Self::Float(fl) => write!(f, "{}", fl),
            Self::Bool(b) => write!(f, "{}", b),
            Self::List(_) => write!(f, "[...]"),
            Self::Dict(_) => write!(f, "{{...}}"),
            Self::Null => write!(f, "null"),
        }
    }
}

// ============================================================================
// Settings Builder
// ============================================================================

/// Builds a resolved settings map from multiple sources.
///
/// Sources are applied in priority order (lowest to highest):
/// 1. Default values from field definitions
/// 2. Secrets files
/// 3. .env file values
/// 4. Environment variables
/// 5. Explicit overrides (keyword arguments)
pub struct SettingsBuilder {
    config: SettingsConfig,
    fields: Vec<FieldDef>,
    overrides: HashMap<String, String>,
}

impl SettingsBuilder {
    pub fn new(config: SettingsConfig) -> Self {
        Self {
            config,
            fields: Vec::new(),
            overrides: HashMap::new(),
        }
    }

    /// Add a field definition.
    pub fn add_field(&mut self, field: FieldDef) {
        self.fields.push(field);
    }

    /// Add explicit overrides (highest priority).
    pub fn add_override(&mut self, key: String, value: String) {
        self.overrides.insert(key, value);
    }

    /// Build the resolved settings map.
    ///
    /// Collects values from all sources in priority order, performs
    /// type coercion, and handles nested field resolution.
    pub fn build(&self) -> Result<HashMap<String, SettingsValue>, SettingsError> {
        // Collect raw string values from sources (low to high priority)
        let mut raw_values: HashMap<String, String> = HashMap::new();

        // 1. Secrets files (lowest priority of external sources)
        if let Some(ref secrets_dir) = self.config.secrets_dir {
            let secrets = SecretsSource::new(secrets_dir.clone());
            let secret_values = secrets.load(&self.config)?;
            raw_values.extend(secret_values);
        }

        // 2. .env file
        if let Some(ref env_file) = self.config.env_file {
            let dotenv = DotenvSource::new(env_file.clone());
            let dotenv_values = dotenv.load(&self.config)?;
            raw_values.extend(dotenv_values);
        }

        // 3. Environment variables (higher priority)
        let env_source = EnvSource::new();
        let env_values = env_source.load(&self.config)?;
        raw_values.extend(env_values);

        // 4. Explicit overrides (highest priority)
        raw_values.extend(self.overrides.clone());

        // Resolve fields
        self.resolve_fields(&self.fields, &raw_values, &self.config.env_prefix)
    }

    /// Resolve field values from raw string map.
    fn resolve_fields(
        &self,
        fields: &[FieldDef],
        raw_values: &HashMap<String, String>,
        prefix: &str,
    ) -> Result<HashMap<String, SettingsValue>, SettingsError> {
        let mut result = HashMap::new();

        for field in fields {
            let value = if field.is_nested {
                self.resolve_nested_field(field, raw_values, prefix)?
            } else {
                self.resolve_simple_field(field, raw_values, prefix)?
            };

            if let Some(v) = value {
                result.insert(field.name.clone(), v);
            } else if let Some(ref default) = field.default {
                result.insert(field.name.clone(), default.clone());
            } else {
                return Err(SettingsError::MissingField(field.name.clone()));
            }
        }

        Ok(result)
    }

    /// Resolve a simple (non-nested) field.
    fn resolve_simple_field(
        &self,
        field: &FieldDef,
        raw_values: &HashMap<String, String>,
        prefix: &str,
    ) -> Result<Option<SettingsValue>, SettingsError> {
        let env_name = format!("{}{}", prefix, field.name);
        let lookup_key = if self.config.case_sensitive {
            env_name.clone()
        } else {
            env_name.to_uppercase()
        };

        // Find matching key (case-insensitive if configured)
        let raw_value = if self.config.case_sensitive {
            raw_values.get(&lookup_key)
        } else {
            raw_values
                .iter()
                .find(|(k, _)| k.to_uppercase() == lookup_key)
                .map(|(_, v)| v)
        };

        match raw_value {
            Some(s) => {
                let coerced = coerce_str(s, &field.field_type)
                    .map_err(|e| SettingsError::CoercionError(field.name.clone(), e))?;
                Ok(Some(coerced))
            }
            None => Ok(None),
        }
    }

    /// Resolve a nested settings field using the delimiter.
    fn resolve_nested_field(
        &self,
        field: &FieldDef,
        raw_values: &HashMap<String, String>,
        prefix: &str,
    ) -> Result<Option<SettingsValue>, SettingsError> {
        let delimiter = self.config.env_nested_delimiter.as_deref().unwrap_or("__");
        let nested_prefix = format!("{}{}{}", prefix, field.name, delimiter);

        // Collect sub-values for nested fields
        let nested_result =
            self.resolve_fields(&field.nested_fields, raw_values, &nested_prefix)?;

        if nested_result.is_empty() {
            // Try loading as a single JSON string
            let env_name = format!("{}{}", prefix, field.name);
            let lookup_key = if self.config.case_sensitive {
                env_name
            } else {
                env_name.to_uppercase()
            };

            let raw_value = if self.config.case_sensitive {
                raw_values.get(&lookup_key)
            } else {
                raw_values
                    .iter()
                    .find(|(k, _)| k.to_uppercase() == lookup_key)
                    .map(|(_, v)| v)
            };

            match raw_value {
                Some(s) => {
                    let coerced = coerce_str(s, &FieldType::Json)
                        .map_err(|e| SettingsError::CoercionError(field.name.clone(), e))?;
                    Ok(Some(coerced))
                }
                None => Ok(None),
            }
        } else {
            Ok(Some(SettingsValue::Dict(nested_result)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_config_default() {
        let config = SettingsConfig::default();
        assert_eq!(config.env_prefix, "");
        assert_eq!(config.env_file, Some(PathBuf::from(".env")));
        assert!(!config.case_sensitive);
        assert!(config.env_nested_delimiter.is_none());
        assert!(config.secrets_dir.is_none());
    }

    #[test]
    fn test_settings_builder_simple() {
        let config = SettingsConfig {
            env_file: None,
            secrets_dir: None,
            ..Default::default()
        };
        let mut builder = SettingsBuilder::new(config);
        builder.add_field(FieldDef {
            name: "host".to_string(),
            field_type: FieldType::String,
            default: Some(SettingsValue::String("localhost".to_string())),
            is_nested: false,
            nested_fields: vec![],
        });
        builder.add_field(FieldDef {
            name: "port".to_string(),
            field_type: FieldType::Int,
            default: Some(SettingsValue::Int(8080)),
            is_nested: false,
            nested_fields: vec![],
        });

        let result = builder.build().unwrap();
        assert_eq!(
            result.get("host"),
            Some(&SettingsValue::String("localhost".to_string()))
        );
        assert_eq!(result.get("port"), Some(&SettingsValue::Int(8080)));
    }

    #[test]
    fn test_settings_builder_with_overrides() {
        let config = SettingsConfig {
            env_file: None,
            secrets_dir: None,
            ..Default::default()
        };
        let mut builder = SettingsBuilder::new(config);
        builder.add_field(FieldDef {
            name: "host".to_string(),
            field_type: FieldType::String,
            default: Some(SettingsValue::String("localhost".to_string())),
            is_nested: false,
            nested_fields: vec![],
        });
        builder.add_override("HOST".to_string(), "example.com".to_string());

        let result = builder.build().unwrap();
        assert_eq!(
            result.get("host"),
            Some(&SettingsValue::String("example.com".to_string()))
        );
    }

    #[test]
    fn test_missing_required_field() {
        let config = SettingsConfig {
            env_file: None,
            secrets_dir: None,
            ..Default::default()
        };
        let mut builder = SettingsBuilder::new(config);
        builder.add_field(FieldDef {
            name: "required_field".to_string(),
            field_type: FieldType::String,
            default: None,
            is_nested: false,
            nested_fields: vec![],
        });

        let result = builder.build();
        assert!(result.is_err());
    }
}
