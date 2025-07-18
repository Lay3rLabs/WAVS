use wasmtime::component::bindgen;

bindgen!({
    world: "wavs-world",
    path: "../../wit-definitions/worker/wit",
    async: {
        only_imports: []
    },
    with: {
        "wasi:keyvalue/store/bucket": crate::KeyValueBucket,
    },
});
