// https://docs.rs/wit-bindgen/0.37.0/wit_bindgen/macro.generate.html

#![allow(clippy::too_many_arguments)]

wit_bindgen::generate!({
    world: "wavs-world",
    path: "../../../wit-definitions/worker/wit",
    pub_export_macro: true,
    generate_all,
    //async: true,
});

#[macro_export]
macro_rules! export_layer_trigger_world {
    ($Component:ty) => {
        $crate::bindings::world::export!(Component with_types_in $crate::bindings::world);
    };
}
