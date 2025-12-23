use crate::prelude::*;

pub struct Triggers {}

impl Triggers {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    pub fn render(self: &Arc<Self>) -> Dom {
        html!("div", {
            .text("Triggers Here")
        })
    }
}
