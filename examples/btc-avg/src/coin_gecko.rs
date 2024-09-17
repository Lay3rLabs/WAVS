use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use wstd::{
    http::{Client, StatusCode},
    runtime::Reactor,
};

#[derive(Deserialize, Debug)]
pub struct CoinInfo {
    //pub name: String,
    //pub unit: String,
    pub value: f32,
    //#[serde(rename = "type")]
    //pub _type: String,
}

#[derive(Deserialize, Debug)]
pub struct CoinGeckoResponse {
    pub rates: HashMap<String, CoinInfo>,
}

impl CoinGeckoResponse {
    fn btc_usd(&self) -> Option<f32> {
        self.rates.get("usd").map(|info| info.value)
    }
}

pub async fn get_btc_usd_price(reactor: &Reactor, api_key: &str) -> Result<Option<f32>> {
    let client = Client::new(reactor);
    let mut res = client
        .get(format!(
            "https://api.coingecko.com/api/v3/exchange_rates?{api_key}"
        ))
        .await?;

    match res.status_code() {
        StatusCode::Ok => Ok(serde_json::from_slice::<CoinGeckoResponse>(
            &res.body().read_all().await?,
        )?
        .btc_usd()),
        StatusCode::Other(429) => Err(anyhow::anyhow!("rate limited, price unavailable")),
        status => Err(anyhow::anyhow!("unexpected status code: {status}")),
    }
}
