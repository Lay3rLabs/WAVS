wit_bindgen::generate!({
    world: "aggregator-world",
    path: "../../../wit-definitions/aggregator/wit",
    pub_export_macro: true,
    generate_all,
    with: {
        "wasi:io/poll@0.2.0": wasip2::io::poll
    },
    features: ["tls"]
});

#[macro_export]
macro_rules! export_aggregator_world {
    ($Component:ty) => {
        $crate::world::export!(Component with_types_in $crate::world);
    };
}
