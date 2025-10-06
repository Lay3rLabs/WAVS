use utils::config::ChainConfigs;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};
use wavs_types::{ComponentDigest, EventId, Service, ServiceId, WorkflowId};

use crate::{
    backend::wasi_keyvalue::context::KeyValueCtx, bindings::aggregator::world::host::LogLevel,
};

pub type AggregatorHostComponentLogger =
    fn(&ServiceId, &WorkflowId, &ComponentDigest, LogLevel, String);

pub struct AggregatorHostComponent {
    pub service: Service,
    pub workflow_id: WorkflowId,
    pub chain_configs: ChainConfigs,
    pub event_id: EventId,
    pub(crate) table: wasmtime::component::ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) http: WasiHttpCtx,
    pub(crate) keyvalue_ctx: KeyValueCtx,
    pub(crate) inner_log: AggregatorHostComponentLogger,
}

impl WasiView for AggregatorHostComponent {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for AggregatorHostComponent {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http
    }

    fn table(&mut self) -> &mut wasmtime::component::ResourceTable {
        &mut self.table
    }
}
