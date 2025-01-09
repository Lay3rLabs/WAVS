use anyhow::{anyhow, Error};
use cosmwasm_std::Event;

// Trigger Contracts must emit this event
#[derive(Debug)]
pub struct LayerTriggerEvent {
    // this is any data that the contract wants to send to the component
    // Layer does _not_ parse this data, it's application-specific
    pub data: Vec<u8>,
}

impl LayerTriggerEvent {
    pub const KEY: &'static str = "layer-trigger";

    pub fn new(data: impl AsRef<[u8]>) -> Self {
        LayerTriggerEvent {
            data: data.as_ref().to_vec(),
        }
    }
}

impl From<LayerTriggerEvent> for Event {
    fn from(src: LayerTriggerEvent) -> Self {
        Event::new(LayerTriggerEvent::KEY).add_attributes(vec![("data", hex::encode(src.data))])
    }
}

impl TryFrom<Event> for LayerTriggerEvent {
    type Error = Error;

    fn try_from(evt: Event) -> anyhow::Result<Self> {
        if evt.ty.as_str() != format!("wasm-{}", LayerTriggerEvent::KEY) {
            return Err(anyhow!(
                "unexpected event type: {}, should be {}",
                evt.ty,
                LayerTriggerEvent::KEY
            ));
        }

        let data = evt
            .attributes
            .into_iter()
            .find_map(|attr| {
                if attr.key == "data" {
                    Some(hex::decode(&attr.value))
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow!("missing required attribute"))??;

        Ok(LayerTriggerEvent { data })
    }
}
