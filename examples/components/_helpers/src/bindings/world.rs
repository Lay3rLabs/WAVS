// https://docs.rs/wit-bindgen/0.37.0/wit_bindgen/macro.generate.html

#![allow(clippy::too_many_arguments)]

wit_bindgen::generate!({
    world: "wavs-world",
    path: "../../../wit-definitions/operator/wit",
    pub_export_macro: true,
    generate_all,
    with: {
        "wasi:io/poll@0.2.0": wasip2::io::poll
    },
    features: ["tls"]
});

/// Bindings for the legacy world (components that only export `run`, without the agent interface).
///
/// Used by `export_layer_trigger_world!` so that non-agent components (which do not implement
/// `exports::wavs::operator::agent::Guest`) can still compile even though `wavs-world` now
/// requires both `run` AND `agent` exports.
///
/// Types are reused from the main `wavs-world` bindgen via `with:` to avoid duplication.
#[allow(clippy::all, dead_code)]
pub mod legacy_world {
    wit_bindgen::generate!({
        world: "wavs-legacy-world",
        path: "../../../wit-definitions/operator/wit",
        pub_export_macro: true,
        generate_all,
        with: {
            "wasi:io/poll@0.2.0": wasip2::io::poll,
            // Reuse types from the main wavs-world bindgen to avoid type duplication.
            // This makes legacy_world::Guest use the same TriggerAction/WasmResponse as the main world.
            "wavs:operator/input@2.7.0": super::wavs::operator::input,
            "wavs:operator/output@2.7.0": super::wavs::operator::output,
            // Also remap the transitive type dependencies from wavs:types
            "wavs:types/service@2.7.0": super::wavs::types::service,
            "wavs:types/events@2.7.0": super::wavs::types::events,
            "wavs:types/core@2.7.0": super::wavs::types::core,
            "wavs:types/chain@2.7.0": super::wavs::types::chain,
        },
        features: ["tls"]
    });
}

/// Export macro for legacy (run-only) components.
///
/// Use this in components that only implement `Guest::run` and do NOT implement the agent
/// continuation interface (`GuestAgent::run_agent`). This uses the `wavs-legacy-world` bindings
/// which only require the `run` export.
///
/// A blanket impl bridges `world::Guest` → `legacy_world::Guest` since the types are identical
/// (they're remapped via `with:` in the legacy_world bindgen). This avoids requiring component
/// source files to change when adding the legacy world.
#[macro_export]
macro_rules! export_layer_trigger_world {
    ($Component:ident) => {
        impl $crate::bindings::world::legacy_world::Guest for $Component {
            fn run(
                trigger_action: $crate::bindings::world::wavs::operator::input::TriggerAction,
            ) -> Result<Vec<$crate::bindings::world::wavs::operator::output::WasmResponse>, String>
            {
                <$Component as $crate::bindings::world::Guest>::run(trigger_action)
            }
        }
        $crate::bindings::world::legacy_world::export!($Component with_types_in $crate::bindings::world::legacy_world);
    };
}

/// Export macro for agent (run + run-agent) components.
///
/// Use this in components that implement BOTH `Guest::run` AND `GuestAgent::run_agent`.
/// This uses the full `wavs-world` bindings which require both exports.
#[macro_export]
macro_rules! export_layer_agent_world {
    ($Component:ident) => {
        $crate::bindings::world::export!($Component with_types_in $crate::bindings::world);
    };
}
