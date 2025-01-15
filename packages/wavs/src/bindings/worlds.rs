pub mod any_contract_event {
    use wasmtime::component::bindgen;

    bindgen!({
        world: "layer-any-contract-event-world",
        path: "../../sdk/wit",
        async: true,
    });

    pub use lay3r::avs::layer_types::*;
}

pub mod cosmos_contract_event {
    use wasmtime::component::bindgen;

    bindgen!({
        world: "layer-cosmos-contract-event-world",
        path: "../../sdk/wit",
        async: true,
    });

    pub use lay3r::avs::layer_types::*;
}

pub mod eth_contract_event {
    use wasmtime::component::bindgen;

    bindgen!({
        world: "layer-eth-contract-event-world",
        path: "../../sdk/wit",
        async: true,
    });

    pub use lay3r::avs::layer_types::*;
}

pub mod raw {
    use wasmtime::component::bindgen;

    bindgen!({
        world: "layer-raw-world",
        path: "../../sdk/wit",
        async: true,
    });
}
// with: {
//     "wasi": wasmtime_wasi::bindings,
//     "wasi:http@0.2.0": wasmtime_wasi_http::bindings::http,
// },
