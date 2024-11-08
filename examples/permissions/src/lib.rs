#[allow(warnings)]
mod bindings;

use bindings::{Guest, Output, TaskQueueInput};

struct Component;

impl Guest for Component {
    fn run_task(request: TaskQueueInput) -> Output {
        unimplemented!()
    }
}

bindings::export!(Component with_types_in bindings);
