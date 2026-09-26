//! CLI subcommands for `cclab pg migrate`.
//!
//! Registers the `pg` subcommand via the `cli_std::registry` distributed slice
//! so it is automatically available when `cclab` is built with this crate linked.
//!
//! # Exposed subcommand
//!
//! ```text
//! cclab pg migrate generate [--message <msg>] [--database-url <url>]
//! cclab pg migrate up       [--database-url <url>]
//! cclab pg migrate down     [--database-url <url>]
//! cclab pg migrate status   [--database-url <url>]
//! ```

pub mod migrate;

pub use migrate::{run_migrate, MigrateAction};

use cli_std::registry::{CliModule, CLI_MODULES};
use clap::{ArgMatches, Command, FromArgMatches, Subcommand};
use linkme::distributed_slice;

/// Top-level `pg` subcommand that hosts `migrate` and legacy migration commands.
pub struct PgMigrateCli;

/// clap `Commands` enum for `cclab pg migrate …`.
#[derive(Subcommand, Debug)]
pub enum PgCommands {
    /// Mamba-native SQL migration runner
    Migrate {
        #[command(subcommand)]
        action: MigrateAction,
    },
}

impl CliModule for PgMigrateCli {
    fn name(&self) -> &'static str {
        "pg-migrate"
    }

    fn command(&self) -> Command {
        let cmd = Command::new("pg-migrate")
            .about("cclab pg migrate — Mamba-native SQL migration runner");
        PgCommands::augment_subcommands(cmd)
    }

    fn execute(&self, matches: &ArgMatches) -> anyhow::Result<()> {
        let cmd = PgCommands::from_arg_matches(matches)?;
        let rt = tokio::runtime::Runtime::new()?;
        match cmd {
            PgCommands::Migrate { action } => {
                rt.block_on(run_migrate(action))?;
            }
        }
        Ok(())
    }
}

#[distributed_slice(CLI_MODULES)]
static PG_MIGRATE_CLI: &dyn CliModule = &PgMigrateCli;
