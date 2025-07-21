use utils::config::ChainConfigs;
use wasmtime_wasi::p2::{IoView, WasiCtx, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};
use wavs_types::{ComponentDigest, Service, ServiceID, WorkflowID};

use crate::backend::wasi_keyvalue::context::KeyValueCtx;
use crate::bindings::worker::world::host::LogLevel;

// TODO: revisit this an understand it.
// Copied blindly from old code
pub struct HostComponent {
    pub service: Service,
    pub workflow_id: WorkflowID,
    pub chain_configs: ChainConfigs,
    pub(crate) table: wasmtime::component::ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) http: WasiHttpCtx,
    pub(crate) keyvalue_ctx: KeyValueCtx,
    pub(crate) inner_log: HostComponentLogger,
}

pub type HostComponentLogger = fn(&ServiceID, &WorkflowID, &ComponentDigest, LogLevel, String);

impl WasiView for HostComponent {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}

impl IoView for HostComponent {
    fn table(&mut self) -> &mut wasmtime_wasi::ResourceTable {
        &mut self.table
    }
}

impl WasiHttpView for HostComponent {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http
    }
}
