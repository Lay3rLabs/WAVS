use utils::config::ChainConfigs;
use wasmtime_wasi::{IoView, WasiCtx, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};
use wavs_types::{Digest, ServiceID, Workflow, WorkflowID};

use crate::bindings::world::host::LogLevel;

// TODO: revisit this an understand it.
// Copied blindly from old code
pub struct HostComponent {
    pub workflow: Workflow,
    pub workflow_id: WorkflowID,
    pub service_id: ServiceID,
    pub chain_configs: ChainConfigs,
    pub(crate) table: wasmtime::component::ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) http: WasiHttpCtx,
    pub(crate) inner_log: HostComponentLogger,
}

pub type HostComponentLogger = fn(&ServiceID, &WorkflowID, &Digest, LogLevel, String);

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
