use wavs_types::{ComponentDigest, ServiceId, WorkflowId};

use crate::bindings::aggregator::world::host::LogLevel;

pub type HostComponentLogger = fn(&ServiceId, &WorkflowId, &ComponentDigest, LogLevel, String);
