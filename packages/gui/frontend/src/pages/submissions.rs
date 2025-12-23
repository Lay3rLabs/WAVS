use crate::prelude::*;

pub struct Submissions {}

impl Submissions {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    pub fn render(self: &Arc<Self>) -> Dom {
        html!("div", {
            .text("Submissions Here")
        })
    }
}
