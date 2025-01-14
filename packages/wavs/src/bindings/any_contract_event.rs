use wasmtime::component::bindgen;

bindgen!({
  world: "layer-any-contract-event-world",
  path: "../../sdk/wit",
  async: true,
});
