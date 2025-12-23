use crate::prelude::*;

pub struct Components {}

impl Components {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    pub fn render(self: &Arc<Self>) -> Dom {
        html!("div", {
            .text("Components Here")
        })
    }
}
