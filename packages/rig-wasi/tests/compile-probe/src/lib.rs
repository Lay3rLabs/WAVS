//! Minimal compile probe for rig-wasi on wasm32-wasip2.
//! This component verifies the fork compiles cleanly on the WASI target.
//! It is NOT a functional WAVS component — just a compilation gate (FORK-05).

use wstd::runtime::block_on;

// Verify WasmCompatSend does NOT require Send on wasm32-wasip2.
// On WASM targets, WasmCompatSend is a blanket impl with no Send requirement.
// Note: rig-wasi Cargo.toml sets [lib] name = "rig", so the crate is imported as "rig".
fn _type_check() {
    fn _accepts_wasm_compat<T: rig::wasm_compat::WasmCompatSend>(_: T) {}
}

// Verify block_on works with an async probe.
pub fn run_probe() {
    block_on(async {
        let _ = std::future::ready(42u32).await;
    });
}
