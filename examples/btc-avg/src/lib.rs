#[allow(warnings)]
mod bindings;

mod coin_gecko;
use bindings::{Error, Guest, Input, Output};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

const PRICE_HISTORY_FILE_PATH: &str = "price_history.json";

struct Component;

impl Guest for Component {
    fn run_task(_request: Input) -> Result<Output, Error> {
        let api_key = std::env::var("API_KEY").or(Err("missing env var `API_KEY`".to_string()))?;
        let price = coin_gecko::get_btc_usd_price(&api_key)
            .map_err(|err| err.to_string())?
            .ok_or("invalid response from coin gecko API")?;

        // read previous price history
        let mut history = match std::fs::read(PRICE_HISTORY_FILE_PATH) {
            Ok(bytes) => {
                serde_json::from_slice::<PriceHistory>(&bytes).map_err(|err| err.to_string())?
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Default::default(),
            Err(err) => return Err(err.to_string()),
        };

        // get current time in secs
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("failed to get current time")
            .as_secs();

        // add latest price to front of the list and truncate to max of 1000
        history.btcusd_prices.push_front((now, price));
        history.btcusd_prices.truncate(1000);

        // write price history
        std::fs::write(
            PRICE_HISTORY_FILE_PATH,
            serde_json::to_vec(&history).map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;

        // calculate average prices
        let avg_last_minute = history.average(now - 60);
        let avg_last_hour = history.average(now - 3600);

        // serialize JSON response
        let response = serde_json::to_string(&Response {
            btcusd: Price {
                price,
                avg_last_minute,
                avg_last_hour,
            },
        })
        .map_err(|err| err.to_string())?;

        Ok(Output { response })
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Response {
    pub btcusd: Price,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Price {
    pub price: f32,
    pub avg_last_minute: AveragePrice,
    pub avg_last_hour: AveragePrice,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AveragePrice {
    pub price: f32,
    pub count: usize,
}

#[derive(Deserialize, Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct PriceHistory {
    pub btcusd_prices: VecDeque<(u64, f32)>,
}

impl PriceHistory {
    fn average(&self, since_time_secs: u64) -> AveragePrice {
        let mut sum = 0f64;
        let mut count = 0;
        for (t, p) in self.btcusd_prices.iter() {
            if t >= &since_time_secs {
                sum += *p as f64;
                count += 1;
            } else {
                break;
            }
        }
        AveragePrice {
            price: (sum / (count as f64)) as f32,
            count,
        }
    }
}

bindings::export!(Component with_types_in bindings);
