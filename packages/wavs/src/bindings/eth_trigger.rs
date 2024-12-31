use wasmtime::component::bindgen;

bindgen!({
  world: "eth-trigger-world",
  path: "../../wit",
  async: true,
});
