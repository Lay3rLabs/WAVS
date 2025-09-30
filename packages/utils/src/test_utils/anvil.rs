use alloy_node_bindings::{Anvil, AnvilInstance};

pub fn safe_spawn_anvil() -> AnvilInstance {
    let mut base_port = 8545u16;
    loop {
        match Anvil::new().port(base_port).try_spawn() {
            Ok(instance) => return instance,
            Err(_) => {
                base_port += 1; // Try next port range
            }
        }
    }
}
