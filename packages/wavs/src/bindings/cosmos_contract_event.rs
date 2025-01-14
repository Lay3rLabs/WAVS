use wasmtime::component::bindgen;

bindgen!({
  world: "layer-cosmos-contract-event-world",
  path: "../../sdk/wit",
  async: true,
});
