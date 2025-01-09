use cosmwasm_schema::cw_serde;
use cosmwasm_std::Uint64;

#[cw_serde]
pub struct ComponentInput {
    // if None, the component will need to fetch the data
    pub message_data: Option<Vec<u8>>,
    pub message_id: Uint64
}

#[cw_serde]
pub struct ComponentOutput {
    pub message_data: Vec<u8>,
    pub message_id: Uint64
}
