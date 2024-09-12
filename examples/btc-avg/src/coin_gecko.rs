use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

use crate::bindings::wasi::http;
use crate::bindings::wasi::http::types::{Headers, OutgoingRequest, Scheme};

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

pub fn get_btc_usd_price(api_key: &str) -> Result<Option<f32>> {
    let req = OutgoingRequest::new(Headers::new());
    let _ = req.set_scheme(Some(&Scheme::Https));
    let _ = req.set_authority(Some("api.coingecko.com"));
    let _ = req.set_path_with_query(Some(&format!("/api/v3/exchange_rates?{api_key}")));

    let future_res = http::outgoing_handler::handle(req, None)
        .map_err(|err| anyhow::anyhow!("outgoing error code: {err}"))?;
    let future_res_pollable = future_res.subscribe();
    future_res_pollable.block();

    let res = future_res
        .get()
        .unwrap()
        .map_err(|err| anyhow::anyhow!("outgoing response error code: {err:?}"))?
        .unwrap();

    match res.status() {
        200 => {}
        429 => return Err(anyhow::anyhow!("rate limited, price unavailable")),
        status => return Err(anyhow::anyhow!("unexpected status code: {status}")),
    }

    let body = res.consume().unwrap();
    let stream = body.stream().unwrap();

    let mut buf = Vec::with_capacity(1024 * 10); // 10KB buf; response seems to be ~6KB
    while let Ok(mut bytes) = stream.blocking_read(1024 * 10) {
        buf.append(&mut bytes);
    }

    let coin_gecko: CoinGeckoResponse = serde_json::from_slice(&buf)?;
    Ok(coin_gecko.btc_usd())
}
