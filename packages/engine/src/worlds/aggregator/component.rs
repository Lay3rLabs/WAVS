use wavs_types::{ComponentDigest, ServiceID, WorkflowID};

use crate::bindings::aggregator::world::host::LogLevel;

pub type HostComponentLogger = fn(&ServiceID, &WorkflowID, &ComponentDigest, LogLevel, String);
