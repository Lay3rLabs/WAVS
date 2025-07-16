wit_bindgen::generate!({
    world: "aggregator-world",
    path: "../../../wit-definitions/worker/wit",
    pub_export_macro: true,
    generate_all,
});

#[macro_export]
macro_rules! export_aggregator_world {
    ($Component:ty) => {
        $crate::world::export!(Component with_types_in $crate::world);
    };
}
