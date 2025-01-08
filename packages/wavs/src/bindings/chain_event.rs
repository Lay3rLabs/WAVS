use wasmtime::component::bindgen;

bindgen!({
  world: "wavs-chain-event-world",
  path: "../../wit",
  async: true,
});
