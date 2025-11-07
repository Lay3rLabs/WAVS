use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};
use wasmtime_wasi_tls::WasiTlsCtx;
use wavs_types::{ChainConfigs, ComponentDigest, EventId, Service, ServiceId, WorkflowId};

use crate::backend::wasi_keyvalue::context::KeyValueCtx;
use crate::bindings::operator::world::host::LogLevel;

// This is defined separately because LogLevel comes from bindings
pub type OperatorHostComponentLogger =
    fn(&ServiceId, &WorkflowId, &ComponentDigest, LogLevel, String);

// TODO: revisit this an understand it.
// Copied blindly from old code
pub struct OperatorHostComponent {
    pub service: Service,
    pub workflow_id: WorkflowId,
    pub chain_configs: ChainConfigs,
    pub event_id: EventId,
    pub(crate) table: wasmtime::component::ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) http_ctx: WasiHttpCtx,
    pub(crate) tls_ctx: WasiTlsCtx,
    pub(crate) keyvalue_ctx: KeyValueCtx,
    pub(crate) inner_log: OperatorHostComponentLogger,
}

impl WasiView for OperatorHostComponent {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for OperatorHostComponent {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http_ctx
    }

    fn table(&mut self) -> &mut wasmtime::component::ResourceTable {
        &mut self.table
    }
}
