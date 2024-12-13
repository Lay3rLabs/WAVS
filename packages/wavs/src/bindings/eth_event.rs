use wasmtime::component::bindgen;

bindgen!({
  world: "eth-event-world",
  path: "../../wit",
  async: true,
});

// require_store_data_send: true,
