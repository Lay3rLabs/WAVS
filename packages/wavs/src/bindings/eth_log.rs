use wasmtime::component::bindgen;

bindgen!({
  world: "wavs-eth-log-world",
  path: "../../wit",
  async: true,
});
