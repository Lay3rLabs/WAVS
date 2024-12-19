use wasmtime::component::bindgen;

bindgen!({
  world: "eth-event-world",
  path: "../../wit",
  async: true,
});
