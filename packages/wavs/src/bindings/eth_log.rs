use wasmtime::component::bindgen;

bindgen!({
  world: "layer-eth-log-world",
  path: "../../sdk/wit",
  async: true,
});
