use wasmtime::component::bindgen;

bindgen!({
  world: "wavs-eth-log-world",
  path: "../../sdk/wit",
  async: true,
});
