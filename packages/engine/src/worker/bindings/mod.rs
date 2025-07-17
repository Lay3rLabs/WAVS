pub mod host;
pub mod world;

wit_bindgen::generate!({
    world: "wavs-world",
    path: "../../../wit-definitions/worker/wit",
    pub_export_macro: true,
    generate_all,
});

pub use world::*;
