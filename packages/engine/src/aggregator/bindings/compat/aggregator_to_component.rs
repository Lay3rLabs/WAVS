use crate::bindings::world::{
    aggregator::wavs::types::core as aggregator_core,
    wavs::types::core as component_core,
};

impl From<aggregator_core::LogLevel> for component_core::LogLevel {
    fn from(src: aggregator_core::LogLevel) -> Self {
        match src {
            aggregator_core::LogLevel::Error => component_core::LogLevel::Error,
            aggregator_core::LogLevel::Warn => component_core::LogLevel::Warn,
            aggregator_core::LogLevel::Info => component_core::LogLevel::Info,
            aggregator_core::LogLevel::Debug => component_core::LogLevel::Debug,
            aggregator_core::LogLevel::Trace => component_core::LogLevel::Trace,
        }
    }
}