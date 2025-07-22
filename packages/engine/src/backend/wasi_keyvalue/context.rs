use utils::storage::db::RedbStorage;
use wasmtime::component::HasData;
use wasmtime_wasi::ResourceTable;

use crate::bindings::worker::world::wasi::keyvalue::{atomics, batch, store};
use crate::{
    utils::error::EngineError, worlds::aggregator::AggregatorHostComponent,
    worlds::worker::component::HostComponent,
};

pub trait KeyValueCtxProvider {
    fn keyvalue_ctx(&self) -> &KeyValueCtx;
    fn table(&mut self) -> &mut ResourceTable;
}

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
    pub fn add_to_linker<T>(linker: &mut wasmtime::component::Linker<T>) -> Result<(), EngineError>
    where
        T: KeyValueCtxProvider + Send,
    {
        store::add_to_linker::<T, KeyValueCtx>(linker, |state| {
            let ctx = state.keyvalue_ctx();
            let db = ctx.db.clone();
            let namespace = ctx.namespace.clone();
            let page_size = ctx.page_size;
            let table = state.table();
            KeyValueState::new(db, namespace, table, page_size)
        })
        .map_err(EngineError::AddToLinker)?;

        atomics::add_to_linker::<T, KeyValueCtx>(linker, |state| {
            let ctx = state.keyvalue_ctx();
            let db = ctx.db.clone();
            let namespace = ctx.namespace.clone();
            let page_size = ctx.page_size;
            let table = state.table();
            KeyValueState::new(db, namespace, table, page_size)
        })
        .map_err(EngineError::AddToLinker)?;

        batch::add_to_linker::<T, KeyValueCtx>(linker, |state| {
            let ctx = state.keyvalue_ctx();
            let db = ctx.db.clone();
            let namespace = ctx.namespace.clone();
            let page_size = ctx.page_size;
            let table = state.table();
            KeyValueState::new(db, namespace, table, page_size)
        })
        .map_err(EngineError::AddToLinker)?;

        Ok(())
    }
}

impl HasData for KeyValueCtx {
    type Data<'a> = KeyValueState<'a>;
}

impl KeyValueCtxProvider for HostComponent {
    fn keyvalue_ctx(&self) -> &KeyValueCtx {
        &self.keyvalue_ctx
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

impl KeyValueCtxProvider for AggregatorHostComponent {
    fn keyvalue_ctx(&self) -> &KeyValueCtx {
        &self.keyvalue_ctx
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
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
