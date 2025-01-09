#[allow(dead_code)]
pub mod lay3r {
    #[allow(dead_code)]
    pub mod avs {
        #[allow(dead_code, clippy::all)]
        pub mod layer_types {
            #[used]
            #[doc(hidden)]
            static __FORCE_SECTION_REF: fn() =
                super::super::super::__link_custom_section_describing_imports;
            use super::super::super::_rt;
            #[derive(Clone)]
            pub enum Address {
                Eth(_rt::Vec<u8>),
                /// the bech32-encoded address, and length of the prefix
                Cosmos((_rt::String, u32)),
            }
            impl ::core::fmt::Debug for Address {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    match self {
                        Address::Eth(e) => f.debug_tuple("Address::Eth").field(e).finish(),
                        Address::Cosmos(e) => f.debug_tuple("Address::Cosmos").field(e).finish(),
                    }
                }
            }
            #[derive(Clone)]
            pub struct Contract {
                pub address: Address,
                pub chain_id: _rt::String,
            }
            impl ::core::fmt::Debug for Contract {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct("Contract")
                        .field("address", &self.address)
                        .field("chain-id", &self.chain_id)
                        .finish()
                }
            }
            /// An alloy log can be recreated with the info here
            #[derive(Clone)]
            pub struct EthLog {
                /// the raw log topics that can be decoded into an event
                pub topics: _rt::Vec<_rt::Vec<u8>>,
                /// the raw log data that can be decoded into an event
                pub data: _rt::Vec<u8>,
            }
            impl ::core::fmt::Debug for EthLog {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct("EthLog")
                        .field("topics", &self.topics)
                        .field("data", &self.data)
                        .finish()
                }
            }
        }
    }
}
mod _rt {
    pub use alloc_crate::string::String;
    pub use alloc_crate::vec::Vec;
    extern crate alloc as alloc_crate;
}
#[cfg(target_arch = "wasm32")]
#[link_section = "component-type:wit-bindgen:0.35.0:lay3r:avs@0.3.0:layer-sdk-world:encoded world"]
#[doc(hidden)]
pub static __WIT_BINDGEN_COMPONENT_TYPE: [u8; 398] = *b"\
\0asm\x0d\0\x01\0\0\x19\x16wit-component-encoding\x04\0\x07\x88\x02\x01A\x02\x01\
A\x08\x01B\x09\x01p}\x01o\x02sy\x01q\x02\x03eth\x01\0\0\x06cosmos\x01\x01\0\x04\0\
\x07address\x03\0\x02\x01r\x02\x07address\x03\x08chain-ids\x04\0\x08contract\x03\
\0\x04\x01p\0\x01r\x02\x06topics\x06\x04data\0\x04\0\x07eth-log\x03\0\x07\x03\0\x1b\
lay3r:avs/layer-types@0.3.0\x05\0\x02\x03\0\0\x07address\x03\0\x07address\x03\0\x01\
\x02\x03\0\0\x08contract\x03\0\x08contract\x03\0\x03\x02\x03\0\0\x07eth-log\x03\0\
\x07eth-log\x03\0\x05\x04\0\x1flay3r:avs/layer-sdk-world@0.3.0\x04\0\x0b\x15\x01\
\0\x0flayer-sdk-world\x03\0\0\0G\x09producers\x01\x0cprocessed-by\x02\x0dwit-com\
ponent\x070.220.0\x10wit-bindgen-rust\x060.35.0";
#[inline(never)]
#[doc(hidden)]
pub fn __link_custom_section_describing_imports() {
    wit_bindgen_rt::maybe_link_cabi_realloc();
}
