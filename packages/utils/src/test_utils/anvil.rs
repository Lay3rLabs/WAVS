use std::collections::HashSet;

use alloy_node_bindings::{Anvil, AnvilInstance};
use rand::Rng;

pub fn safe_spawn_anvil() -> AnvilInstance {
    let mut attempted_ports = HashSet::new();
    loop {
        // If the port is already in use, try a different one
        let mut rng = rand::rng();
        let port: u16 = rng.random_range(49152..=65535);
        if attempted_ports.contains(&port) {
            continue; // Already tried this port, pick another
        }
        attempted_ports.insert(port);
        if attempted_ports.len() > 1000 {
            panic!("Failed to spawn Anvil after 1000 attempts");
        }
        if let Ok(instance) = Anvil::new().port(port).try_spawn() {
            return instance;
        }
    }
}
