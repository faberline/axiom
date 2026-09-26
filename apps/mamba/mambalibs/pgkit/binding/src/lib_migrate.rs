//! `mambalibs.pg.migrate` Mamba sub-module.
//!
//! Exposes the schema-versioning surface of `pg::migrate`
//! (Alembic equivalent) — `MigrationRunner` with 7 verbs plus the
//! `Migration` value-object constructor.

use linkme::distributed_slice;
use mamba_registry::{rt_sym, MambaModule, ModuleRegistrar, MAMBA_MODULES};

/// Migrate surface mounted under `mambalibs.pg.migrate`.
pub struct PgMigrateMambaModule;

impl MambaModule for PgMigrateMambaModule {
    fn name(&self) -> &'static str {
        "mambalibs.pg.migrate"
    }

    fn doc(&self) -> &'static str {
        "Mamba interface for pg::migrate — schema versioning (MigrationRunner)"
    }

    fn register(&self, r: &mut ModuleRegistrar) {
        use crate::methods::{
            mb_pg_migration_new, mb_pg_migration_runner_applied_migrations,
            mb_pg_migration_runner_apply, mb_pg_migration_runner_down, mb_pg_migration_runner_init,
            mb_pg_migration_runner_new, mb_pg_migration_runner_revert,
            mb_pg_migration_runner_status, mb_pg_migration_runner_up,
        };

        r.add_symbols([
            rt_sym!(
                "MigrationRunner",
                mb_pg_migration_runner_new,
                "MigrationRunner(conn, table=None) -> Runner"
            ),
            rt_sym!(
                "runner_init",
                mb_pg_migration_runner_init,
                "runner_init(runner) -> None"
            ),
            rt_sym!(
                "runner_apply",
                mb_pg_migration_runner_apply,
                "runner_apply(runner, migration) -> None"
            ),
            rt_sym!(
                "runner_revert",
                mb_pg_migration_runner_revert,
                "runner_revert(runner, migration) -> None"
            ),
            rt_sym!(
                "runner_up",
                mb_pg_migration_runner_up,
                "runner_up(runner, dir: str) -> list[str]"
            ),
            rt_sym!(
                "runner_down",
                mb_pg_migration_runner_down,
                "runner_down(runner, dir: str) -> str?"
            ),
            rt_sym!(
                "runner_applied_migrations",
                mb_pg_migration_runner_applied_migrations,
                "runner_applied_migrations(runner) -> list[str]"
            ),
            rt_sym!(
                "runner_status",
                mb_pg_migration_runner_status,
                "runner_status(runner, migrations: list) -> dict"
            ),
            rt_sym!(
                "Migration",
                mb_pg_migration_new,
                "Migration(version, name, up, down) -> Migration"
            ),
        ]);
    }
}

#[distributed_slice(MAMBA_MODULES)]
static PG_MIGRATE_MAMBA_MODULE: &dyn MambaModule = &PgMigrateMambaModule;
