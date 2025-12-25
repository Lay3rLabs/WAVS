use crate::prelude::*;

pub struct NotFound {}

impl NotFound {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    pub fn render(self: &Arc<Self>) -> Dom {
        html!("div", {
            .text("Page Not Found")
        })
    }
}
