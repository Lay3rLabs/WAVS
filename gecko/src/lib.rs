use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[allow(warnings)]
mod bindings;

use bindings::wasi::http;
use bindings::wasi::http::types::{Headers, OutgoingRequest, Scheme};
use bindings::Guest;

#[derive(Deserialize, Debug)]
pub struct CoinInfo {
    pub name: String,
    pub unit: String,
    pub value: f64,
    #[serde(rename = "type")]
    pub _type: String,
}

#[derive(Deserialize, Debug)]
pub struct CoinGeckoResponse {
    pub rates: HashMap<String, CoinInfo>,
}

impl CoinGeckoResponse {
    fn btc_price_usd(&self) -> String {
        let btc_usd = self.rates.get("usd").unwrap();
        btc_usd.value.to_string()
    }
}
struct Component;

impl Guest for Component {
    /// Say hello!
    fn handler(input: String) -> String {
        let http_req = OutgoingRequest::new(Headers::new());

        // set the request URL
        let _ = http_req.set_scheme(Some(&Scheme::Https));
        let _ = http_req.set_authority(Some("api.coingecko.com"));
        let _ = http_req.set_path_with_query(Some(
            "/api/v3/exchange_rates?x-cg-demo-api-key=CG-PsTvxDqXZP3RD4TWNxPFamcW",
        ));
        dbg!("HTTP REQ");

        // make the outgoing request
        let future_res = http::outgoing_handler::handle(http_req, None).unwrap();

        // wait for request to complete
        let future_res_pollable = future_res.subscribe();
        future_res_pollable.block();
        // check the response
        let http_res = future_res.get().unwrap().unwrap().unwrap();
        dbg!("GOT");
        // {
        //     Ok(res) => res,
        //     Err(code) => return Err(format!("http response error: {code}")),
        // };

        // response status code
        let status = http_res.status();
        dbg!(&status);
        let body = http_res.consume().unwrap();
        let stream = body.stream().unwrap();
        let mut more_bytes = true;
        let mut full = Vec::new();
        while more_bytes {
            let bytes = stream.blocking_read(10000000000000000000);
            if let Ok(b) = bytes {
                dbg!("MORE BYTES");
                // write_stream.write(&b).unwrap();
                full.extend(b)
            } else {
                more_bytes = false;
            }
        }
        let resp: Result<CoinGeckoResponse, serde_json::Error> = serde_json::from_slice(&full);
        match &resp {
            Ok(r) => return r.btc_price_usd(),
            Err(ref e) => {
                dbg!(&e);
            }
        }
        "Hello, World!".to_string()
    }
}

bindings::export!(Component with_types_in bindings);
