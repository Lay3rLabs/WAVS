use wasmtime::component::bindgen;
bindgen!({
  world: "eth-event",
  with: {
      "wasi": wasmtime_wasi::bindings,
      "wasi:http@0.2.0": wasmtime_wasi_http::bindings::http,
  },
  async: true,
});
