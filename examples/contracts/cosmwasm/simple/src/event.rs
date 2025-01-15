use anyhow::{anyhow, Error};
use cosmwasm_std::{Event, Uint64};

#[derive(Debug)]
pub struct NewMessageEvent {
    pub data: Vec<u8>,
    pub id: Uint64,
}

impl NewMessageEvent {
    pub const KEY: &'static str = "new-message";
}

impl From<NewMessageEvent> for Event {
    fn from(src: NewMessageEvent) -> Self {
        Event::new(NewMessageEvent::KEY).add_attributes(vec![
            ("id", src.id.to_string()),
            ("data", hex::encode(src.data)),
        ])
    }
}

impl TryFrom<Event> for NewMessageEvent {
    type Error = Error;

    fn try_from(evt: Event) -> anyhow::Result<Self> {
        if evt.ty.as_str() != format!("wasm-{}", NewMessageEvent::KEY)
            && evt.ty.as_str() != NewMessageEvent::KEY
        {
            return Err(anyhow!(
                "unexpected event type: {}, should be {}",
                evt.ty,
                NewMessageEvent::KEY
            ));
        }

        let mut id = None;
        let mut data = None;

        for attr in evt.attributes.iter() {
            match attr.key.as_str() {
                "id" => id = Some(Uint64::new(attr.value.parse()?)),
                "data" => data = Some(hex::decode(&attr.value)?),
                _ => {}
            }
        }

        match (id, data) {
            (Some(id), Some(data)) => Ok(NewMessageEvent { id, data }),
            _ => Err(anyhow!("missing required attributes")),
        }
    }
}
