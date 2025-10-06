use wasmtime::component::bindgen;

bindgen!({
    world: "aggregator-world",
    path: "../../wit-definitions/aggregator/wit",
    with: {
        "wasi:keyvalue/store/bucket": crate::backend::wasi_keyvalue::bucket_keys::KeyValueBucket,
        "wasi:keyvalue/atomics/cas": crate::backend::wasi_keyvalue::atomics::KeyValueCas,
    },
    exports: {
        default: async,
    },
});
