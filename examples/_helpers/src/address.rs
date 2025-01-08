// parses an address from a component into a climb address
// since components may re-export their address type from different places,
// and this happens by way of bindings (so they are technically different),
// we only care about the overall shape, without depending on implementing a trait
// - but the type must be passed in
// call like `parse_address!(my_wit_bindings::Address, my_component_address)`
#[macro_export]
macro_rules! parse_address {
    (
        $input_enum:ty, // the address type exported from the wit module 
        $addr:expr
    ) => {{
        // see https://github.com/rust-lang/rust/issues/86935#issuecomment-1699575571
        type Alias = $input_enum;

        match $addr {
            Alias::Eth(bytes) => layer_climb_address::Address::Eth(
                layer_climb_address::AddrEth::new_vec(bytes).unwrap(),
            ),
            Alias::Cosmos((bech32_addr, prefix_len)) => layer_climb_address::Address::Cosmos {
                bech32_addr,
                prefix_len: prefix_len as usize,
            },
        }
    }};
}

// like `parse_address!`, but for Ethereum addresses only, gives a Vec<u8>
#[macro_export]
macro_rules! parse_address_eth {
    (
        $input_enum:ty, // the address type exported from the wit module 
        $addr:expr
    ) => {{
        // see https://github.com/rust-lang/rust/issues/86935#issuecomment-1699575571
        type Alias = $input_enum;

        match $addr {
            Alias::Eth(bytes) => bytes,
            Alias::Cosmos(_) => panic!("expected Ethereum address, got Cosmos address"),
        }
    }};
}
