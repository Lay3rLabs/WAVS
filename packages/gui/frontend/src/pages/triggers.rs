use wavs_types::{Trigger, TriggerAction};

use crate::prelude::*;

pub struct Triggers {
    state: AppState,
}

impl Triggers {
    pub fn new(state: AppState) -> Arc<Self> {
        Arc::new(Self { state })
    }

    pub fn render(self: &Arc<Self>) -> Dom {
        let state = &self.state;
        static SCROLL_CONTAINER: LazyLock<String> = LazyLock::new(|| {
            class! {
                .style("display", "flex")
                .style("flex-direction", "column")
                .style("gap", "0.75rem")
                .style("max-height", "calc(100vh - 12rem)")
                .style("overflow-y", "auto")
                .style("padding-right", "0.5rem")
            }
        });

        html!("div", {
            .class(&*SCROLL_CONTAINER)
            .children_signal_vec(state.triggers_list.signal_vec_cloned().map(clone!(state => move |item| {
                render_item(&state, item)
            })))
        })
    }
}

fn render_item(state: &AppState, item: TriggerAction) -> Dom {
    let service_label = state.service_label(&item.config.service_id);

    let trigger_label = match &item.config.trigger {
        Trigger::CosmosContractEvent { .. } => "Cosmos Contract Event",
        Trigger::EvmContractEvent { .. } => "EVM Contract Event",
        Trigger::BlockInterval { .. } => "Block Interval",
        Trigger::Cron { .. } => "Cron",
        Trigger::AtProtoEvent { .. } => "AtProto Event",
        Trigger::Manual => "Manual",
    };

    let label = format!(
        "[{}/{}]: {}",
        service_label, item.config.workflow_id, trigger_label
    );

    let content = html!("pre", {
        .text(&serde_json::to_string_pretty(&item).unwrap_or_else(|_| "Failed to serialize trigger".to_string()))
    });

    render_expander(&label, content, false)
}
