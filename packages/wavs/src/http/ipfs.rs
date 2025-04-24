use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct PinataResponse {
    pub data: PinataFileData,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PinataFileData {
    pub id: String,
    pub name: String,
    pub cid: String,
    pub created_at: String,
    pub size: u64,
}
