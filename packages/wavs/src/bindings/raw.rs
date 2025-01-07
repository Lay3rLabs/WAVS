use wasmtime::component::bindgen;

bindgen!({
  world: "raw-world",
  path: "../../wit",
  async: true,
});
