#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
pub mod bindings {
    pub mod compat {
        pub use super::world::lay3r::avs::layer_types::*;
        impl From<CosmosEvent> for cosmwasm_std::Event {
            fn from(event: CosmosEvent) -> Self {
                cosmwasm_std::Event::new(event.ty).add_attributes(event.attributes)
            }
        }
        impl From<cosmwasm_std::Event> for CosmosEvent {
            fn from(event: cosmwasm_std::Event) -> Self {
                CosmosEvent {
                    ty: event.ty,
                    attributes: event
                        .attributes
                        .into_iter()
                        .map(|attr| (attr.key, attr.value))
                        .collect(),
                }
            }
        }
        impl From<alloy_primitives::LogData> for EthEventLogData {
            fn from(log_data: alloy_primitives::LogData) -> Self {
                EthEventLogData {
                    topics: log_data
                        .topics()
                        .iter()
                        .map(|topic| topic.to_vec())
                        .collect(),
                    data: log_data.data.to_vec(),
                }
            }
        }
        impl From<EthEventLogData> for alloy_primitives::LogData {
            fn from(log_data: EthEventLogData) -> Self {
                alloy_primitives::LogData::new(
                        log_data
                            .topics
                            .into_iter()
                            .map(|topic| alloy_primitives::FixedBytes::<
                                32,
                            >::from_slice(&topic))
                            .collect(),
                        log_data.data.into(),
                    )
                    .unwrap()
            }
        }
        impl TryFrom<layer_climb::prelude::Address> for CosmosAddress {
            type Error = anyhow::Error;
            fn try_from(
                addr: layer_climb::prelude::Address,
            ) -> Result<Self, Self::Error> {
                match addr {
                    layer_climb::prelude::Address::Cosmos { bech32_addr, prefix_len } => {
                        Ok(CosmosAddress {
                            bech32_addr,
                            prefix_len: prefix_len as u32,
                        })
                    }
                    _ => {
                        Err(
                            ::anyhow::__private::must_use({
                                let error = ::anyhow::__private::format_err(
                                    format_args!("Cannot convert to CosmosAddr"),
                                );
                                error
                            }),
                        )
                    }
                }
            }
        }
        impl From<CosmosAddress> for layer_climb::prelude::Address {
            fn from(addr: CosmosAddress) -> Self {
                layer_climb::prelude::Address::Cosmos {
                    bech32_addr: addr.bech32_addr,
                    prefix_len: addr.prefix_len as usize,
                }
            }
        }
        impl TryFrom<layer_climb::prelude::Address> for EthAddress {
            type Error = anyhow::Error;
            fn try_from(
                addr: layer_climb::prelude::Address,
            ) -> Result<Self, Self::Error> {
                match addr {
                    layer_climb::prelude::Address::Eth(eth) => {
                        Ok(EthAddress {
                            raw_bytes: eth.as_bytes().to_vec(),
                        })
                    }
                    _ => {
                        Err(
                            ::anyhow::__private::must_use({
                                let error = ::anyhow::__private::format_err(
                                    format_args!("Cannot convert to EthAddr"),
                                );
                                error
                            }),
                        )
                    }
                }
            }
        }
        impl From<EthAddress> for layer_climb::prelude::Address {
            fn from(addr: EthAddress) -> Self {
                alloy_primitives::Address::from(addr).into()
            }
        }
        impl From<alloy_primitives::Address> for EthAddress {
            fn from(addr: alloy_primitives::Address) -> Self {
                EthAddress {
                    raw_bytes: addr.to_vec(),
                }
            }
        }
        impl From<EthAddress> for alloy_primitives::Address {
            fn from(addr: EthAddress) -> Self {
                alloy_primitives::Address::from_slice(&addr.raw_bytes)
            }
        }
        impl From<CosmosChainConfig> for layer_climb::prelude::ChainConfig {
            fn from(config: CosmosChainConfig) -> layer_climb::prelude::ChainConfig {
                layer_climb::prelude::ChainConfig {
                    chain_id: layer_climb::prelude::ChainId::new(config.chain_id),
                    rpc_endpoint: config.rpc_endpoint,
                    grpc_endpoint: config.grpc_endpoint,
                    grpc_web_endpoint: config.grpc_web_endpoint,
                    gas_denom: config.gas_denom,
                    gas_price: config.gas_price,
                    address_kind: layer_climb::prelude::AddrKind::Cosmos {
                        prefix: config.bech32_prefix,
                    },
                }
            }
        }
        impl From<layer_climb::prelude::ChainConfig> for CosmosChainConfig {
            fn from(config: layer_climb::prelude::ChainConfig) -> CosmosChainConfig {
                CosmosChainConfig {
                    chain_id: config.chain_id.as_str().to_string(),
                    rpc_endpoint: config.rpc_endpoint,
                    grpc_endpoint: config.grpc_endpoint,
                    grpc_web_endpoint: config.grpc_web_endpoint,
                    gas_denom: config.gas_denom,
                    gas_price: config.gas_price,
                    bech32_prefix: match config.address_kind {
                        layer_climb::prelude::AddrKind::Cosmos { prefix } => prefix,
                        _ => "".to_string(),
                    },
                }
            }
        }
    }
    pub mod world {
        #![allow(clippy::too_many_arguments)]
        pub type TriggerAction = lay3r::avs::layer_types::TriggerAction;
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub unsafe fn _export_run_cabi<T: Guest>(arg0: *mut u8) -> *mut u8 {
            let l0 = *arg0.add(0).cast::<*mut u8>();
            let l1 = *arg0.add(4).cast::<usize>();
            let len2 = l1;
            let bytes2 = _rt::Vec::from_raw_parts(l0.cast(), len2, len2);
            let l3 = *arg0.add(8).cast::<*mut u8>();
            let l4 = *arg0.add(12).cast::<usize>();
            let len5 = l4;
            let bytes5 = _rt::Vec::from_raw_parts(l3.cast(), len5, len5);
            let l6 = i32::from(*arg0.add(16).cast::<u8>());
            use lay3r::avs::layer_types::TriggerSource as V26;
            let v26 = match l6 {
                0 => {
                    let e26 = {
                        let l7 = *arg0.add(20).cast::<*mut u8>();
                        let l8 = *arg0.add(24).cast::<usize>();
                        let len9 = l8;
                        let l10 = *arg0.add(28).cast::<*mut u8>();
                        let l11 = *arg0.add(32).cast::<usize>();
                        let len12 = l11;
                        let bytes12 = _rt::Vec::from_raw_parts(l10.cast(), len12, len12);
                        let l13 = *arg0.add(36).cast::<*mut u8>();
                        let l14 = *arg0.add(40).cast::<usize>();
                        let len15 = l14;
                        lay3r::avs::layer_types::TriggerSourceEthContractEvent {
                            address: lay3r::avs::layer_types::EthAddress {
                                raw_bytes: _rt::Vec::from_raw_parts(l7.cast(), len9, len9),
                            },
                            chain_name: _rt::string_lift(bytes12),
                            event_hash: _rt::Vec::from_raw_parts(
                                l13.cast(),
                                len15,
                                len15,
                            ),
                        }
                    };
                    V26::EthContractEvent(e26)
                }
                1 => {
                    let e26 = {
                        let l16 = *arg0.add(20).cast::<*mut u8>();
                        let l17 = *arg0.add(24).cast::<usize>();
                        let len18 = l17;
                        let bytes18 = _rt::Vec::from_raw_parts(l16.cast(), len18, len18);
                        let l19 = *arg0.add(28).cast::<i32>();
                        let l20 = *arg0.add(32).cast::<*mut u8>();
                        let l21 = *arg0.add(36).cast::<usize>();
                        let len22 = l21;
                        let bytes22 = _rt::Vec::from_raw_parts(l20.cast(), len22, len22);
                        let l23 = *arg0.add(40).cast::<*mut u8>();
                        let l24 = *arg0.add(44).cast::<usize>();
                        let len25 = l24;
                        let bytes25 = _rt::Vec::from_raw_parts(l23.cast(), len25, len25);
                        lay3r::avs::layer_types::TriggerSourceCosmosContractEvent {
                            address: lay3r::avs::layer_types::CosmosAddress {
                                bech32_addr: _rt::string_lift(bytes18),
                                prefix_len: l19 as u32,
                            },
                            chain_name: _rt::string_lift(bytes22),
                            event_type: _rt::string_lift(bytes25),
                        }
                    };
                    V26::CosmosContractEvent(e26)
                }
                n => {
                    if true {
                        match (&n, &2) {
                            (left_val, right_val) => {
                                if !(*left_val == *right_val) {
                                    let kind = ::core::panicking::AssertKind::Eq;
                                    ::core::panicking::assert_failed(
                                        kind,
                                        &*left_val,
                                        &*right_val,
                                        ::core::option::Option::Some(
                                            format_args!("invalid enum discriminant"),
                                        ),
                                    );
                                }
                            }
                        };
                    }
                    V26::Manual
                }
            };
            let l27 = i32::from(*arg0.add(48).cast::<u8>());
            use lay3r::avs::layer_types::TriggerData as V67;
            let v67 = match l27 {
                0 => {
                    let e67 = {
                        let l28 = *arg0.add(56).cast::<*mut u8>();
                        let l29 = *arg0.add(60).cast::<usize>();
                        let len30 = l29;
                        let l31 = *arg0.add(64).cast::<*mut u8>();
                        let l32 = *arg0.add(68).cast::<usize>();
                        let len33 = l32;
                        let bytes33 = _rt::Vec::from_raw_parts(l31.cast(), len33, len33);
                        let l34 = *arg0.add(72).cast::<*mut u8>();
                        let l35 = *arg0.add(76).cast::<usize>();
                        let base39 = l34;
                        let len39 = l35;
                        let mut result39 = _rt::Vec::with_capacity(len39);
                        for i in 0..len39 {
                            let base = base39.add(i * 8);
                            let e39 = {
                                let l36 = *base.add(0).cast::<*mut u8>();
                                let l37 = *base.add(4).cast::<usize>();
                                let len38 = l37;
                                _rt::Vec::from_raw_parts(l36.cast(), len38, len38)
                            };
                            result39.push(e39);
                        }
                        _rt::cabi_dealloc(base39, len39 * 8, 4);
                        let l40 = *arg0.add(80).cast::<*mut u8>();
                        let l41 = *arg0.add(84).cast::<usize>();
                        let len42 = l41;
                        let l43 = *arg0.add(88).cast::<i64>();
                        lay3r::avs::layer_types::TriggerDataEthContractEvent {
                            contract_address: lay3r::avs::layer_types::EthAddress {
                                raw_bytes: _rt::Vec::from_raw_parts(
                                    l28.cast(),
                                    len30,
                                    len30,
                                ),
                            },
                            chain_name: _rt::string_lift(bytes33),
                            log: lay3r::avs::layer_types::EthEventLogData {
                                topics: result39,
                                data: _rt::Vec::from_raw_parts(l40.cast(), len42, len42),
                            },
                            block_height: l43 as u64,
                        }
                    };
                    V67::EthContractEvent(e67)
                }
                1 => {
                    let e67 = {
                        let l44 = *arg0.add(56).cast::<*mut u8>();
                        let l45 = *arg0.add(60).cast::<usize>();
                        let len46 = l45;
                        let bytes46 = _rt::Vec::from_raw_parts(l44.cast(), len46, len46);
                        let l47 = *arg0.add(64).cast::<i32>();
                        let l48 = *arg0.add(68).cast::<*mut u8>();
                        let l49 = *arg0.add(72).cast::<usize>();
                        let len50 = l49;
                        let bytes50 = _rt::Vec::from_raw_parts(l48.cast(), len50, len50);
                        let l51 = *arg0.add(76).cast::<*mut u8>();
                        let l52 = *arg0.add(80).cast::<usize>();
                        let len53 = l52;
                        let bytes53 = _rt::Vec::from_raw_parts(l51.cast(), len53, len53);
                        let l54 = *arg0.add(84).cast::<*mut u8>();
                        let l55 = *arg0.add(88).cast::<usize>();
                        let base62 = l54;
                        let len62 = l55;
                        let mut result62 = _rt::Vec::with_capacity(len62);
                        for i in 0..len62 {
                            let base = base62.add(i * 16);
                            let e62 = {
                                let l56 = *base.add(0).cast::<*mut u8>();
                                let l57 = *base.add(4).cast::<usize>();
                                let len58 = l57;
                                let bytes58 = _rt::Vec::from_raw_parts(
                                    l56.cast(),
                                    len58,
                                    len58,
                                );
                                let l59 = *base.add(8).cast::<*mut u8>();
                                let l60 = *base.add(12).cast::<usize>();
                                let len61 = l60;
                                let bytes61 = _rt::Vec::from_raw_parts(
                                    l59.cast(),
                                    len61,
                                    len61,
                                );
                                (_rt::string_lift(bytes58), _rt::string_lift(bytes61))
                            };
                            result62.push(e62);
                        }
                        _rt::cabi_dealloc(base62, len62 * 16, 4);
                        let l63 = *arg0.add(96).cast::<i64>();
                        lay3r::avs::layer_types::TriggerDataCosmosContractEvent {
                            contract_address: lay3r::avs::layer_types::CosmosAddress {
                                bech32_addr: _rt::string_lift(bytes46),
                                prefix_len: l47 as u32,
                            },
                            chain_name: _rt::string_lift(bytes50),
                            event: lay3r::avs::layer_types::CosmosEvent {
                                ty: _rt::string_lift(bytes53),
                                attributes: result62,
                            },
                            block_height: l63 as u64,
                        }
                    };
                    V67::CosmosContractEvent(e67)
                }
                n => {
                    if true {
                        match (&n, &2) {
                            (left_val, right_val) => {
                                if !(*left_val == *right_val) {
                                    let kind = ::core::panicking::AssertKind::Eq;
                                    ::core::panicking::assert_failed(
                                        kind,
                                        &*left_val,
                                        &*right_val,
                                        ::core::option::Option::Some(
                                            format_args!("invalid enum discriminant"),
                                        ),
                                    );
                                }
                            }
                        };
                    }
                    let e67 = {
                        let l64 = *arg0.add(56).cast::<*mut u8>();
                        let l65 = *arg0.add(60).cast::<usize>();
                        let len66 = l65;
                        _rt::Vec::from_raw_parts(l64.cast(), len66, len66)
                    };
                    V67::Raw(e67)
                }
            };
            let result68 = T::run(lay3r::avs::layer_types::TriggerAction {
                config: lay3r::avs::layer_types::TriggerConfig {
                    service_id: _rt::string_lift(bytes2),
                    workflow_id: _rt::string_lift(bytes5),
                    trigger_source: v26,
                },
                data: v67,
            });
            _rt::cabi_dealloc(arg0, 104, 8);
            let ptr69 = _RET_AREA.0.as_mut_ptr().cast::<u8>();
            match result68 {
                Ok(e) => {
                    *ptr69.add(0).cast::<u8>() = (0i32) as u8;
                    let vec70 = (e).into_boxed_slice();
                    let ptr70 = vec70.as_ptr().cast::<u8>();
                    let len70 = vec70.len();
                    ::core::mem::forget(vec70);
                    *ptr69.add(8).cast::<usize>() = len70;
                    *ptr69.add(4).cast::<*mut u8>() = ptr70.cast_mut();
                }
                Err(e) => {
                    *ptr69.add(0).cast::<u8>() = (1i32) as u8;
                    let vec71 = (e.into_bytes()).into_boxed_slice();
                    let ptr71 = vec71.as_ptr().cast::<u8>();
                    let len71 = vec71.len();
                    ::core::mem::forget(vec71);
                    *ptr69.add(8).cast::<usize>() = len71;
                    *ptr69.add(4).cast::<*mut u8>() = ptr71.cast_mut();
                }
            };
            ptr69
        }
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub unsafe fn __post_return_run<T: Guest>(arg0: *mut u8) {
            let l0 = i32::from(*arg0.add(0).cast::<u8>());
            match l0 {
                0 => {
                    let l1 = *arg0.add(4).cast::<*mut u8>();
                    let l2 = *arg0.add(8).cast::<usize>();
                    let base3 = l1;
                    let len3 = l2;
                    _rt::cabi_dealloc(base3, len3 * 1, 1);
                }
                _ => {
                    let l4 = *arg0.add(4).cast::<*mut u8>();
                    let l5 = *arg0.add(8).cast::<usize>();
                    _rt::cabi_dealloc(l4, l5, 1);
                }
            }
        }
        pub trait Guest {
            fn run(trigger_action: TriggerAction) -> Result<_rt::Vec<u8>, _rt::String>;
        }
        #[doc(hidden)]
        pub use __export_world_layer_trigger_world_cabi;
        #[repr(align(4))]
        struct _RetArea([::core::mem::MaybeUninit<u8>; 12]);
        static mut _RET_AREA: _RetArea = _RetArea(
            [::core::mem::MaybeUninit::uninit(); 12],
        );
        #[allow(dead_code, clippy::all)]
        pub mod lay3r {
            pub mod avs {
                #[allow(dead_code, unused_imports, clippy::all)]
                pub mod layer_types {
                    #[used]
                    #[doc(hidden)]
                    static __FORCE_SECTION_REF: fn() = super::super::super::__link_custom_section_describing_imports;
                    use super::super::super::_rt;
                    pub struct CosmosAddress {
                        pub bech32_addr: _rt::String,
                        /// prefix is the first part of the bech32 address
                        pub prefix_len: u32,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for CosmosAddress {
                        #[inline]
                        fn clone(&self) -> CosmosAddress {
                            CosmosAddress {
                                bech32_addr: ::core::clone::Clone::clone(&self.bech32_addr),
                                prefix_len: ::core::clone::Clone::clone(&self.prefix_len),
                            }
                        }
                    }
                    impl ::core::fmt::Debug for CosmosAddress {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            f.debug_struct("CosmosAddress")
                                .field("bech32-addr", &self.bech32_addr)
                                .field("prefix-len", &self.prefix_len)
                                .finish()
                        }
                    }
                    pub struct CosmosEvent {
                        pub ty: _rt::String,
                        pub attributes: _rt::Vec<(_rt::String, _rt::String)>,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for CosmosEvent {
                        #[inline]
                        fn clone(&self) -> CosmosEvent {
                            CosmosEvent {
                                ty: ::core::clone::Clone::clone(&self.ty),
                                attributes: ::core::clone::Clone::clone(&self.attributes),
                            }
                        }
                    }
                    impl ::core::fmt::Debug for CosmosEvent {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            f.debug_struct("CosmosEvent")
                                .field("ty", &self.ty)
                                .field("attributes", &self.attributes)
                                .finish()
                        }
                    }
                    pub struct CosmosChainConfig {
                        pub chain_id: _rt::String,
                        pub rpc_endpoint: Option<_rt::String>,
                        pub grpc_endpoint: Option<_rt::String>,
                        pub grpc_web_endpoint: Option<_rt::String>,
                        pub gas_price: f32,
                        pub gas_denom: _rt::String,
                        pub bech32_prefix: _rt::String,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for CosmosChainConfig {
                        #[inline]
                        fn clone(&self) -> CosmosChainConfig {
                            CosmosChainConfig {
                                chain_id: ::core::clone::Clone::clone(&self.chain_id),
                                rpc_endpoint: ::core::clone::Clone::clone(
                                    &self.rpc_endpoint,
                                ),
                                grpc_endpoint: ::core::clone::Clone::clone(
                                    &self.grpc_endpoint,
                                ),
                                grpc_web_endpoint: ::core::clone::Clone::clone(
                                    &self.grpc_web_endpoint,
                                ),
                                gas_price: ::core::clone::Clone::clone(&self.gas_price),
                                gas_denom: ::core::clone::Clone::clone(&self.gas_denom),
                                bech32_prefix: ::core::clone::Clone::clone(
                                    &self.bech32_prefix,
                                ),
                            }
                        }
                    }
                    impl ::core::fmt::Debug for CosmosChainConfig {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            f.debug_struct("CosmosChainConfig")
                                .field("chain-id", &self.chain_id)
                                .field("rpc-endpoint", &self.rpc_endpoint)
                                .field("grpc-endpoint", &self.grpc_endpoint)
                                .field("grpc-web-endpoint", &self.grpc_web_endpoint)
                                .field("gas-price", &self.gas_price)
                                .field("gas-denom", &self.gas_denom)
                                .field("bech32-prefix", &self.bech32_prefix)
                                .finish()
                        }
                    }
                    pub struct EthAddress {
                        pub raw_bytes: _rt::Vec<u8>,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for EthAddress {
                        #[inline]
                        fn clone(&self) -> EthAddress {
                            EthAddress {
                                raw_bytes: ::core::clone::Clone::clone(&self.raw_bytes),
                            }
                        }
                    }
                    impl ::core::fmt::Debug for EthAddress {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            f.debug_struct("EthAddress")
                                .field("raw-bytes", &self.raw_bytes)
                                .finish()
                        }
                    }
                    pub struct EthEventLogData {
                        /// the raw log topics that can be decoded into an event
                        pub topics: _rt::Vec<_rt::Vec<u8>>,
                        /// the raw log data that can be decoded into an event
                        pub data: _rt::Vec<u8>,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for EthEventLogData {
                        #[inline]
                        fn clone(&self) -> EthEventLogData {
                            EthEventLogData {
                                topics: ::core::clone::Clone::clone(&self.topics),
                                data: ::core::clone::Clone::clone(&self.data),
                            }
                        }
                    }
                    impl ::core::fmt::Debug for EthEventLogData {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            f.debug_struct("EthEventLogData")
                                .field("topics", &self.topics)
                                .field("data", &self.data)
                                .finish()
                        }
                    }
                    pub struct EthChainConfig {
                        pub chain_id: _rt::String,
                        pub ws_endpoint: Option<_rt::String>,
                        pub http_endpoint: Option<_rt::String>,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for EthChainConfig {
                        #[inline]
                        fn clone(&self) -> EthChainConfig {
                            EthChainConfig {
                                chain_id: ::core::clone::Clone::clone(&self.chain_id),
                                ws_endpoint: ::core::clone::Clone::clone(&self.ws_endpoint),
                                http_endpoint: ::core::clone::Clone::clone(
                                    &self.http_endpoint,
                                ),
                            }
                        }
                    }
                    impl ::core::fmt::Debug for EthChainConfig {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            f.debug_struct("EthChainConfig")
                                .field("chain-id", &self.chain_id)
                                .field("ws-endpoint", &self.ws_endpoint)
                                .field("http-endpoint", &self.http_endpoint)
                                .finish()
                        }
                    }
                    pub struct TriggerSourceEthContractEvent {
                        pub address: EthAddress,
                        pub chain_name: _rt::String,
                        pub event_hash: _rt::Vec<u8>,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for TriggerSourceEthContractEvent {
                        #[inline]
                        fn clone(&self) -> TriggerSourceEthContractEvent {
                            TriggerSourceEthContractEvent {
                                address: ::core::clone::Clone::clone(&self.address),
                                chain_name: ::core::clone::Clone::clone(&self.chain_name),
                                event_hash: ::core::clone::Clone::clone(&self.event_hash),
                            }
                        }
                    }
                    impl ::core::fmt::Debug for TriggerSourceEthContractEvent {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            f.debug_struct("TriggerSourceEthContractEvent")
                                .field("address", &self.address)
                                .field("chain-name", &self.chain_name)
                                .field("event-hash", &self.event_hash)
                                .finish()
                        }
                    }
                    pub struct TriggerSourceCosmosContractEvent {
                        pub address: CosmosAddress,
                        pub chain_name: _rt::String,
                        pub event_type: _rt::String,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for TriggerSourceCosmosContractEvent {
                        #[inline]
                        fn clone(&self) -> TriggerSourceCosmosContractEvent {
                            TriggerSourceCosmosContractEvent {
                                address: ::core::clone::Clone::clone(&self.address),
                                chain_name: ::core::clone::Clone::clone(&self.chain_name),
                                event_type: ::core::clone::Clone::clone(&self.event_type),
                            }
                        }
                    }
                    impl ::core::fmt::Debug for TriggerSourceCosmosContractEvent {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            f.debug_struct("TriggerSourceCosmosContractEvent")
                                .field("address", &self.address)
                                .field("chain-name", &self.chain_name)
                                .field("event-type", &self.event_type)
                                .finish()
                        }
                    }
                    pub enum TriggerSource {
                        EthContractEvent(TriggerSourceEthContractEvent),
                        CosmosContractEvent(TriggerSourceCosmosContractEvent),
                        Manual,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for TriggerSource {
                        #[inline]
                        fn clone(&self) -> TriggerSource {
                            match self {
                                TriggerSource::EthContractEvent(__self_0) => {
                                    TriggerSource::EthContractEvent(
                                        ::core::clone::Clone::clone(__self_0),
                                    )
                                }
                                TriggerSource::CosmosContractEvent(__self_0) => {
                                    TriggerSource::CosmosContractEvent(
                                        ::core::clone::Clone::clone(__self_0),
                                    )
                                }
                                TriggerSource::Manual => TriggerSource::Manual,
                            }
                        }
                    }
                    impl ::core::fmt::Debug for TriggerSource {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            match self {
                                TriggerSource::EthContractEvent(e) => {
                                    f.debug_tuple("TriggerSource::EthContractEvent")
                                        .field(e)
                                        .finish()
                                }
                                TriggerSource::CosmosContractEvent(e) => {
                                    f.debug_tuple("TriggerSource::CosmosContractEvent")
                                        .field(e)
                                        .finish()
                                }
                                TriggerSource::Manual => {
                                    f.debug_tuple("TriggerSource::Manual").finish()
                                }
                            }
                        }
                    }
                    pub struct TriggerConfig {
                        pub service_id: _rt::String,
                        pub workflow_id: _rt::String,
                        pub trigger_source: TriggerSource,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for TriggerConfig {
                        #[inline]
                        fn clone(&self) -> TriggerConfig {
                            TriggerConfig {
                                service_id: ::core::clone::Clone::clone(&self.service_id),
                                workflow_id: ::core::clone::Clone::clone(&self.workflow_id),
                                trigger_source: ::core::clone::Clone::clone(
                                    &self.trigger_source,
                                ),
                            }
                        }
                    }
                    impl ::core::fmt::Debug for TriggerConfig {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            f.debug_struct("TriggerConfig")
                                .field("service-id", &self.service_id)
                                .field("workflow-id", &self.workflow_id)
                                .field("trigger-source", &self.trigger_source)
                                .finish()
                        }
                    }
                    pub struct TriggerDataEthContractEvent {
                        pub contract_address: EthAddress,
                        pub chain_name: _rt::String,
                        pub log: EthEventLogData,
                        pub block_height: u64,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for TriggerDataEthContractEvent {
                        #[inline]
                        fn clone(&self) -> TriggerDataEthContractEvent {
                            TriggerDataEthContractEvent {
                                contract_address: ::core::clone::Clone::clone(
                                    &self.contract_address,
                                ),
                                chain_name: ::core::clone::Clone::clone(&self.chain_name),
                                log: ::core::clone::Clone::clone(&self.log),
                                block_height: ::core::clone::Clone::clone(
                                    &self.block_height,
                                ),
                            }
                        }
                    }
                    impl ::core::fmt::Debug for TriggerDataEthContractEvent {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            f.debug_struct("TriggerDataEthContractEvent")
                                .field("contract-address", &self.contract_address)
                                .field("chain-name", &self.chain_name)
                                .field("log", &self.log)
                                .field("block-height", &self.block_height)
                                .finish()
                        }
                    }
                    pub struct TriggerDataCosmosContractEvent {
                        pub contract_address: CosmosAddress,
                        pub chain_name: _rt::String,
                        pub event: CosmosEvent,
                        pub block_height: u64,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for TriggerDataCosmosContractEvent {
                        #[inline]
                        fn clone(&self) -> TriggerDataCosmosContractEvent {
                            TriggerDataCosmosContractEvent {
                                contract_address: ::core::clone::Clone::clone(
                                    &self.contract_address,
                                ),
                                chain_name: ::core::clone::Clone::clone(&self.chain_name),
                                event: ::core::clone::Clone::clone(&self.event),
                                block_height: ::core::clone::Clone::clone(
                                    &self.block_height,
                                ),
                            }
                        }
                    }
                    impl ::core::fmt::Debug for TriggerDataCosmosContractEvent {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            f.debug_struct("TriggerDataCosmosContractEvent")
                                .field("contract-address", &self.contract_address)
                                .field("chain-name", &self.chain_name)
                                .field("event", &self.event)
                                .field("block-height", &self.block_height)
                                .finish()
                        }
                    }
                    pub enum TriggerData {
                        EthContractEvent(TriggerDataEthContractEvent),
                        CosmosContractEvent(TriggerDataCosmosContractEvent),
                        Raw(_rt::Vec<u8>),
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for TriggerData {
                        #[inline]
                        fn clone(&self) -> TriggerData {
                            match self {
                                TriggerData::EthContractEvent(__self_0) => {
                                    TriggerData::EthContractEvent(
                                        ::core::clone::Clone::clone(__self_0),
                                    )
                                }
                                TriggerData::CosmosContractEvent(__self_0) => {
                                    TriggerData::CosmosContractEvent(
                                        ::core::clone::Clone::clone(__self_0),
                                    )
                                }
                                TriggerData::Raw(__self_0) => {
                                    TriggerData::Raw(::core::clone::Clone::clone(__self_0))
                                }
                            }
                        }
                    }
                    impl ::core::fmt::Debug for TriggerData {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            match self {
                                TriggerData::EthContractEvent(e) => {
                                    f.debug_tuple("TriggerData::EthContractEvent")
                                        .field(e)
                                        .finish()
                                }
                                TriggerData::CosmosContractEvent(e) => {
                                    f.debug_tuple("TriggerData::CosmosContractEvent")
                                        .field(e)
                                        .finish()
                                }
                                TriggerData::Raw(e) => {
                                    f.debug_tuple("TriggerData::Raw").field(e).finish()
                                }
                            }
                        }
                    }
                    pub struct TriggerAction {
                        pub config: TriggerConfig,
                        pub data: TriggerData,
                    }
                    #[automatically_derived]
                    impl ::core::clone::Clone for TriggerAction {
                        #[inline]
                        fn clone(&self) -> TriggerAction {
                            TriggerAction {
                                config: ::core::clone::Clone::clone(&self.config),
                                data: ::core::clone::Clone::clone(&self.data),
                            }
                        }
                    }
                    impl ::core::fmt::Debug for TriggerAction {
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter<'_>,
                        ) -> ::core::fmt::Result {
                            f.debug_struct("TriggerAction")
                                .field("config", &self.config)
                                .field("data", &self.data)
                                .finish()
                        }
                    }
                }
            }
        }
        #[allow(dead_code, clippy::all)]
        pub mod wasi {
            pub mod io0_2_2 {
                /// A poll API intended to let users wait for I/O events on multiple handles
                /// at once.
                #[allow(dead_code, unused_imports, clippy::all)]
                pub mod poll {
                    #[used]
                    #[doc(hidden)]
                    static __FORCE_SECTION_REF: fn() = super::super::super::__link_custom_section_describing_imports;
                    use super::super::super::_rt;
                    /// `pollable` represents a single I/O event which may be ready, or not.
                    #[repr(transparent)]
                    pub struct Pollable {
                        handle: _rt::Resource<Pollable>,
                    }
                    #[automatically_derived]
                    impl ::core::fmt::Debug for Pollable {
                        #[inline]
                        fn fmt(
                            &self,
                            f: &mut ::core::fmt::Formatter,
                        ) -> ::core::fmt::Result {
                            ::core::fmt::Formatter::debug_struct_field1_finish(
                                f,
                                "Pollable",
                                "handle",
                                &&self.handle,
                            )
                        }
                    }
                    impl Pollable {
                        #[doc(hidden)]
                        pub unsafe fn from_handle(handle: u32) -> Self {
                            Self {
                                handle: _rt::Resource::from_handle(handle),
                            }
                        }
                        #[doc(hidden)]
                        pub fn take_handle(&self) -> u32 {
                            _rt::Resource::take_handle(&self.handle)
                        }
                        #[doc(hidden)]
                        pub fn handle(&self) -> u32 {
                            _rt::Resource::handle(&self.handle)
                        }
                    }
                    unsafe impl _rt::WasmResource for Pollable {
                        #[inline]
                        unsafe fn drop(_handle: u32) {
                            ::core::panicking::panic(
                                "internal error: entered unreachable code",
                            );
                        }
                    }
                    impl Pollable {
                        #[allow(unused_unsafe, clippy::all)]
                        /// Return the readiness of a pollable. This function never blocks.
                        ///
                        /// Returns `true` when the pollable is ready, and `false` otherwise.
                        pub fn ready(&self) -> bool {
                            unsafe {
                                #[cfg(not(target_arch = "wasm32"))]
                                extern "C" fn wit_import0(_: i32) -> i32 {
                                    ::core::panicking::panic(
                                        "internal error: entered unreachable code",
                                    )
                                }
                                let ret = wit_import0((self).handle() as i32);
                                _rt::bool_lift(ret as u8)
                            }
                        }
                    }
                    impl Pollable {
                        #[allow(unused_unsafe, clippy::all)]
                        /// `block` returns immediately if the pollable is ready, and otherwise
                        /// blocks until ready.
                        ///
                        /// This function is equivalent to calling `poll.poll` on a list
                        /// containing only this pollable.
                        pub fn block(&self) -> () {
                            unsafe {
                                #[cfg(not(target_arch = "wasm32"))]
                                extern "C" fn wit_import0(_: i32) {
                                    ::core::panicking::panic(
                                        "internal error: entered unreachable code",
                                    )
                                }
                                wit_import0((self).handle() as i32);
                            }
                        }
                    }
                    #[allow(unused_unsafe, clippy::all)]
                    /// Poll for completion on a set of pollables.
                    ///
                    /// This function takes a list of pollables, which identify I/O sources of
                    /// interest, and waits until one or more of the events is ready for I/O.
                    ///
                    /// The result `list<u32>` contains one or more indices of handles in the
                    /// argument list that is ready for I/O.
                    ///
                    /// This function traps if either:
                    /// - the list is empty, or:
                    /// - the list contains more elements than can be indexed with a `u32` value.
                    ///
                    /// A timeout can be implemented by adding a pollable from the
                    /// wasi-clocks API to the list.
                    ///
                    /// This function does not return a `result`; polling in itself does not
                    /// do any I/O so it doesn't fail. If any of the I/O sources identified by
                    /// the pollables has an error, it is indicated by marking the source as
                    /// being ready for I/O.
                    pub fn poll(in_: &[&Pollable]) -> _rt::Vec<u32> {
                        unsafe {
                            #[repr(align(4))]
                            struct RetArea([::core::mem::MaybeUninit<u8>; 8]);
                            let mut ret_area = RetArea(
                                [::core::mem::MaybeUninit::uninit(); 8],
                            );
                            let vec0 = in_;
                            let len0 = vec0.len();
                            let layout0 = _rt::alloc::Layout::from_size_align_unchecked(
                                vec0.len() * 4,
                                4,
                            );
                            let result0 = if layout0.size() != 0 {
                                let ptr = _rt::alloc::alloc(layout0).cast::<u8>();
                                if ptr.is_null() {
                                    _rt::alloc::handle_alloc_error(layout0);
                                }
                                ptr
                            } else {
                                ::core::ptr::null_mut()
                            };
                            for (i, e) in vec0.into_iter().enumerate() {
                                let base = result0.add(i * 4);
                                {
                                    *base.add(0).cast::<i32>() = (e).handle() as i32;
                                }
                            }
                            let ptr1 = ret_area.0.as_mut_ptr().cast::<u8>();
                            #[cfg(not(target_arch = "wasm32"))]
                            extern "C" fn wit_import2(_: *mut u8, _: usize, _: *mut u8) {
                                ::core::panicking::panic(
                                    "internal error: entered unreachable code",
                                )
                            }
                            wit_import2(result0, len0, ptr1);
                            let l3 = *ptr1.add(0).cast::<*mut u8>();
                            let l4 = *ptr1.add(4).cast::<usize>();
                            let len5 = l4;
                            let result6 = _rt::Vec::from_raw_parts(
                                l3.cast(),
                                len5,
                                len5,
                            );
                            if layout0.size() != 0 {
                                _rt::alloc::dealloc(result0.cast(), layout0);
                            }
                            result6
                        }
                    }
                }
            }
        }
        #[allow(dead_code, unused_imports, clippy::all)]
        pub mod host {
            #[used]
            #[doc(hidden)]
            static __FORCE_SECTION_REF: fn() = super::__link_custom_section_describing_imports;
            use super::_rt;
            pub type EthChainConfig = super::lay3r::avs::layer_types::EthChainConfig;
            pub type CosmosChainConfig = super::lay3r::avs::layer_types::CosmosChainConfig;
            #[allow(unused_unsafe, clippy::all)]
            pub fn get_eth_chain_config(chain_name: &str) -> Option<EthChainConfig> {
                unsafe {
                    #[repr(align(4))]
                    struct RetArea([::core::mem::MaybeUninit<u8>; 36]);
                    let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 36]);
                    let vec0 = chain_name;
                    let ptr0 = vec0.as_ptr().cast::<u8>();
                    let len0 = vec0.len();
                    let ptr1 = ret_area.0.as_mut_ptr().cast::<u8>();
                    #[cfg(not(target_arch = "wasm32"))]
                    extern "C" fn wit_import2(_: *mut u8, _: usize, _: *mut u8) {
                        ::core::panicking::panic(
                            "internal error: entered unreachable code",
                        )
                    }
                    wit_import2(ptr0.cast_mut(), len0, ptr1);
                    let l3 = i32::from(*ptr1.add(0).cast::<u8>());
                    let result15 = match l3 {
                        0 => None,
                        1 => {
                            let e = {
                                let l4 = *ptr1.add(4).cast::<*mut u8>();
                                let l5 = *ptr1.add(8).cast::<usize>();
                                let len6 = l5;
                                let bytes6 = _rt::Vec::from_raw_parts(
                                    l4.cast(),
                                    len6,
                                    len6,
                                );
                                let l7 = i32::from(*ptr1.add(12).cast::<u8>());
                                let l11 = i32::from(*ptr1.add(24).cast::<u8>());
                                super::lay3r::avs::layer_types::EthChainConfig {
                                    chain_id: _rt::string_lift(bytes6),
                                    ws_endpoint: match l7 {
                                        0 => None,
                                        1 => {
                                            let e = {
                                                let l8 = *ptr1.add(16).cast::<*mut u8>();
                                                let l9 = *ptr1.add(20).cast::<usize>();
                                                let len10 = l9;
                                                let bytes10 = _rt::Vec::from_raw_parts(
                                                    l8.cast(),
                                                    len10,
                                                    len10,
                                                );
                                                _rt::string_lift(bytes10)
                                            };
                                            Some(e)
                                        }
                                        _ => _rt::invalid_enum_discriminant(),
                                    },
                                    http_endpoint: match l11 {
                                        0 => None,
                                        1 => {
                                            let e = {
                                                let l12 = *ptr1.add(28).cast::<*mut u8>();
                                                let l13 = *ptr1.add(32).cast::<usize>();
                                                let len14 = l13;
                                                let bytes14 = _rt::Vec::from_raw_parts(
                                                    l12.cast(),
                                                    len14,
                                                    len14,
                                                );
                                                _rt::string_lift(bytes14)
                                            };
                                            Some(e)
                                        }
                                        _ => _rt::invalid_enum_discriminant(),
                                    },
                                }
                            };
                            Some(e)
                        }
                        _ => _rt::invalid_enum_discriminant(),
                    };
                    result15
                }
            }
            #[allow(unused_unsafe, clippy::all)]
            pub fn get_cosmos_chain_config(
                chain_name: &str,
            ) -> Option<CosmosChainConfig> {
                unsafe {
                    #[repr(align(4))]
                    struct RetArea([::core::mem::MaybeUninit<u8>; 68]);
                    let mut ret_area = RetArea([::core::mem::MaybeUninit::uninit(); 68]);
                    let vec0 = chain_name;
                    let ptr0 = vec0.as_ptr().cast::<u8>();
                    let len0 = vec0.len();
                    let ptr1 = ret_area.0.as_mut_ptr().cast::<u8>();
                    #[cfg(not(target_arch = "wasm32"))]
                    extern "C" fn wit_import2(_: *mut u8, _: usize, _: *mut u8) {
                        ::core::panicking::panic(
                            "internal error: entered unreachable code",
                        )
                    }
                    wit_import2(ptr0.cast_mut(), len0, ptr1);
                    let l3 = i32::from(*ptr1.add(0).cast::<u8>());
                    let result26 = match l3 {
                        0 => None,
                        1 => {
                            let e = {
                                let l4 = *ptr1.add(4).cast::<*mut u8>();
                                let l5 = *ptr1.add(8).cast::<usize>();
                                let len6 = l5;
                                let bytes6 = _rt::Vec::from_raw_parts(
                                    l4.cast(),
                                    len6,
                                    len6,
                                );
                                let l7 = i32::from(*ptr1.add(12).cast::<u8>());
                                let l11 = i32::from(*ptr1.add(24).cast::<u8>());
                                let l15 = i32::from(*ptr1.add(36).cast::<u8>());
                                let l19 = *ptr1.add(48).cast::<f32>();
                                let l20 = *ptr1.add(52).cast::<*mut u8>();
                                let l21 = *ptr1.add(56).cast::<usize>();
                                let len22 = l21;
                                let bytes22 = _rt::Vec::from_raw_parts(
                                    l20.cast(),
                                    len22,
                                    len22,
                                );
                                let l23 = *ptr1.add(60).cast::<*mut u8>();
                                let l24 = *ptr1.add(64).cast::<usize>();
                                let len25 = l24;
                                let bytes25 = _rt::Vec::from_raw_parts(
                                    l23.cast(),
                                    len25,
                                    len25,
                                );
                                super::lay3r::avs::layer_types::CosmosChainConfig {
                                    chain_id: _rt::string_lift(bytes6),
                                    rpc_endpoint: match l7 {
                                        0 => None,
                                        1 => {
                                            let e = {
                                                let l8 = *ptr1.add(16).cast::<*mut u8>();
                                                let l9 = *ptr1.add(20).cast::<usize>();
                                                let len10 = l9;
                                                let bytes10 = _rt::Vec::from_raw_parts(
                                                    l8.cast(),
                                                    len10,
                                                    len10,
                                                );
                                                _rt::string_lift(bytes10)
                                            };
                                            Some(e)
                                        }
                                        _ => _rt::invalid_enum_discriminant(),
                                    },
                                    grpc_endpoint: match l11 {
                                        0 => None,
                                        1 => {
                                            let e = {
                                                let l12 = *ptr1.add(28).cast::<*mut u8>();
                                                let l13 = *ptr1.add(32).cast::<usize>();
                                                let len14 = l13;
                                                let bytes14 = _rt::Vec::from_raw_parts(
                                                    l12.cast(),
                                                    len14,
                                                    len14,
                                                );
                                                _rt::string_lift(bytes14)
                                            };
                                            Some(e)
                                        }
                                        _ => _rt::invalid_enum_discriminant(),
                                    },
                                    grpc_web_endpoint: match l15 {
                                        0 => None,
                                        1 => {
                                            let e = {
                                                let l16 = *ptr1.add(40).cast::<*mut u8>();
                                                let l17 = *ptr1.add(44).cast::<usize>();
                                                let len18 = l17;
                                                let bytes18 = _rt::Vec::from_raw_parts(
                                                    l16.cast(),
                                                    len18,
                                                    len18,
                                                );
                                                _rt::string_lift(bytes18)
                                            };
                                            Some(e)
                                        }
                                        _ => _rt::invalid_enum_discriminant(),
                                    },
                                    gas_price: l19,
                                    gas_denom: _rt::string_lift(bytes22),
                                    bech32_prefix: _rt::string_lift(bytes25),
                                }
                            };
                            Some(e)
                        }
                        _ => _rt::invalid_enum_discriminant(),
                    };
                    result26
                }
            }
        }
        mod _rt {
            #![allow(dead_code, clippy::all)]
            pub use alloc_crate::string::String;
            pub use alloc_crate::vec::Vec;
            use core::fmt;
            use core::marker;
            use core::sync::atomic::{AtomicU32, Ordering::Relaxed};
            /// A type which represents a component model resource, either imported or
            /// exported into this component.
            ///
            /// This is a low-level wrapper which handles the lifetime of the resource
            /// (namely this has a destructor). The `T` provided defines the component model
            /// intrinsics that this wrapper uses.
            ///
            /// One of the chief purposes of this type is to provide `Deref` implementations
            /// to access the underlying data when it is owned.
            ///
            /// This type is primarily used in generated code for exported and imported
            /// resources.
            #[repr(transparent)]
            pub struct Resource<T: WasmResource> {
                handle: AtomicU32,
                _marker: marker::PhantomData<T>,
            }
            /// A trait which all wasm resources implement, namely providing the ability to
            /// drop a resource.
            ///
            /// This generally is implemented by generated code, not user-facing code.
            #[allow(clippy::missing_safety_doc)]
            pub unsafe trait WasmResource {
                /// Invokes the `[resource-drop]...` intrinsic.
                unsafe fn drop(handle: u32);
            }
            impl<T: WasmResource> Resource<T> {
                #[doc(hidden)]
                pub unsafe fn from_handle(handle: u32) -> Self {
                    if true {
                        if !(handle != u32::MAX) {
                            ::core::panicking::panic(
                                "assertion failed: handle != u32::MAX",
                            )
                        }
                    }
                    Self {
                        handle: AtomicU32::new(handle),
                        _marker: marker::PhantomData,
                    }
                }
                /// Takes ownership of the handle owned by `resource`.
                ///
                /// Note that this ideally would be `into_handle` taking `Resource<T>` by
                /// ownership. The code generator does not enable that in all situations,
                /// unfortunately, so this is provided instead.
                ///
                /// Also note that `take_handle` is in theory only ever called on values
                /// owned by a generated function. For example a generated function might
                /// take `Resource<T>` as an argument but then call `take_handle` on a
                /// reference to that argument. In that sense the dynamic nature of
                /// `take_handle` should only be exposed internally to generated code, not
                /// to user code.
                #[doc(hidden)]
                pub fn take_handle(resource: &Resource<T>) -> u32 {
                    resource.handle.swap(u32::MAX, Relaxed)
                }
                #[doc(hidden)]
                pub fn handle(resource: &Resource<T>) -> u32 {
                    resource.handle.load(Relaxed)
                }
            }
            impl<T: WasmResource> fmt::Debug for Resource<T> {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.debug_struct("Resource").field("handle", &self.handle).finish()
                }
            }
            impl<T: WasmResource> Drop for Resource<T> {
                fn drop(&mut self) {
                    unsafe {
                        match self.handle.load(Relaxed) {
                            u32::MAX => {}
                            other => T::drop(other),
                        }
                    }
                }
            }
            pub unsafe fn bool_lift(val: u8) -> bool {
                if true {
                    match val {
                        0 => false,
                        1 => true,
                        _ => {
                            ::core::panicking::panic_fmt(
                                format_args!("invalid bool discriminant"),
                            );
                        }
                    }
                } else {
                    val != 0
                }
            }
            pub use alloc_crate::alloc;
            pub unsafe fn string_lift(bytes: Vec<u8>) -> String {
                if true {
                    String::from_utf8(bytes).unwrap()
                } else {
                    String::from_utf8_unchecked(bytes)
                }
            }
            pub unsafe fn invalid_enum_discriminant<T>() -> T {
                if true {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!("invalid enum discriminant"),
                        );
                    }
                } else {
                    core::hint::unreachable_unchecked()
                }
            }
            pub unsafe fn cabi_dealloc(ptr: *mut u8, size: usize, align: usize) {
                if size == 0 {
                    return;
                }
                let layout = alloc::Layout::from_size_align_unchecked(size, align);
                alloc::dealloc(ptr, layout);
            }
            extern crate alloc as alloc_crate;
        }
        #[doc(inline)]
        pub use __export_layer_trigger_world_impl as export;
        #[inline(never)]
        #[doc(hidden)]
        pub fn __link_custom_section_describing_imports() {
            wit_bindgen::rt::maybe_link_cabi_realloc();
        }
        const _: &[u8] = b"package lay3r:avs@0.3.0;\n\nuse wasi:io/poll@0.2.2;\nuse wasi:clocks/monotonic-clock@0.2.0;\nuse wasi:io/error@0.2.0;\nuse wasi:io/streams@0.2.0;\nuse wasi:http/types@0.2.0 as http-types;\nuse wasi:http/outgoing-handler@0.2.0 as http-outgoing-handler;\n\ninterface layer-types {\n  record cosmos-address {\n    bech32-addr: string,\n    // prefix is the first part of the bech32 address\n    prefix-len: u32\n  } \n\n  record cosmos-event {\n    ty: string,\n    attributes: list<tuple<string, string>>,\n  }\n\n  record cosmos-chain-config {\n    chain-id: string,\n    rpc-endpoint: option<string>,\n    grpc-endpoint: option<string>,\n    grpc-web-endpoint: option<string>,\n    gas-price: f32,\n    gas-denom: string,\n    bech32-prefix: string,\n  }\n\n  record eth-address {\n    raw-bytes: list<u8>\n  }\n\n  record eth-event-log-data {\n    // the raw log topics that can be decoded into an event\n    topics: list<list<u8>>,\n    // the raw log data that can be decoded into an event\n    data: list<u8>,\n  }\n\n  record eth-chain-config {\n    chain-id: string,\n    ws-endpoint: option<string>,\n    http-endpoint: option<string>,\n  }\n\n  record trigger-action {\n    config: trigger-config,\n    data: trigger-data\n  }\n\n  record trigger-config {\n    service-id: string,\n    workflow-id: string,\n    trigger-source: trigger-source\n  }\n\n  variant trigger-source {\n    eth-contract-event(trigger-source-eth-contract-event),\n    cosmos-contract-event(trigger-source-cosmos-contract-event),\n    manual\n  }\n\n  record trigger-source-eth-contract-event {\n    address: eth-address,\n    chain-name: string,\n    event-hash: list<u8>\n  }\n\n  record trigger-source-cosmos-contract-event {\n    address: cosmos-address,\n    chain-name: string,\n    event-type: string\n  }\n\n  variant trigger-data {\n    eth-contract-event(trigger-data-eth-contract-event),\n    cosmos-contract-event(trigger-data-cosmos-contract-event),\n    raw(list<u8>)\n  }\n\n  record trigger-data-eth-contract-event {\n    contract-address: eth-address,\n    chain-name: string,\n    log: eth-event-log-data,\n    block-height: u64\n  }\n\n  record trigger-data-cosmos-contract-event {\n    contract-address: cosmos-address,\n    chain-name: string,\n    event: cosmos-event,\n    block-height: u64\n  }\n}\n\nworld layer-trigger-world {\n  import wasi:io/poll@0.2.2;\n  import host: interface {\n    use layer-types.{eth-chain-config, cosmos-chain-config};\n\n    get-eth-chain-config: func(chain-name: string) -> option<eth-chain-config>;\n    get-cosmos-chain-config: func(chain-name: string) -> option<cosmos-chain-config>;\n  }\n\n  use layer-types.{trigger-action};\n\n  export run: func(trigger-action: trigger-action) -> result<list<u8>, string>;\n}";
        const _: &[u8] = b"package wasi:io@0.2.2;\n\n@since(version = 0.2.0)\ninterface error {\n  /// A resource which represents some error information.\n  ///\n  /// The only method provided by this resource is `to-debug-string`,\n  /// which provides some human-readable information about the error.\n  ///\n  /// In the `wasi:io` package, this resource is returned through the\n  /// `wasi:io/streams/stream-error` type.\n  ///\n  /// To provide more specific error information, other interfaces may\n  /// offer functions to \"downcast\" this error into more specific types. For example,\n  /// errors returned from streams derived from filesystem types can be described using\n  /// the filesystem\'s own error-code type. This is done using the function\n  /// `wasi:filesystem/types/filesystem-error-code`, which takes a `borrow<error>`\n  /// parameter and returns an `option<wasi:filesystem/types/error-code>`.\n  ///\n  /// The set of functions which can \"downcast\" an `error` into a more\n  /// concrete type is open.\n  @since(version = 0.2.0)\n  resource error {\n    /// Returns a string that is suitable to assist humans in debugging\n    /// this error.\n    ///\n    /// WARNING: The returned string should not be consumed mechanically!\n    /// It may change across platforms, hosts, or other implementation\n    /// details. Parsing this string is a major platform-compatibility\n    /// hazard.\n    @since(version = 0.2.0)\n    to-debug-string: func() -> string;\n  }\n}\n\n/// A poll API intended to let users wait for I/O events on multiple handles\n/// at once.\n@since(version = 0.2.0)\ninterface poll {\n  /// `pollable` represents a single I/O event which may be ready, or not.\n  @since(version = 0.2.0)\n  resource pollable {\n    /// Return the readiness of a pollable. This function never blocks.\n    ///\n    /// Returns `true` when the pollable is ready, and `false` otherwise.\n    @since(version = 0.2.0)\n    ready: func() -> bool;\n    /// `block` returns immediately if the pollable is ready, and otherwise\n    /// blocks until ready.\n    ///\n    /// This function is equivalent to calling `poll.poll` on a list\n    /// containing only this pollable.\n    @since(version = 0.2.0)\n    block: func();\n  }\n\n  /// Poll for completion on a set of pollables.\n  ///\n  /// This function takes a list of pollables, which identify I/O sources of\n  /// interest, and waits until one or more of the events is ready for I/O.\n  ///\n  /// The result `list<u32>` contains one or more indices of handles in the\n  /// argument list that is ready for I/O.\n  ///\n  /// This function traps if either:\n  /// - the list is empty, or:\n  /// - the list contains more elements than can be indexed with a `u32` value.\n  ///\n  /// A timeout can be implemented by adding a pollable from the\n  /// wasi-clocks API to the list.\n  ///\n  /// This function does not return a `result`; polling in itself does not\n  /// do any I/O so it doesn\'t fail. If any of the I/O sources identified by\n  /// the pollables has an error, it is indicated by marking the source as\n  /// being ready for I/O.\n  @since(version = 0.2.0)\n  poll: func(in: list<borrow<pollable>>) -> list<u32>;\n}\n\n/// WASI I/O is an I/O abstraction API which is currently focused on providing\n/// stream types.\n///\n/// In the future, the component model is expected to add built-in stream types;\n/// when it does, they are expected to subsume this API.\n@since(version = 0.2.0)\ninterface streams {\n  @since(version = 0.2.0)\n  use error.{error};\n  @since(version = 0.2.0)\n  use poll.{pollable};\n\n  /// An error for input-stream and output-stream operations.\n  @since(version = 0.2.0)\n  variant stream-error {\n    /// The last operation (a write or flush) failed before completion.\n    ///\n    /// More information is available in the `error` payload.\n    ///\n    /// After this, the stream will be closed. All future operations return\n    /// `stream-error::closed`.\n    last-operation-failed(error),\n    /// The stream is closed: no more input will be accepted by the\n    /// stream. A closed output-stream will return this error on all\n    /// future operations.\n    closed,\n  }\n\n  /// An input bytestream.\n  ///\n  /// `input-stream`s are *non-blocking* to the extent practical on underlying\n  /// platforms. I/O operations always return promptly; if fewer bytes are\n  /// promptly available than requested, they return the number of bytes promptly\n  /// available, which could even be zero. To wait for data to be available,\n  /// use the `subscribe` function to obtain a `pollable` which can be polled\n  /// for using `wasi:io/poll`.\n  @since(version = 0.2.0)\n  resource input-stream {\n    /// Perform a non-blocking read from the stream.\n    ///\n    /// When the source of a `read` is binary data, the bytes from the source\n    /// are returned verbatim. When the source of a `read` is known to the\n    /// implementation to be text, bytes containing the UTF-8 encoding of the\n    /// text are returned.\n    ///\n    /// This function returns a list of bytes containing the read data,\n    /// when successful. The returned list will contain up to `len` bytes;\n    /// it may return fewer than requested, but not more. The list is\n    /// empty when no bytes are available for reading at this time. The\n    /// pollable given by `subscribe` will be ready when more bytes are\n    /// available.\n    ///\n    /// This function fails with a `stream-error` when the operation\n    /// encounters an error, giving `last-operation-failed`, or when the\n    /// stream is closed, giving `closed`.\n    ///\n    /// When the caller gives a `len` of 0, it represents a request to\n    /// read 0 bytes. If the stream is still open, this call should\n    /// succeed and return an empty list, or otherwise fail with `closed`.\n    ///\n    /// The `len` parameter is a `u64`, which could represent a list of u8 which\n    /// is not possible to allocate in wasm32, or not desirable to allocate as\n    /// as a return value by the callee. The callee may return a list of bytes\n    /// less than `len` in size while more bytes are available for reading.\n    @since(version = 0.2.0)\n    read: func(len: u64) -> result<list<u8>, stream-error>;\n    /// Read bytes from a stream, after blocking until at least one byte can\n    /// be read. Except for blocking, behavior is identical to `read`.\n    @since(version = 0.2.0)\n    blocking-read: func(len: u64) -> result<list<u8>, stream-error>;\n    /// Skip bytes from a stream. Returns number of bytes skipped.\n    ///\n    /// Behaves identical to `read`, except instead of returning a list\n    /// of bytes, returns the number of bytes consumed from the stream.\n    @since(version = 0.2.0)\n    skip: func(len: u64) -> result<u64, stream-error>;\n    /// Skip bytes from a stream, after blocking until at least one byte\n    /// can be skipped. Except for blocking behavior, identical to `skip`.\n    @since(version = 0.2.0)\n    blocking-skip: func(len: u64) -> result<u64, stream-error>;\n    /// Create a `pollable` which will resolve once either the specified stream\n    /// has bytes available to read or the other end of the stream has been\n    /// closed.\n    /// The created `pollable` is a child resource of the `input-stream`.\n    /// Implementations may trap if the `input-stream` is dropped before\n    /// all derived `pollable`s created with this function are dropped.\n    @since(version = 0.2.0)\n    subscribe: func() -> pollable;\n  }\n\n  /// An output bytestream.\n  ///\n  /// `output-stream`s are *non-blocking* to the extent practical on\n  /// underlying platforms. Except where specified otherwise, I/O operations also\n  /// always return promptly, after the number of bytes that can be written\n  /// promptly, which could even be zero. To wait for the stream to be ready to\n  /// accept data, the `subscribe` function to obtain a `pollable` which can be\n  /// polled for using `wasi:io/poll`.\n  ///\n  /// Dropping an `output-stream` while there\'s still an active write in\n  /// progress may result in the data being lost. Before dropping the stream,\n  /// be sure to fully flush your writes.\n  @since(version = 0.2.0)\n  resource output-stream {\n    /// Check readiness for writing. This function never blocks.\n    ///\n    /// Returns the number of bytes permitted for the next call to `write`,\n    /// or an error. Calling `write` with more bytes than this function has\n    /// permitted will trap.\n    ///\n    /// When this function returns 0 bytes, the `subscribe` pollable will\n    /// become ready when this function will report at least 1 byte, or an\n    /// error.\n    @since(version = 0.2.0)\n    check-write: func() -> result<u64, stream-error>;\n    /// Perform a write. This function never blocks.\n    ///\n    /// When the destination of a `write` is binary data, the bytes from\n    /// `contents` are written verbatim. When the destination of a `write` is\n    /// known to the implementation to be text, the bytes of `contents` are\n    /// transcoded from UTF-8 into the encoding of the destination and then\n    /// written.\n    ///\n    /// Precondition: check-write gave permit of Ok(n) and contents has a\n    /// length of less than or equal to n. Otherwise, this function will trap.\n    ///\n    /// returns Err(closed) without writing if the stream has closed since\n    /// the last call to check-write provided a permit.\n    @since(version = 0.2.0)\n    write: func(contents: list<u8>) -> result<_, stream-error>;\n    /// Perform a write of up to 4096 bytes, and then flush the stream. Block\n    /// until all of these operations are complete, or an error occurs.\n    ///\n    /// This is a convenience wrapper around the use of `check-write`,\n    /// `subscribe`, `write`, and `flush`, and is implemented with the\n    /// following pseudo-code:\n    ///\n    /// ```text\n    /// let pollable = this.subscribe();\n    /// while !contents.is_empty() {\n      /// // Wait for the stream to become writable\n      /// pollable.block();\n      /// let Ok(n) = this.check-write(); // eliding error handling\n      /// let len = min(n, contents.len());\n      /// let (chunk, rest) = contents.split_at(len);\n      /// this.write(chunk  );            // eliding error handling\n      /// contents = rest;\n      /// }\n    /// this.flush();\n    /// // Wait for completion of `flush`\n    /// pollable.block();\n    /// // Check for any errors that arose during `flush`\n    /// let _ = this.check-write();         // eliding error handling\n    /// ```\n    @since(version = 0.2.0)\n    blocking-write-and-flush: func(contents: list<u8>) -> result<_, stream-error>;\n    /// Request to flush buffered output. This function never blocks.\n    ///\n    /// This tells the output-stream that the caller intends any buffered\n    /// output to be flushed. the output which is expected to be flushed\n    /// is all that has been passed to `write` prior to this call.\n    ///\n    /// Upon calling this function, the `output-stream` will not accept any\n    /// writes (`check-write` will return `ok(0)`) until the flush has\n    /// completed. The `subscribe` pollable will become ready when the\n    /// flush has completed and the stream can accept more writes.\n    @since(version = 0.2.0)\n    flush: func() -> result<_, stream-error>;\n    /// Request to flush buffered output, and block until flush completes\n    /// and stream is ready for writing again.\n    @since(version = 0.2.0)\n    blocking-flush: func() -> result<_, stream-error>;\n    /// Create a `pollable` which will resolve once the output-stream\n    /// is ready for more writing, or an error has occurred. When this\n    /// pollable is ready, `check-write` will return `ok(n)` with n>0, or an\n    /// error.\n    ///\n    /// If the stream is closed, this pollable is always ready immediately.\n    ///\n    /// The created `pollable` is a child resource of the `output-stream`.\n    /// Implementations may trap if the `output-stream` is dropped before\n    /// all derived `pollable`s created with this function are dropped.\n    @since(version = 0.2.0)\n    subscribe: func() -> pollable;\n    /// Write zeroes to a stream.\n    ///\n    /// This should be used precisely like `write` with the exact same\n    /// preconditions (must use check-write first), but instead of\n    /// passing a list of bytes, you simply pass the number of zero-bytes\n    /// that should be written.\n    @since(version = 0.2.0)\n    write-zeroes: func(len: u64) -> result<_, stream-error>;\n    /// Perform a write of up to 4096 zeroes, and then flush the stream.\n    /// Block until all of these operations are complete, or an error\n    /// occurs.\n    ///\n    /// This is a convenience wrapper around the use of `check-write`,\n    /// `subscribe`, `write-zeroes`, and `flush`, and is implemented with\n    /// the following pseudo-code:\n    ///\n    /// ```text\n    /// let pollable = this.subscribe();\n    /// while num_zeroes != 0 {\n      /// // Wait for the stream to become writable\n      /// pollable.block();\n      /// let Ok(n) = this.check-write(); // eliding error handling\n      /// let len = min(n, num_zeroes);\n      /// this.write-zeroes(len);         // eliding error handling\n      /// num_zeroes -= len;\n      /// }\n    /// this.flush();\n    /// // Wait for completion of `flush`\n    /// pollable.block();\n    /// // Check for any errors that arose during `flush`\n    /// let _ = this.check-write();         // eliding error handling\n    /// ```\n    @since(version = 0.2.0)\n    blocking-write-zeroes-and-flush: func(len: u64) -> result<_, stream-error>;\n    /// Read from one stream and write to another.\n    ///\n    /// The behavior of splice is equivalent to:\n    /// 1. calling `check-write` on the `output-stream`\n    /// 2. calling `read` on the `input-stream` with the smaller of the\n    /// `check-write` permitted length and the `len` provided to `splice`\n    /// 3. calling `write` on the `output-stream` with that read data.\n    ///\n    /// Any error reported by the call to `check-write`, `read`, or\n    /// `write` ends the splice and reports that error.\n    ///\n    /// This function returns the number of bytes transferred; it may be less\n    /// than `len`.\n    @since(version = 0.2.0)\n    splice: func(src: borrow<input-stream>, len: u64) -> result<u64, stream-error>;\n    /// Read from one stream and write to another, with blocking.\n    ///\n    /// This is similar to `splice`, except that it blocks until the\n    /// `output-stream` is ready for writing, and the `input-stream`\n    /// is ready for reading, before performing the `splice`.\n    @since(version = 0.2.0)\n    blocking-splice: func(src: borrow<input-stream>, len: u64) -> result<u64, stream-error>;\n  }\n}\n\n@since(version = 0.2.0)\nworld imports {\n  @since(version = 0.2.0)\n  import error;\n  @since(version = 0.2.0)\n  import poll;\n  @since(version = 0.2.0)\n  import streams;\n}\n";
        const _: &[u8] = b"package wasi:cli@0.2.0;\n\ninterface stdout {\n  use wasi:io/streams@0.2.0.{output-stream};\n\n  get-stdout: func() -> output-stream;\n}\n\ninterface stderr {\n  use wasi:io/streams@0.2.0.{output-stream};\n\n  get-stderr: func() -> output-stream;\n}\n\ninterface stdin {\n  use wasi:io/streams@0.2.0.{input-stream};\n\n  get-stdin: func() -> input-stream;\n}\n\n";
        const _: &[u8] = b"package wasi:http@0.2.0;\n\n/// This interface defines all of the types and methods for implementing\n/// HTTP Requests and Responses, both incoming and outgoing, as well as\n/// their headers, trailers, and bodies.\ninterface types {\n  use wasi:clocks/monotonic-clock@0.2.0.{duration};\n  use wasi:io/streams@0.2.0.{input-stream, output-stream};\n  use wasi:io/error@0.2.0.{error as io-error};\n  use wasi:io/poll@0.2.0.{pollable};\n\n  /// This type corresponds to HTTP standard Methods.\n  variant method {\n    get,\n    head,\n    post,\n    put,\n    delete,\n    connect,\n    options,\n    trace,\n    patch,\n    other(string),\n  }\n\n  /// This type corresponds to HTTP standard Related Schemes.\n  variant scheme {\n    HTTP,\n    HTTPS,\n    other(string),\n  }\n\n  /// Defines the case payload type for `DNS-error` above:\n  record DNS-error-payload {\n    rcode: option<string>,\n    info-code: option<u16>,\n  }\n\n  /// Defines the case payload type for `TLS-alert-received` above:\n  record TLS-alert-received-payload {\n    alert-id: option<u8>,\n    alert-message: option<string>,\n  }\n\n  /// Defines the case payload type for `HTTP-response-{header,trailer}-size` above:\n  record field-size-payload {\n    field-name: option<string>,\n    field-size: option<u32>,\n  }\n\n  /// These cases are inspired by the IANA HTTP Proxy Error Types:\n  /// https://www.iana.org/assignments/http-proxy-status/http-proxy-status.xhtml#table-http-proxy-error-types\n  variant error-code {\n    DNS-timeout,\n    DNS-error(DNS-error-payload),\n    destination-not-found,\n    destination-unavailable,\n    destination-IP-prohibited,\n    destination-IP-unroutable,\n    connection-refused,\n    connection-terminated,\n    connection-timeout,\n    connection-read-timeout,\n    connection-write-timeout,\n    connection-limit-reached,\n    TLS-protocol-error,\n    TLS-certificate-error,\n    TLS-alert-received(TLS-alert-received-payload),\n    HTTP-request-denied,\n    HTTP-request-length-required,\n    HTTP-request-body-size(option<u64>),\n    HTTP-request-method-invalid,\n    HTTP-request-URI-invalid,\n    HTTP-request-URI-too-long,\n    HTTP-request-header-section-size(option<u32>),\n    HTTP-request-header-size(option<field-size-payload>),\n    HTTP-request-trailer-section-size(option<u32>),\n    HTTP-request-trailer-size(field-size-payload),\n    HTTP-response-incomplete,\n    HTTP-response-header-section-size(option<u32>),\n    HTTP-response-header-size(field-size-payload),\n    HTTP-response-body-size(option<u64>),\n    HTTP-response-trailer-section-size(option<u32>),\n    HTTP-response-trailer-size(field-size-payload),\n    HTTP-response-transfer-coding(option<string>),\n    HTTP-response-content-coding(option<string>),\n    HTTP-response-timeout,\n    HTTP-upgrade-failed,\n    HTTP-protocol-error,\n    loop-detected,\n    configuration-error,\n    /// This is a catch-all error for anything that doesn\'t fit cleanly into a\n    /// more specific case. It also includes an optional string for an\n    /// unstructured description of the error. Users should not depend on the\n    /// string for diagnosing errors, as it\'s not required to be consistent\n    /// between implementations.\n    internal-error(option<string>),\n  }\n\n  /// This type enumerates the different kinds of errors that may occur when\n  /// setting or appending to a `fields` resource.\n  variant header-error {\n    /// This error indicates that a `field-key` or `field-value` was\n    /// syntactically invalid when used with an operation that sets headers in a\n    /// `fields`.\n    invalid-syntax,\n    /// This error indicates that a forbidden `field-key` was used when trying\n    /// to set a header in a `fields`.\n    forbidden,\n    /// This error indicates that the operation on the `fields` was not\n    /// permitted because the fields are immutable.\n    immutable,\n  }\n\n  /// Field keys are always strings.\n  type field-key = string;\n\n  /// Field values should always be ASCII strings. However, in\n  /// reality, HTTP implementations often have to interpret malformed values,\n  /// so they are provided as a list of bytes.\n  type field-value = list<u8>;\n\n  /// This following block defines the `fields` resource which corresponds to\n  /// HTTP standard Fields. Fields are a common representation used for both\n  /// Headers and Trailers.\n  ///\n  /// A `fields` may be mutable or immutable. A `fields` created using the\n  /// constructor, `from-list`, or `clone` will be mutable, but a `fields`\n  /// resource given by other means (including, but not limited to,\n  /// `incoming-request.headers`, `outgoing-request.headers`) might be be\n  /// immutable. In an immutable fields, the `set`, `append`, and `delete`\n  /// operations will fail with `header-error.immutable`.\n  resource fields {\n    /// Construct an empty HTTP Fields.\n    ///\n    /// The resulting `fields` is mutable.\n    constructor();\n    /// Construct an HTTP Fields.\n    ///\n    /// The resulting `fields` is mutable.\n    ///\n    /// The list represents each key-value pair in the Fields. Keys\n    /// which have multiple values are represented by multiple entries in this\n    /// list with the same key.\n    ///\n    /// The tuple is a pair of the field key, represented as a string, and\n    /// Value, represented as a list of bytes. In a valid Fields, all keys\n    /// and values are valid UTF-8 strings. However, values are not always\n    /// well-formed, so they are represented as a raw list of bytes.\n    ///\n    /// An error result will be returned if any header or value was\n    /// syntactically invalid, or if a header was forbidden.\n    from-list: static func(entries: list<tuple<field-key, field-value>>) -> result<fields, header-error>;\n    /// Get all of the values corresponding to a key. If the key is not present\n    /// in this `fields`, an empty list is returned. However, if the key is\n    /// present but empty, this is represented by a list with one or more\n    /// empty field-values present.\n    get: func(name: field-key) -> list<field-value>;\n    /// Returns `true` when the key is present in this `fields`. If the key is\n    /// syntactically invalid, `false` is returned.\n    has: func(name: field-key) -> bool;\n    /// Set all of the values for a key. Clears any existing values for that\n    /// key, if they have been set.\n    ///\n    /// Fails with `header-error.immutable` if the `fields` are immutable.\n    set: func(name: field-key, value: list<field-value>) -> result<_, header-error>;\n    /// Delete all values for a key. Does nothing if no values for the key\n    /// exist.\n    ///\n    /// Fails with `header-error.immutable` if the `fields` are immutable.\n    delete: func(name: field-key) -> result<_, header-error>;\n    /// Append a value for a key. Does not change or delete any existing\n    /// values for that key.\n    ///\n    /// Fails with `header-error.immutable` if the `fields` are immutable.\n    append: func(name: field-key, value: field-value) -> result<_, header-error>;\n    /// Retrieve the full set of keys and values in the Fields. Like the\n    /// constructor, the list represents each key-value pair.\n    ///\n    /// The outer list represents each key-value pair in the Fields. Keys\n    /// which have multiple values are represented by multiple entries in this\n    /// list with the same key.\n    entries: func() -> list<tuple<field-key, field-value>>;\n    /// Make a deep copy of the Fields. Equivelant in behavior to calling the\n    /// `fields` constructor on the return value of `entries`. The resulting\n    /// `fields` is mutable.\n    clone: func() -> fields;\n  }\n\n  /// Headers is an alias for Fields.\n  type headers = fields;\n\n  /// Trailers is an alias for Fields.\n  type trailers = fields;\n\n  /// Represents an incoming HTTP Request.\n  resource incoming-request {\n    /// Returns the method of the incoming request.\n    method: func() -> method;\n    /// Returns the path with query parameters from the request, as a string.\n    path-with-query: func() -> option<string>;\n    /// Returns the protocol scheme from the request.\n    scheme: func() -> option<scheme>;\n    /// Returns the authority from the request, if it was present.\n    authority: func() -> option<string>;\n    /// Get the `headers` associated with the request.\n    ///\n    /// The returned `headers` resource is immutable: `set`, `append`, and\n    /// `delete` operations will fail with `header-error.immutable`.\n    ///\n    /// The `headers` returned are a child resource: it must be dropped before\n    /// the parent `incoming-request` is dropped. Dropping this\n    /// `incoming-request` before all children are dropped will trap.\n    headers: func() -> headers;\n    /// Gives the `incoming-body` associated with this request. Will only\n    /// return success at most once, and subsequent calls will return error.\n    consume: func() -> result<incoming-body>;\n  }\n\n  /// Represents an outgoing HTTP Request.\n  resource outgoing-request {\n    /// Construct a new `outgoing-request` with a default `method` of `GET`, and\n    /// `none` values for `path-with-query`, `scheme`, and `authority`.\n    ///\n    /// * `headers` is the HTTP Headers for the Request.\n    ///\n    /// It is possible to construct, or manipulate with the accessor functions\n    /// below, an `outgoing-request` with an invalid combination of `scheme`\n    /// and `authority`, or `headers` which are not permitted to be sent.\n    /// It is the obligation of the `outgoing-handler.handle` implementation\n    /// to reject invalid constructions of `outgoing-request`.\n    constructor(headers: headers);\n    /// Returns the resource corresponding to the outgoing Body for this\n    /// Request.\n    ///\n    /// Returns success on the first call: the `outgoing-body` resource for\n    /// this `outgoing-request` can be retrieved at most once. Subsequent\n    /// calls will return error.\n    body: func() -> result<outgoing-body>;\n    /// Get the Method for the Request.\n    method: func() -> method;\n    /// Set the Method for the Request. Fails if the string present in a\n    /// `method.other` argument is not a syntactically valid method.\n    set-method: func(method: method) -> result;\n    /// Get the combination of the HTTP Path and Query for the Request.\n    /// When `none`, this represents an empty Path and empty Query.\n    path-with-query: func() -> option<string>;\n    /// Set the combination of the HTTP Path and Query for the Request.\n    /// When `none`, this represents an empty Path and empty Query. Fails is the\n    /// string given is not a syntactically valid path and query uri component.\n    set-path-with-query: func(path-with-query: option<string>) -> result;\n    /// Get the HTTP Related Scheme for the Request. When `none`, the\n    /// implementation may choose an appropriate default scheme.\n    scheme: func() -> option<scheme>;\n    /// Set the HTTP Related Scheme for the Request. When `none`, the\n    /// implementation may choose an appropriate default scheme. Fails if the\n    /// string given is not a syntactically valid uri scheme.\n    set-scheme: func(scheme: option<scheme>) -> result;\n    /// Get the HTTP Authority for the Request. A value of `none` may be used\n    /// with Related Schemes which do not require an Authority. The HTTP and\n    /// HTTPS schemes always require an authority.\n    authority: func() -> option<string>;\n    /// Set the HTTP Authority for the Request. A value of `none` may be used\n    /// with Related Schemes which do not require an Authority. The HTTP and\n    /// HTTPS schemes always require an authority. Fails if the string given is\n    /// not a syntactically valid uri authority.\n    set-authority: func(authority: option<string>) -> result;\n    /// Get the headers associated with the Request.\n    ///\n    /// The returned `headers` resource is immutable: `set`, `append`, and\n    /// `delete` operations will fail with `header-error.immutable`.\n    ///\n    /// This headers resource is a child: it must be dropped before the parent\n    /// `outgoing-request` is dropped, or its ownership is transfered to\n    /// another component by e.g. `outgoing-handler.handle`.\n    headers: func() -> headers;\n  }\n\n  /// Parameters for making an HTTP Request. Each of these parameters is\n  /// currently an optional timeout applicable to the transport layer of the\n  /// HTTP protocol.\n  ///\n  /// These timeouts are separate from any the user may use to bound a\n  /// blocking call to `wasi:io/poll.poll`.\n  resource request-options {\n    /// Construct a default `request-options` value.\n    constructor();\n    /// The timeout for the initial connect to the HTTP Server.\n    connect-timeout: func() -> option<duration>;\n    /// Set the timeout for the initial connect to the HTTP Server. An error\n    /// return value indicates that this timeout is not supported.\n    set-connect-timeout: func(duration: option<duration>) -> result;\n    /// The timeout for receiving the first byte of the Response body.\n    first-byte-timeout: func() -> option<duration>;\n    /// Set the timeout for receiving the first byte of the Response body. An\n    /// error return value indicates that this timeout is not supported.\n    set-first-byte-timeout: func(duration: option<duration>) -> result;\n    /// The timeout for receiving subsequent chunks of bytes in the Response\n    /// body stream.\n    between-bytes-timeout: func() -> option<duration>;\n    /// Set the timeout for receiving subsequent chunks of bytes in the Response\n    /// body stream. An error return value indicates that this timeout is not\n    /// supported.\n    set-between-bytes-timeout: func(duration: option<duration>) -> result;\n  }\n\n  /// Represents the ability to send an HTTP Response.\n  ///\n  /// This resource is used by the `wasi:http/incoming-handler` interface to\n  /// allow a Response to be sent corresponding to the Request provided as the\n  /// other argument to `incoming-handler.handle`.\n  resource response-outparam {\n    /// Set the value of the `response-outparam` to either send a response,\n    /// or indicate an error.\n    ///\n    /// This method consumes the `response-outparam` to ensure that it is\n    /// called at most once. If it is never called, the implementation\n    /// will respond with an error.\n    ///\n    /// The user may provide an `error` to `response` to allow the\n    /// implementation determine how to respond with an HTTP error response.\n    set: static func(param: response-outparam, response: result<outgoing-response, error-code>);\n  }\n\n  /// This type corresponds to the HTTP standard Status Code.\n  type status-code = u16;\n\n  /// Represents an incoming HTTP Response.\n  resource incoming-response {\n    /// Returns the status code from the incoming response.\n    status: func() -> status-code;\n    /// Returns the headers from the incoming response.\n    ///\n    /// The returned `headers` resource is immutable: `set`, `append`, and\n    /// `delete` operations will fail with `header-error.immutable`.\n    ///\n    /// This headers resource is a child: it must be dropped before the parent\n    /// `incoming-response` is dropped.\n    headers: func() -> headers;\n    /// Returns the incoming body. May be called at most once. Returns error\n    /// if called additional times.\n    consume: func() -> result<incoming-body>;\n  }\n\n  /// Represents an incoming HTTP Request or Response\'s Body.\n  ///\n  /// A body has both its contents - a stream of bytes - and a (possibly\n  /// empty) set of trailers, indicating that the full contents of the\n  /// body have been received. This resource represents the contents as\n  /// an `input-stream` and the delivery of trailers as a `future-trailers`,\n  /// and ensures that the user of this interface may only be consuming either\n  /// the body contents or waiting on trailers at any given time.\n  resource incoming-body {\n    /// Returns the contents of the body, as a stream of bytes.\n    ///\n    /// Returns success on first call: the stream representing the contents\n    /// can be retrieved at most once. Subsequent calls will return error.\n    ///\n    /// The returned `input-stream` resource is a child: it must be dropped\n    /// before the parent `incoming-body` is dropped, or consumed by\n    /// `incoming-body.finish`.\n    ///\n    /// This invariant ensures that the implementation can determine whether\n    /// the user is consuming the contents of the body, waiting on the\n    /// `future-trailers` to be ready, or neither. This allows for network\n    /// backpressure is to be applied when the user is consuming the body,\n    /// and for that backpressure to not inhibit delivery of the trailers if\n    /// the user does not read the entire body.\n    %stream: func() -> result<input-stream>;\n    /// Takes ownership of `incoming-body`, and returns a `future-trailers`.\n    /// This function will trap if the `input-stream` child is still alive.\n    finish: static func(this: incoming-body) -> future-trailers;\n  }\n\n  /// Represents a future which may eventaully return trailers, or an error.\n  ///\n  /// In the case that the incoming HTTP Request or Response did not have any\n  /// trailers, this future will resolve to the empty set of trailers once the\n  /// complete Request or Response body has been received.\n  resource future-trailers {\n    /// Returns a pollable which becomes ready when either the trailers have\n    /// been received, or an error has occured. When this pollable is ready,\n    /// the `get` method will return `some`.\n    subscribe: func() -> pollable;\n    /// Returns the contents of the trailers, or an error which occured,\n    /// once the future is ready.\n    ///\n    /// The outer `option` represents future readiness. Users can wait on this\n    /// `option` to become `some` using the `subscribe` method.\n    ///\n    /// The outer `result` is used to retrieve the trailers or error at most\n    /// once. It will be success on the first call in which the outer option\n    /// is `some`, and error on subsequent calls.\n    ///\n    /// The inner `result` represents that either the HTTP Request or Response\n    /// body, as well as any trailers, were received successfully, or that an\n    /// error occured receiving them. The optional `trailers` indicates whether\n    /// or not trailers were present in the body.\n    ///\n    /// When some `trailers` are returned by this method, the `trailers`\n    /// resource is immutable, and a child. Use of the `set`, `append`, or\n    /// `delete` methods will return an error, and the resource must be\n    /// dropped before the parent `future-trailers` is dropped.\n    get: func() -> option<result<result<option<trailers>, error-code>>>;\n  }\n\n  /// Represents an outgoing HTTP Response.\n  resource outgoing-response {\n    /// Construct an `outgoing-response`, with a default `status-code` of `200`.\n    /// If a different `status-code` is needed, it must be set via the\n    /// `set-status-code` method.\n    ///\n    /// * `headers` is the HTTP Headers for the Response.\n    constructor(headers: headers);\n    /// Get the HTTP Status Code for the Response.\n    status-code: func() -> status-code;\n    /// Set the HTTP Status Code for the Response. Fails if the status-code\n    /// given is not a valid http status code.\n    set-status-code: func(status-code: status-code) -> result;\n    /// Get the headers associated with the Request.\n    ///\n    /// The returned `headers` resource is immutable: `set`, `append`, and\n    /// `delete` operations will fail with `header-error.immutable`.\n    ///\n    /// This headers resource is a child: it must be dropped before the parent\n    /// `outgoing-request` is dropped, or its ownership is transfered to\n    /// another component by e.g. `outgoing-handler.handle`.\n    headers: func() -> headers;\n    /// Returns the resource corresponding to the outgoing Body for this Response.\n    ///\n    /// Returns success on the first call: the `outgoing-body` resource for\n    /// this `outgoing-response` can be retrieved at most once. Subsequent\n    /// calls will return error.\n    body: func() -> result<outgoing-body>;\n  }\n\n  /// Represents an outgoing HTTP Request or Response\'s Body.\n  ///\n  /// A body has both its contents - a stream of bytes - and a (possibly\n  /// empty) set of trailers, inducating the full contents of the body\n  /// have been sent. This resource represents the contents as an\n  /// `output-stream` child resource, and the completion of the body (with\n  /// optional trailers) with a static function that consumes the\n  /// `outgoing-body` resource, and ensures that the user of this interface\n  /// may not write to the body contents after the body has been finished.\n  ///\n  /// If the user code drops this resource, as opposed to calling the static\n  /// method `finish`, the implementation should treat the body as incomplete,\n  /// and that an error has occured. The implementation should propogate this\n  /// error to the HTTP protocol by whatever means it has available,\n  /// including: corrupting the body on the wire, aborting the associated\n  /// Request, or sending a late status code for the Response.\n  resource outgoing-body {\n    /// Returns a stream for writing the body contents.\n    ///\n    /// The returned `output-stream` is a child resource: it must be dropped\n    /// before the parent `outgoing-body` resource is dropped (or finished),\n    /// otherwise the `outgoing-body` drop or `finish` will trap.\n    ///\n    /// Returns success on the first call: the `output-stream` resource for\n    /// this `outgoing-body` may be retrieved at most once. Subsequent calls\n    /// will return error.\n    write: func() -> result<output-stream>;\n    /// Finalize an outgoing body, optionally providing trailers. This must be\n    /// called to signal that the response is complete. If the `outgoing-body`\n    /// is dropped without calling `outgoing-body.finalize`, the implementation\n    /// should treat the body as corrupted.\n    ///\n    /// Fails if the body\'s `outgoing-request` or `outgoing-response` was\n    /// constructed with a Content-Length header, and the contents written\n    /// to the body (via `write`) does not match the value given in the\n    /// Content-Length.\n    finish: static func(this: outgoing-body, trailers: option<trailers>) -> result<_, error-code>;\n  }\n\n  /// Represents a future which may eventaully return an incoming HTTP\n  /// Response, or an error.\n  ///\n  /// This resource is returned by the `wasi:http/outgoing-handler` interface to\n  /// provide the HTTP Response corresponding to the sent Request.\n  resource future-incoming-response {\n    /// Returns a pollable which becomes ready when either the Response has\n    /// been received, or an error has occured. When this pollable is ready,\n    /// the `get` method will return `some`.\n    subscribe: func() -> pollable;\n    /// Returns the incoming HTTP Response, or an error, once one is ready.\n    ///\n    /// The outer `option` represents future readiness. Users can wait on this\n    /// `option` to become `some` using the `subscribe` method.\n    ///\n    /// The outer `result` is used to retrieve the response or error at most\n    /// once. It will be success on the first call in which the outer option\n    /// is `some`, and error on subsequent calls.\n    ///\n    /// The inner `result` represents that either the incoming HTTP Response\n    /// status and headers have recieved successfully, or that an error\n    /// occured. Errors may also occur while consuming the response body,\n    /// but those will be reported by the `incoming-body` and its\n    /// `output-stream` child.\n    get: func() -> option<result<result<incoming-response, error-code>>>;\n  }\n\n  /// Attempts to extract a http-related `error` from the wasi:io `error`\n  /// provided.\n  ///\n  /// Stream operations which return\n  /// `wasi:io/stream/stream-error::last-operation-failed` have a payload of\n  /// type `wasi:io/error/error` with more information about the operation\n  /// that failed. This payload can be passed through to this function to see\n  /// if there\'s http-related information about the error to return.\n  ///\n  /// Note that this function is fallible because not all io-errors are\n  /// http-related errors.\n  http-error-code: func(err: borrow<io-error>) -> option<error-code>;\n}\n\n/// This interface defines a handler of incoming HTTP Requests. It should\n/// be exported by components which can respond to HTTP Requests.\ninterface incoming-handler {\n  use types.{incoming-request, response-outparam};\n\n  /// This function is invoked with an incoming HTTP Request, and a resource\n  /// `response-outparam` which provides the capability to reply with an HTTP\n  /// Response. The response is sent by calling the `response-outparam.set`\n  /// method, which allows execution to continue after the response has been\n  /// sent. This enables both streaming to the response body, and performing other\n  /// work.\n  ///\n  /// The implementor of this function must write a response to the\n  /// `response-outparam` before returning, or else the caller will respond\n  /// with an error on its behalf.\n  handle: func(request: incoming-request, response-out: response-outparam);\n}\n\n/// This interface defines a handler of outgoing HTTP Requests. It should be\n/// imported by components which wish to make HTTP Requests.\ninterface outgoing-handler {\n  use types.{outgoing-request, request-options, future-incoming-response, error-code};\n\n  /// This function is invoked with an outgoing HTTP Request, and it returns\n  /// a resource `future-incoming-response` which represents an HTTP Response\n  /// which may arrive in the future.\n  ///\n  /// The `options` argument accepts optional parameters for the HTTP\n  /// protocol\'s transport layer.\n  ///\n  /// This function may return an error if the `outgoing-request` is invalid\n  /// or not allowed to be made. Otherwise, protocol errors are reported\n  /// through the `future-incoming-response`.\n  handle: func(request: outgoing-request, options: option<request-options>) -> result<future-incoming-response, error-code>;\n}\n\n/// The `wasi:http/proxy` world captures a widely-implementable intersection of\n/// hosts that includes HTTP forward and reverse proxies. Components targeting\n/// this world may concurrently stream in and out any number of incoming and\n/// outgoing HTTP requests.\nworld proxy {\n  import wasi:random/random@0.2.0;\n  import wasi:io/error@0.2.0;\n  import wasi:io/poll@0.2.0;\n  import wasi:io/streams@0.2.0;\n  import wasi:cli/stdout@0.2.0;\n  import wasi:cli/stderr@0.2.0;\n  import wasi:cli/stdin@0.2.0;\n  import wasi:clocks/monotonic-clock@0.2.0;\n  import types;\n  import outgoing-handler;\n  import wasi:clocks/wall-clock@0.2.0;\n\n  export incoming-handler;\n}\n";
        const _: &[u8] = b"package wasi:io@0.2.0;\n\ninterface poll {\n  resource pollable {\n    ready: func() -> bool;\n    block: func();\n  }\n\n  poll: func(in: list<borrow<pollable>>) -> list<u32>;\n}\n\ninterface error {\n  resource error {\n    to-debug-string: func() -> string;\n  }\n}\n\ninterface streams {\n  use error.{error};\n  use poll.{pollable};\n\n  variant stream-error {\n    last-operation-failed(error),\n    closed,\n  }\n\n  resource input-stream {\n    read: func(len: u64) -> result<list<u8>, stream-error>;\n    blocking-read: func(len: u64) -> result<list<u8>, stream-error>;\n    skip: func(len: u64) -> result<u64, stream-error>;\n    blocking-skip: func(len: u64) -> result<u64, stream-error>;\n    subscribe: func() -> pollable;\n  }\n\n  resource output-stream {\n    check-write: func() -> result<u64, stream-error>;\n    write: func(contents: list<u8>) -> result<_, stream-error>;\n    blocking-write-and-flush: func(contents: list<u8>) -> result<_, stream-error>;\n    flush: func() -> result<_, stream-error>;\n    blocking-flush: func() -> result<_, stream-error>;\n    subscribe: func() -> pollable;\n    write-zeroes: func(len: u64) -> result<_, stream-error>;\n    blocking-write-zeroes-and-flush: func(len: u64) -> result<_, stream-error>;\n    splice: func(src: borrow<input-stream>, len: u64) -> result<u64, stream-error>;\n    blocking-splice: func(src: borrow<input-stream>, len: u64) -> result<u64, stream-error>;\n  }\n}\n\n";
        const _: &[u8] = b"package wasi:random@0.2.0;\n\ninterface random {\n  get-random-bytes: func(len: u64) -> list<u8>;\n\n  get-random-u64: func() -> u64;\n}\n\n";
        const _: &[u8] = b"package wasi:clocks@0.2.0;\n\n/// WASI Monotonic Clock is a clock API intended to let users measure elapsed\n/// time.\n///\n/// It is intended to be portable at least between Unix-family platforms and\n/// Windows.\n///\n/// A monotonic clock is a clock which has an unspecified initial value, and\n/// successive reads of the clock will produce non-decreasing values.\n///\n/// It is intended for measuring elapsed time.\ninterface monotonic-clock {\n  use wasi:io/poll@0.2.0.{pollable};\n\n  /// An instant in time, in nanoseconds. An instant is relative to an\n  /// unspecified initial value, and can only be compared to instances from\n  /// the same monotonic-clock.\n  type instant = u64;\n\n  /// A duration of time, in nanoseconds.\n  type duration = u64;\n\n  /// Read the current value of the clock.\n  ///\n  /// The clock is monotonic, therefore calling this function repeatedly will\n  /// produce a sequence of non-decreasing values.\n  now: func() -> instant;\n\n  /// Query the resolution of the clock. Returns the duration of time\n  /// corresponding to a clock tick.\n  resolution: func() -> duration;\n\n  /// Create a `pollable` which will resolve once the specified instant\n  /// occured.\n  subscribe-instant: func(when: instant) -> pollable;\n\n  /// Create a `pollable` which will resolve once the given duration has\n  /// elapsed, starting at the time at which this function was called.\n  /// occured.\n  subscribe-duration: func(when: duration) -> pollable;\n}\n\n/// WASI Wall Clock is a clock API intended to let users query the current\n/// time. The name \"wall\" makes an analogy to a \"clock on the wall\", which\n/// is not necessarily monotonic as it may be reset.\n///\n/// It is intended to be portable at least between Unix-family platforms and\n/// Windows.\n///\n/// A wall clock is a clock which measures the date and time according to\n/// some external reference.\n///\n/// External references may be reset, so this clock is not necessarily\n/// monotonic, making it unsuitable for measuring elapsed time.\n///\n/// It is intended for reporting the current date and time for humans.\ninterface wall-clock {\n  /// A time and date in seconds plus nanoseconds.\n  record datetime {\n    seconds: u64,\n    nanoseconds: u32,\n  }\n\n  /// Read the current value of the clock.\n  ///\n  /// This clock is not monotonic, therefore calling this function repeatedly\n  /// will not necessarily produce a sequence of non-decreasing values.\n  ///\n  /// The returned timestamps represent the number of seconds since\n  /// 1970-01-01T00:00:00Z, also known as [POSIX\'s Seconds Since the Epoch],\n  /// also known as [Unix Time].\n  ///\n  /// The nanoseconds field of the output is always less than 1000000000.\n  ///\n  /// [POSIX\'s Seconds Since the Epoch]: https://pubs.opengroup.org/onlinepubs/9699919799/xrat/V4_xbd_chap04.html#tag_21_04_16\n  /// [Unix Time]: https://en.wikipedia.org/wiki/Unix_time\n  now: func() -> datetime;\n\n  /// Query the resolution of the clock.\n  ///\n  /// The nanoseconds field of the output is always less than 1000000000.\n  resolution: func() -> datetime;\n}\n\nworld imports {\n  import wasi:io/poll@0.2.0;\n  import monotonic-clock;\n  import wall-clock;\n}\n";
    }
}
pub mod cosmos {
    mod query {
        #![allow(unused_imports)]
        #![allow(dead_code)]
        use anyhow::{anyhow, Result};
        use async_trait::async_trait;
        use layer_climb::prelude::*;
        use serde::{de::DeserializeOwned, Serialize};
        use std::sync::Arc;
        use wasi::http::types::Method;
        use wstd::runtime::Reactor;
        use crate::{bindings::compat::CosmosChainConfig, wasi::{Request, WasiPollable}};
        struct WasiCosmosRpcTransport {
            reactor: Reactor,
        }
        unsafe impl Sync for WasiCosmosRpcTransport {}
        unsafe impl Send for WasiCosmosRpcTransport {}
        pub async fn new_cosmos_query_client(
            chain_config: CosmosChainConfig,
            _reactor: Reactor,
        ) -> Result<QueryClient> {
            let chain_config: layer_climb::prelude::ChainConfig = chain_config.into();
            QueryClient::new(
                    chain_config.clone(),
                    Some(Connection {
                        preferred_mode: Some(ConnectionMode::Rpc),
                        ..Default::default()
                    }),
                )
                .await
        }
    }
    pub use query::*;
}
pub mod ethereum {
    mod event {
        use crate::bindings::compat::EthEventLogData;
        use alloy_primitives::FixedBytes;
        use anyhow::{anyhow, Result};
        pub fn decode_event_log_data<T: alloy_sol_types::SolEvent>(
            log_data: EthEventLogData,
        ) -> Result<T> {
            let topics = log_data
                .topics
                .iter()
                .map(|t| FixedBytes::<32>::from_slice(t))
                .collect();
            let log_data = alloy_primitives::LogData::new(topics, log_data.data.into())
                .ok_or_else(|| ::anyhow::__private::must_use({
                    let error = ::anyhow::__private::format_err(
                        format_args!("failed to create log data"),
                    );
                    error
                }))?;
            T::decode_log_data(&log_data, false)
                .map_err(|e| ::anyhow::Error::msg(
                    ::alloc::__export::must_use({
                        let res = ::alloc::fmt::format(
                            format_args!("failed to decode event: {0}", e),
                        );
                        res
                    }),
                ))
        }
    }
    mod provider {
        #![allow(unused_imports)]
        #![allow(dead_code)]
        use std::{
            future::Future, pin::{pin, Pin},
            sync::Arc, task,
        };
        use alloy_json_rpc::{RequestPacket, ResponsePacket};
        use alloy_provider::{network::Ethereum, Network, Provider, RootProvider};
        use alloy_rpc_client::RpcClient;
        use alloy_transport::{
            utils::guess_local_url, BoxTransport, Pbf, TransportConnect, TransportError,
            TransportErrorKind, TransportFut,
        };
        use alloy_transport_http::{Http, HttpConnect};
        use tower_service::Service;
        use wasi::http::types::Method;
        use wit_bindgen_rt::async_support::futures::pin_mut;
        use wstd::runtime::Reactor;
        use crate::wasi::{Request, Response, WasiPollable};
        pub fn new_eth_provider<N: Network>(
            _reactor: Reactor,
            _endpoint: String,
        ) -> RootProvider<BoxTransport, N> {
            ::core::panicking::panic("not implemented")
        }
    }
    pub use event::*;
    pub use provider::*;
}
pub mod wasi {
    #![allow(async_fn_in_trait)]
    use serde::{de::DeserializeOwned, Serialize};
    use std::cmp::min;
    pub use url::Url;
    pub use wasi::http::types::Method;
    pub use wstd::runtime::{block_on, Reactor};
    /// The error type.
    pub type Error = String;
    /// The result type.
    pub type Result<T> = std::result::Result<T, Error>;
    /// An HTTP request.
    pub struct Request {
        pub method: Method,
        pub url: Url,
        pub headers: Vec<(String, String)>,
        pub body: Vec<u8>,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Request {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field4_finish(
                f,
                "Request",
                "method",
                &self.method,
                "url",
                &self.url,
                "headers",
                &self.headers,
                "body",
                &&self.body,
            )
        }
    }
    impl Request {
        /// Construct request.
        pub fn new(method: Method, url: &str) -> Result<Self> {
            Ok(Self {
                method,
                url: Url::parse(url).map_err(|e| e.to_string())?,
                headers: ::alloc::vec::Vec::new(),
                body: ::alloc::vec::Vec::new(),
            })
        }
        /// Construct GET request.
        pub fn get(url: &str) -> Result<Self> {
            Request::new(Method::Get, url)
        }
        /// Construct POST request.
        pub fn post(url: &str) -> Result<Self> {
            Request::new(Method::Post, url)
        }
        /// Construct PUT request.
        pub fn put(url: &str) -> Result<Self> {
            Request::new(Method::Put, url)
        }
        /// Construct PATCH request.
        pub fn patch(url: &str) -> Result<Self> {
            Request::new(Method::Patch, url)
        }
        /// Construct DELETE request.
        pub fn delete(url: &str) -> Result<Self> {
            Request::new(Method::Delete, url)
        }
        /// Set JSON body.
        pub fn json<T: Serialize + ?Sized>(&mut self, json: &T) -> Result<&mut Self> {
            self.body = serde_json::to_vec(json).map_err(|e| e.to_string())?;
            if !self.headers.iter().any(|(k, _)| &k.to_lowercase() == "content-type") {
                self.headers
                    .push(("content-type".to_string(), "application/json".to_string()));
            }
            Ok(self)
        }
    }
    /// An HTTP response.
    pub struct Response {
        pub status: u16,
        pub headers: Vec<(String, String)>,
        pub body: Vec<u8>,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Response {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field3_finish(
                f,
                "Response",
                "status",
                &self.status,
                "headers",
                &self.headers,
                "body",
                &&self.body,
            )
        }
    }
    impl Response {
        /// Get JSON body.
        pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
            serde_json::from_slice(&self.body).map_err(|e| e.to_string())
        }
    }
    /// Trait adding WASI methods to the `wstd::runtime::Reactor`.
    pub trait WasiPollable {
        async fn read_all(
            &self,
            stream: wasi::io::streams::InputStream,
            size: Option<usize>,
        ) -> Result<Vec<u8>>;
        async fn write_all(
            &self,
            stream: wasi::io::streams::OutputStream,
            bytes: &[u8],
        ) -> Result<()>;
        async fn send(&self, req: Request) -> Result<Response>;
    }
    impl WasiPollable for wstd::runtime::Reactor {
        /// Read `wasi:io` `input-stream` into memory.
        async fn read_all(
            &self,
            stream: wasi::io::streams::InputStream,
            size: Option<usize>,
        ) -> Result<Vec<u8>> {
            let mut buf = if let Some(size) = size {
                Vec::with_capacity(size)
            } else {
                Vec::new()
            };
            loop {
                self.wait_for(stream.subscribe()).await;
                let mut bytes = match stream.read(4096) {
                    Ok(bytes) => bytes,
                    Err(wasi::io::streams::StreamError::Closed) => {
                        return Ok(buf);
                    }
                    Err(wasi::io::streams::StreamError::LastOperationFailed(err)) => {
                        return Err(
                            ::alloc::__export::must_use({
                                let res = ::alloc::fmt::format(
                                    format_args!(
                                        "failed to read from stream: {0}",
                                        err.to_debug_string(),
                                    ),
                                );
                                res
                            }),
                        );
                    }
                };
                buf.append(&mut bytes);
            }
        }
        /// Write `wasi:io` `output-stream` from memory.
        async fn write_all(
            &self,
            stream: wasi::io::streams::OutputStream,
            mut bytes: &[u8],
        ) -> Result<()> {
            let err = "failed to write to stream";
            while !bytes.is_empty() {
                self.wait_for(stream.subscribe()).await;
                let n = stream.check_write().map_err(|_| err)? as usize;
                let stop = min(n, bytes.len());
                stream.write(&bytes[..stop]).map_err(|_| err)?;
                stream.flush().map_err(|_| err)?;
                if stop == bytes.len() {
                    self.wait_for(stream.subscribe()).await;
                    break;
                } else {
                    bytes = &bytes[stop..];
                }
            }
            Ok(())
        }
        /// Send the HTTP request.
        async fn send(&self, req: Request) -> Result<Response> {
            let wasi_headers = wasi::http::types::Fields::from_list(
                    &req
                        .headers
                        .into_iter()
                        .map(|(k, v)| (k, v.into_bytes()))
                        .collect::<Vec<(String, Vec<u8>)>>(),
                )
                .or(Err("invalid header".to_string()))?;
            let wasi_req = wasi::http::types::OutgoingRequest::new(wasi_headers);
            wasi_req.set_method(&req.method).or(Err("invalid method".to_string()))?;
            use wasi::http::types::Scheme;
            let scheme = match req.url.scheme() {
                "http" => Scheme::Http,
                "https" => Scheme::Https,
                other => Scheme::Other(other.to_owned()),
            };
            wasi_req
                .set_scheme(Some(&scheme))
                .or(Err("invalid url scheme".to_string()))?;
            let path = match req.url.query() {
                Some(query) => {
                    ::alloc::__export::must_use({
                        let res = ::alloc::fmt::format(
                            format_args!("{0}?{1}", req.url.path(), query),
                        );
                        res
                    })
                }
                None => req.url.path().to_owned(),
            };
            wasi_req
                .set_path_with_query(Some(&path))
                .or(Err("invalid url path".to_string()))?;
            wasi_req
                .set_authority(Some(req.url.authority()))
                .or(Err("invalid url authority".to_string()))?;
            let wasi_body = wasi_req.body().unwrap();
            let body_stream = wasi_body.write().unwrap();
            let res = wasi::http::outgoing_handler::handle(wasi_req, None)
                .or(Err("failed to send request".to_string()))?;
            self.write_all(body_stream, &req.body).await?;
            wasi::http::types::OutgoingBody::finish(wasi_body, None).unwrap();
            self.wait_for(res.subscribe()).await;
            let res = res
                .get()
                .unwrap()
                .unwrap()
                .map_err(|err| ::alloc::__export::must_use({
                    let res = ::alloc::fmt::format(
                        format_args!("response error: {0}", err),
                    );
                    res
                }))?;
            let res_status = res.status();
            let mut content_length = None;
            let res_headers = res
                .headers()
                .entries()
                .into_iter()
                .map(|(k, v)| {
                    if k.to_lowercase() == "content-length" {
                        content_length = std::str::from_utf8(&v)
                            .ok()
                            .and_then(|s| s.parse::<usize>().ok());
                    }
                    let v = std::string::String::from_utf8(v)
                        .or(
                            Err(
                                ::alloc::__export::must_use({
                                    let res = ::alloc::fmt::format(
                                        format_args!("invalid response header value for `{0}`", k),
                                    );
                                    res
                                }),
                            ),
                        )?;
                    Ok((k, v))
                })
                .collect::<Result<Vec<(String, String)>>>()?;
            let res_body = res.consume().unwrap();
            let res_body_stream = res_body.stream().unwrap();
            Ok(Response {
                status: res_status,
                headers: res_headers,
                body: self.read_all(res_body_stream, content_length).await?,
            })
        }
    }
}
