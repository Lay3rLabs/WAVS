use crate::bindings::lay3r::avs::layer_types::CosmosEvent;

impl From<CosmosEvent> for cosmwasm_std::Event {
    fn from(event: CosmosEvent) -> Self {
        cosmwasm_std::Event::new(event.ty).add_attributes(event.attributes)
    }
}

impl From<cosmwasm_std::Event> for CosmosEvent {
    fn from(event: cosmwasm_std::Event) -> Self {
        CosmosEvent {
            ty: event.ty,
            attributes: event
                .attributes
                .into_iter()
                .map(|attr| (attr.key, attr.value))
                .collect(),
        }
    }
}
