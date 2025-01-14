use wasmtime::component::bindgen;

bindgen!({
  world: "layer-eth-contract-event-world",
  path: "../../sdk/wit",
  async: true,
});
