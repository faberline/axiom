//! Mamba interface for `httpkit`.
//!
//! Core source and logic live in the sibling `httpkit` crate. This crate owns
//! the Mamba import namespace `mambalibs.http` and the native binding surface.

use mamba_registry::{MambaModule, ModuleRegistrar, MAMBA_MODULES};
use linkme::distributed_slice;

pub mod app;
pub mod client;
pub mod health;
pub mod http_exception;
pub mod request_response;
pub mod server;

fn register_http_surface(r: &mut ModuleRegistrar) {
    health::register(r);
    http_exception::register(r);
    request_response::register(r);
    app::register(r);
    client::register(r);
    server::register(r);
}

pub struct MambalibsHttpModule;

impl MambaModule for MambalibsHttpModule {
    fn name(&self) -> &'static str {
        "mambalibs.http"
    }

    fn doc(&self) -> &'static str {
        "Mamba-native HTTP interface"
    }

    fn register(&self, r: &mut ModuleRegistrar) {
        register_http_surface(r);
    }
}

#[distributed_slice(MAMBA_MODULES)]
static MAMBALIBS_HTTP_MODULE: &dyn MambaModule = &MambalibsHttpModule;
