use wasmtime::component::types::{ComponentFunc, ComponentItem};
use wasmtime::Engine;

/// Gather all exported functions from a component type, including nested instance exports.
///
/// Returns a list of (qualified_name, ComponentFunc) pairs. For functions inside
/// a ComponentInstance export, the name is formatted as "instance_name/func_name".
/// Only exported functions are collected (D-05: imports are excluded).
pub fn gather_exports(
    component_type: &wasmtime::component::types::Component,
    engine: &Engine,
) -> Vec<(String, ComponentFunc)> {
    let mut funcs = Vec::new();

    for (name, item) in component_type.exports(engine) {
        match item {
            ComponentItem::ComponentFunc(func) => {
                funcs.push((name.to_string(), func));
            }
            ComponentItem::ComponentInstance(instance) => {
                // Recurse into instance exports to find nested functions
                for (sub_name, sub_item) in instance.exports(engine) {
                    if let ComponentItem::ComponentFunc(func) = sub_item {
                        funcs.push((format!("{}/{}", name, sub_name), func));
                    }
                }
            }
            // Skip all other ComponentItem variants (Module, Component, Type, Resource, CoreFunc)
            _ => {}
        }
    }

    funcs
}
