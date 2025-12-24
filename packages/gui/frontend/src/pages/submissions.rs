use wavs_gui_shared::event::SubmissionEvent;
use wavs_types::TriggerData;

use crate::prelude::*;

pub struct Submissions {
    state: AppState,
}

impl Submissions {
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
            .children_signal_vec(state.submissions_list.signal_vec_cloned().map(clone!(state => move |item| {
                render_item(&state, item)
            })))
        })
    }
}

fn render_item(state: &AppState, item: SubmissionEvent) -> Dom {
    let service_label = state.service_label(&item.service_id);

    let submission_label = match &item.trigger_data {
        TriggerData::CosmosContractEvent { .. } => "Cosmos Contract Event",
        TriggerData::EvmContractEvent { .. } => "EVM Contract Event",
        TriggerData::BlockInterval { .. } => "Block Interval",
        TriggerData::Cron { .. } => "Cron",
        TriggerData::AtProtoEvent { .. } => "AtProto Event",
        TriggerData::Raw { .. } => "Raw",
    };

    let label = format!(
        "[{}/{}]: {}",
        service_label, item.workflow_id, submission_label
    );

    let content = html!("pre", {
        .text(&serde_json::to_string_pretty(&item).unwrap_or_else(|_| "Failed to serialize submission".to_string()))
    });

    render_expander(&label, content, false)
}
