#![allow(clippy::too_many_arguments)]
pub mod any_contract_event {
    pub mod inner {
        wit_bindgen::generate!({
            world: "layer-any-contract-event-world",
            path: "../../sdk/wit",
            pub_export_macro: true,
            //async: true,
        });
    }

    pub use inner::lay3r::avs::layer_types::*;
    pub use inner::{Guest, Input};

    #[macro_export]
    macro_rules! export_any_contract_event_world {
        ($Component:ty) => {
            $crate::bindings::worlds::any_contract_event::inner::export!(Component with_types_in $crate::bindings::worlds::any_contract_event::inner);
        };
    }
}

pub mod cosmos_contract_event {
    pub mod inner {
        wit_bindgen::generate!({
            world: "layer-cosmos-contract-event-world",
            path: "../../sdk/wit",
            pub_export_macro: true,
            //async: true,
        });
    }

    pub use inner::lay3r::avs::layer_types::*;
    pub use inner::{Guest, Input};

    #[macro_export]
    macro_rules! export_cosmos_contract_event_world {
        ($Component:ty) => {
            $crate::bindings::worlds::cosmos_contract_event::inner::export!(Component with_types_in $crate::bindings::worlds::cosmos_contract_event::inner);
        };
    }
}

pub mod eth_contract_event {
    pub mod inner {
        wit_bindgen::generate!({
            world: "layer-eth-contract-event-world",
            path: "../../sdk/wit",
            pub_export_macro: true,
            //async: true,
        });
    }

    pub use inner::lay3r::avs::layer_types::*;
    pub use inner::{Guest, Input};

    #[macro_export]
    macro_rules! export_eth_contract_event_world {
        ($Component:ty) => {
            $crate::bindings::worlds::eth_contract_event::inner::export!(Component with_types_in $crate::bindings::worlds::eth_contract_event::inner);
        };
    }
}

pub mod raw {
    pub mod inner {
        wit_bindgen::generate!({
            world: "layer-raw-world",
            path: "../../sdk/wit",
            pub_export_macro: true,
            //async: true,
        });
    }

    pub use inner::Guest;

    #[macro_export]
    macro_rules! export_raw_world {
        ($Component:ty) => {
            $crate::bindings::worlds::raw::inner::export!(Component with_types_in $crate::bindings::worlds::raw::inner);
        };
    }
}
