use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use wstd::{
    http::{Client, Method, Request, StatusCode, Url},
    runtime::Reactor,
};

#[derive(Deserialize, Debug)]
pub struct CoinInfo {
    pub value: f32,
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
    let mut req = Request::new(
        Method::Get,
        Url::parse("https://api.coingecko.com/api/v3/exchange_rates")?,
    );
    req.headers_mut().append(
        "x-cg-pro-api-key".to_string(),
        api_key.to_owned().into_bytes(),
    );

    let mut res = Client::new(reactor).send(req).await?;

    match res.status_code() {
        StatusCode::Ok => {
            Ok(serde_json::from_slice::<CoinGeckoResponse>(&res.body().bytes().await?)?.btc_usd())
        }
        StatusCode::Other(429) => Err(anyhow::anyhow!("rate limited, price unavailable")),
        status => Err(anyhow::anyhow!("unexpected status code: {status}")),
    }
}
