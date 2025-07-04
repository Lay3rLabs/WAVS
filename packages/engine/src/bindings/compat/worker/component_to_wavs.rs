use crate::{bindings::world::wavs::worker::output as component_output};

impl From<component_output::WasmResponse> for wavs_types::WasmResponse {
    fn from(src: component_output::WasmResponse) -> Self {
        Self {
            payload: src.payload,
            ordering: src.ordering,
        }
    }
}