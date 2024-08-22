#[allow(warnings)]
mod bindings;

use bindings::Guest;

struct Component;

impl Guest for Component {
    /// Say hello!
    fn handler(input: String) -> String {
        "Hello, World!".to_string()
    }
}

bindings::export!(Component with_types_in bindings);
