impl From<crate::wit_bindings::AnyContract> for layer_climb_address::Address {
    fn from(contract: crate::wit_bindings::AnyContract) -> layer_climb_address::Address {
        match contract {
            crate::wit_bindings::AnyContract::Eth(contract) => layer_climb_address::Address::Eth(
                layer_climb_address::AddrEth::new_vec(contract).unwrap(),
            ),
            crate::wit_bindings::AnyContract::Cosmos(contract) => {
                layer_climb_address::Address::Cosmos {
                    bech32_addr: contract.bech32_addr,
                    prefix_len: contract.prefix_len as usize,
                }
            }
        }
    }
}

impl From<layer_climb_address::Address> for crate::wit_bindings::AnyContract {
    fn from(address: layer_climb_address::Address) -> crate::wit_bindings::AnyContract {
        match address {
            layer_climb_address::Address::Eth(bytes) => {
                crate::wit_bindings::AnyContract::Eth(bytes.as_bytes().to_vec())
            }
            layer_climb_address::Address::Cosmos {
                bech32_addr,
                prefix_len,
            } => crate::wit_bindings::AnyContract::Cosmos(crate::wit_bindings::CosmosContract {
                bech32_addr,
                prefix_len: prefix_len as u32,
            }),
        }
    }
}

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
