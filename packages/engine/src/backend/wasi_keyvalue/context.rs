use utils::storage::db::RedbStorage;
use wasmtime::component::HasData;
use wasmtime_wasi::ResourceTable;

use crate::bindings::worker::world::wasi::keyvalue::{atomics, batch, store};
use crate::{utils::error::EngineError, worlds::worker::component::HostComponent};

#[derive(Clone)]
pub struct KeyValueCtx {
    db: RedbStorage,
    // should be a unique identifier for the keyvalue store, e.g. per-service
    // this is *not* the namespace per-bucket, each KeyValueCtx may have multiple buckets
    namespace: String,
    // for pagination
    pub page_size: Option<usize>,
}

impl KeyValueCtx {
    pub fn new(db: RedbStorage, namespace: String) -> Self {
        KeyValueCtx {
            db,
            namespace,
            page_size: None,
        }
    }
    pub fn add_to_linker(
        linker: &mut wasmtime::component::Linker<HostComponent>,
    ) -> Result<(), EngineError> {
        store::add_to_linker::<HostComponent, KeyValueCtx>(linker, |state| {
            KeyValueState::new(
                state.keyvalue_ctx.db.clone(),
                state.keyvalue_ctx.namespace.clone(),
                &mut state.table,
                state.keyvalue_ctx.page_size,
            )
        })
        .map_err(EngineError::AddToLinker)?;

        atomics::add_to_linker::<HostComponent, KeyValueCtx>(linker, |state| {
            KeyValueState::new(
                state.keyvalue_ctx.db.clone(),
                state.keyvalue_ctx.namespace.clone(),
                &mut state.table,
                state.keyvalue_ctx.page_size,
            )
        })
        .map_err(EngineError::AddToLinker)?;

        batch::add_to_linker::<HostComponent, KeyValueCtx>(linker, |state| {
            KeyValueState::new(
                state.keyvalue_ctx.db.clone(),
                state.keyvalue_ctx.namespace.clone(),
                &mut state.table,
                state.keyvalue_ctx.page_size,
            )
        })
        .map_err(EngineError::AddToLinker)?;

        Ok(())
    }
}

impl HasData for KeyValueCtx {
    type Data<'a> = KeyValueState<'a>;
}

pub struct KeyValueState<'a> {
    pub db: RedbStorage,
    pub namespace: String,
    pub resource_table: &'a mut ResourceTable,
    pub page_size: Option<usize>,
}

impl<'a> KeyValueState<'a> {
    pub fn new(
        db: RedbStorage,
        namespace: String,
        resource_table: &'a mut ResourceTable,
        page_size: Option<usize>,
    ) -> Self {
        Self {
            db,
            namespace,
            resource_table,
            page_size,
        }
    }
}
