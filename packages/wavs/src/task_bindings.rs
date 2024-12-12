use wasmtime::component::bindgen;
bindgen!({
  world: "task-queue",
  with: {
      "wasi": wasmtime_wasi::bindings,
      "wasi:http@0.2.0": wasmtime_wasi_http::bindings::http,
  },
  path: "../../wit",
  async: true,
});

// require_store_data_send: true,
