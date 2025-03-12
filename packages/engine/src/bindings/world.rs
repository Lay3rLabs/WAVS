// https://docs.rs/wasmtime/latest/wasmtime/component/macro.bindgen.html#options-reference

use wasmtime::component::bindgen;

bindgen!({
    world: "layer-trigger-world",
    path: "../../wit",
    async: {
        only_imports: []
    }
});
