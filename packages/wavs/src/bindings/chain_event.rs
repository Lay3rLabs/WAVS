use wasmtime::component::bindgen;

bindgen!({
  world: "layer-chain-event-world",
  path: "../../sdk/wit",
  async: true,
});
