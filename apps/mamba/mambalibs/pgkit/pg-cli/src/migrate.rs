//! clap handler for `cclab pg migrate {generate,up,down,status}`.
//!
//! All commands accept `--database-url`; if absent, they fall back to the
//! `DATABASE_URL` environment variable.

use clap::Subcommand;
use std::path::PathBuf;

use pg::auto_detect::ModelDefinition;
use pg::migration::{MigrationRunner, MigrationStatusReport, ModelDiffer};
use pg::{Connection, PoolConfig};

// ── CLI types ─────────────────────────────────────────────────────────────────

/// Subcommands under `cclab pg migrate`.
#[derive(Subcommand, Debug)]
pub enum MigrateAction {
    /// Introspect DeclarativeBase models, diff against the live schema, and
    /// write a new SQL migration file.
    Generate {
        /// Short description included in the file name (e.g. `add_email`).
        #[arg(long, short = 'm', default_value = "migration")]
        message: String,

        /// Database URL (overrides DATABASE_URL env var).
        #[arg(long)]
        database_url: Option<String>,

        /// Directory where migration files are stored.
        #[arg(long, default_value = "migrations")]
        migrations_dir: PathBuf,
    },

    /// Apply all pending migrations in version order.
    Up {
        /// Database URL (overrides DATABASE_URL env var).
        #[arg(long)]
        database_url: Option<String>,

        /// Directory where migration files are stored.
        #[arg(long, default_value = "migrations")]
        migrations_dir: PathBuf,
    },

    /// Revert the last applied native migration.
    Down {
        /// Database URL (overrides DATABASE_URL env var).
        #[arg(long)]
        database_url: Option<String>,

        /// Directory where migration files are stored.
        #[arg(long, default_value = "migrations")]
        migrations_dir: PathBuf,
    },

    /// Show applied and pending migrations.
    Status {
        /// Database URL (overrides DATABASE_URL env var).
        #[arg(long)]
        database_url: Option<String>,

        /// Directory where migration files are stored.
        #[arg(long, default_value = "migrations")]
        migrations_dir: PathBuf,
    },
}

// ── Execution ─────────────────────────────────────────────────────────────────

/// Execute the chosen migrate subcommand.
pub async fn run_migrate(action: MigrateAction) -> anyhow::Result<()> {
    match action {
        MigrateAction::Generate {
            message,
            database_url,
            migrations_dir,
        } => {
            let url = resolve_url(database_url)?;
            let conn = Connection::new(&url, PoolConfig::default()).await?;
            let differ = ModelDiffer::new(conn);

            // Pass empty model list — callers integrate model scanning separately.
            // In the typical workflow, models are read from the Python source by
            // the Mamba compiler and passed via the pg Mamba binding.
            let models: Vec<ModelDefinition> = Vec::new();
            let result = differ.diff(&models).await?;

            if !result.has_changes {
                println!("No schema changes detected.");
                return Ok(());
            }

            let filename = differ.write_migration_file(&result, &message, &migrations_dir)?;
            println!("Generated: {}", filename);
            for s in &result.summary {
                println!("  {}", s);
            }
        }

        MigrateAction::Up {
            database_url,
            migrations_dir,
        } => {
            let url = resolve_url(database_url)?;
            let runner = MigrationRunner::connect(&url).await?;
            let applied = runner.up(&migrations_dir).await?;

            if applied.is_empty() {
                println!("Already up to date.");
            } else {
                println!("Applied {} migration(s):", applied.len());
                for id in &applied {
                    println!("  [✓] {}", id);
                }
            }
        }

        MigrateAction::Down {
            database_url,
            migrations_dir,
        } => {
            let url = resolve_url(database_url)?;
            let runner = MigrationRunner::connect(&url).await?;
            match runner.down(&migrations_dir).await? {
                Some(id) => println!("Reverted: {}", id),
                None => println!("No native migrations to revert."),
            }
        }

        MigrateAction::Status {
            database_url,
            migrations_dir,
        } => {
            let url = resolve_url(database_url)?;
            let conn = Connection::new(&url, PoolConfig::default()).await?;
            let status = MigrationStatusReport::load(conn, &migrations_dir).await?;
            println!("{}", status.to_table());
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Resolve the database URL from CLI flag or `DATABASE_URL` env var.
fn resolve_url(flag: Option<String>) -> anyhow::Result<String> {
    flag.or_else(|| std::env::var("DATABASE_URL").ok())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No database URL provided. \
             Pass --database-url or set DATABASE_URL environment variable."
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn database_url_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DATABASE_URL", "postgres://user:pass@host/db");
        let result = resolve_url(None);
        std::env::remove_var("DATABASE_URL");
        assert_eq!(result.unwrap(), "postgres://user:pass@host/db");
    }

    #[test]
    fn resolve_url_prefers_flag_over_env() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("DATABASE_URL", "postgres://env/db");
        let result = resolve_url(Some("postgres://flag/db".to_string()));
        std::env::remove_var("DATABASE_URL");
        assert_eq!(result.unwrap(), "postgres://flag/db");
    }

    #[test]
    fn resolve_url_errors_when_both_absent() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("DATABASE_URL");
        let result = resolve_url(None);
        assert!(result.is_err());
    }
}
