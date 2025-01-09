use wasmtime::component::bindgen;

bindgen!({
  world: "wavs-chain-event-world",
  path: "../../sdk/wit",
  async: true,
});
