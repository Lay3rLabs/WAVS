use wasmtime::component::types::{ComponentItem, ComponentFunc};
use wasmtime::Engine;

/// Gather all exported functions from a component type, including nested instance exports.
///
/// Returns a list of (qualified_name, ComponentFunc) pairs. For functions inside
/// a ComponentInstance export, the name is formatted as "instance_name/func_name".
pub fn gather_exports(
    component_type: &wasmtime::component::types::Component,
    engine: &Engine,
) -> Vec<(String, ComponentFunc)> {
    todo!("implement gather_exports")
}
