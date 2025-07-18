use wasmtime::component::bindgen;

bindgen!({
    world: "aggregator-world",
    path: "../../wit-definitions/aggregator/wit",
    async: {
        only_imports: []
    },
    with: {
        "wasi:keyvalue/store/bucket": crate::keyvalue::bucket_keys::KeyValueBucket,
        "wasi:keyvalue/atomics/cas": crate::keyvalue::atomics::KeyValueCas,
    },
});
