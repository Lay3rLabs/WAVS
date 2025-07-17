pub mod host;
pub mod world;

wit_bindgen::generate!({
    world: "aggregator-world",
    path: "../../../wit-definitions/aggregator/wit",
    pub_export_macro: true,
    generate_all,
});

pub use world::*;
