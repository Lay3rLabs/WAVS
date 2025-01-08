use wasmtime::component::bindgen;

bindgen!({
  world: "wavs-raw-world",
  // with: {
  //     "wasi": wasmtime_wasi::bindings,
  //     "wasi:http@0.2.0": wasmtime_wasi_http::bindings::http,
  // },
  path: "../../wit",
  async: true,
});
