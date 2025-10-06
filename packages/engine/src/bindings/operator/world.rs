// https://docs.rs/wasmtime/latest/wasmtime/component/macro.bindgen.html#options-reference

use wasmtime::component::bindgen;

bindgen!({
    world: "wavs-world",
    path: "../../wit-definitions/operator/wit",
    with: {
        "wasi:keyvalue/store/bucket": crate::backend::wasi_keyvalue::bucket_keys::KeyValueBucket,
        "wasi:keyvalue/atomics/cas": crate::backend::wasi_keyvalue::atomics::KeyValueCas,
    },
});
