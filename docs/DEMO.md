# Deploy a new component & launch onto a local network

Evan's more detailed instructions based on this demo: <https://aware-class-ea3.notion.site/Reece-s-WAVS-Tutorial-1749b73c338d805b9f1ad18456a07256?pvs=4>

## requirements
- rust installed
- anvil / foundry
- solidity
- just `cargo install just`
- `git clone git@github.com:Lay3rLabs/WAVS.git --branch reece/january-building-your-wavs`

## Create new component

There is probably a better way to do this, for now it's fairly straight forward.

```bash
cp -r examples/eth-trigger-echo examples/eth-trigger-reece-weather
sed -i -e 's/eth-trigger-echo/eth-trigger-reece-weather/g' examples/eth-trigger-reece-weather/Cargo.toml

# update examples/Cargo.toml workspace and add: `eth-trigger-reece-weather`

# open in code editor
code examples/eth-trigger-reece-weather
```

## Update Cargo.toml
```toml

[dependencies]
wit-bindgen-rt = { workspace = true, features = ["bitflags"] }
layer-wasi = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = {workspace = true}
reqwest = {version = "0.12.12"}

```
## Write Component Code

```rust title=lib.rs
#[allow(warnings)]
mod bindings;
use bindings::Guest;
use layer_wasi::{block_on, Reactor, Request, WasiPollable};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn process_eth_trigger(input: Vec<u8>) -> std::result::Result<Vec<u8>, String> {
        println!("process_eth_trigger Received input: {:?}", input); // Nashville,TN as input

        if !input.contains(&b',') {
            return Err("Input must be in the format of City,State".to_string());
        }
        let input = std::str::from_utf8(&input).unwrap(); // TODO:

        // open weather API, not wavs specific -- https://github.com/Lay3rLabs/WAVS/issues/10
        // Or just hardcode it (bad practice / high risk - testing only)
        let api_key = std::env::var("WAVS_ENGINE_ENV_OPEN_WEATHER_API_KEY")
            .or(Err("missing env var `WAVS_ENGINE_ENV_OPEN_WEATHER_API_KEY`".to_string()))?;

        block_on(|reactor| async move {
            let loc: Result<LocDataNested, String> =
                get_location(&reactor, api_key.clone(), input).await;
            let location = match loc {
                Ok(data) => data,
                Err(e) => return Err(e),
            };

            let weather_data = get_weather(&reactor, location, api_key).await;

            match weather_data {
                Ok(data) => {
                    let output: Vec<u8> = data.into();
                    Ok(output)
                }
                Err(e) => Err(e),
            }
        })
    }
}

async fn get_location(
    reactor: &Reactor,
    app_key: String,
    loc_input: &str,
) -> Result<LocDataNested, String> {
    let url: &str = "http://api.openweathermap.org/geo/1.0/direct";
    let loc_input_formatted = format!("{},US", loc_input);
    let params = [
        ("q", loc_input_formatted.as_str()),
        ("appid", app_key.as_str()),
    ];

    let url_with_params = reqwest::Url::parse_with_params(url, &params).unwrap();
    let mut req = Request::get(url_with_params.as_str())?;
    req.headers = vec![
        ("Accept".to_string(), "application/json".to_string()),
        ("Content-Type".to_string(), "application/json".to_string()),
    ];

    let response = reactor.send(req).await;

    match response {
        Ok(response) => {
            println!("{:?}", response);
            let finalresp = response.json::<LocationData>().map_err(|e| {
                let resp_body = response.body;
                let resp_str = String::from_utf8_lossy(&resp_body);
                format!(
                    "Error debugging location response to JSON. Error: {:?}. had response: {:?} | using URL: {:?}",
                    e, resp_str, url_with_params,
                )
            })?;
            println!("{:?}", finalresp);
            return Ok(finalresp[0].clone());
        }
        Err(e) => {
            println!("{:?}", e);
            return Err(e.to_string());
        }
    }
}

async fn get_weather(
    reactor: &Reactor,
    location: LocDataNested,
    app_key: String,
) -> Result<WeatherResponse, String> {
    let url: &str = "https://api.openweathermap.org/data/2.5/weather";
    let params = [
        ("lat", location.lat.to_string()),
        ("lon", location.lon.to_string()),
        ("appid", app_key),
        ("units", "imperial".to_string()),
    ];

    let url_with_params = reqwest::Url::parse_with_params(url, &params).unwrap();
    let mut req = Request::get(url_with_params.as_str())?;
    req.headers = vec![
        ("Accept".to_string(), "application/json".to_string()),
        ("Content-Type".to_string(), "application/json".to_string()),
    ];

    let response = reactor.send(req).await;

    match response {
        // print out either option
        Ok(response) => {
            println!("{:?}", response);
            let finalresp = response.json::<WeatherResponse>().map_err(|e| {
                let resp_body = response.body;
                let resp_str = String::from_utf8_lossy(&resp_body);
                format!(
                    "Error debugging weather response to JSON. Error: {:?}. had response: {:?} | using URL: {:?}",
                    e, resp_str, url_with_params,
                )
            })?;
            println!("{:?}", finalresp.main.temp);
            return Ok(finalresp);
        }
        Err(e) => {
            println!("{:?}", e);
            return Err(e.to_string());
        }
    }
}

/// -----
/// Given the JSON response, use an unescape tool like: https://jsonformatter.org/json-unescape
/// {\"coord\":{\"lon\":-86.7743,\"lat\":36.1623},\"weather\":[{\"id\":804,\"main\":\"Clouds\",\"description\":\"overcast clouds\",\"icon\":\"04d\"}],\"base\":\"stations\",\"main\":{\"temp\":28.13,\"feels_like\":16.21,\"temp_min\":26.17,\"temp_max\":29.34,\"pressure\":1018,\"humidity\":76,\"sea_level\":1018,\"grnd_level\":995},\"visibility\":10000,\"clouds\":{\"all\":100},\"dt\":1736193327,\"sys\":{\"type\":1,\"id\":3477,\"country\":\"US\",\"sunrise\":1736168310,\"sunset\":1736203634},\"timezone\":-21600,\"id\":4644585,\"name\":\"Nashville\",\"cod\":200}
///
/// Then put that into a JSON to struct converter: https://transform.tools/json-to-rust-serde
/// some types will not entirely convert as expected, so you can just ignore them if you get issues or properly convert to the types.
/// -----

// location based
pub type LocationData = Vec<LocDataNested>;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocDataNested {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub country: String,
    pub state: String,
}

//

#[derive(Serialize, Deserialize, Debug)]
pub struct WeatherResponse {
    coord: Coord,
    weather: Vec<Weather>,
    base: String,
    main: Main,
    visibility: i64,
    // wind: Wind, // this needs to be a floating point / string
    clouds: Clouds,
    dt: i64, // the unix time this was taken, in UTC.
    sys: Sys,
    timezone: i64,
    id: i64,
    name: String,
    cod: i64,
}

// convert WeatherResponse to bytes
impl Into<Vec<u8>> for WeatherResponse {
    fn into(self) -> Vec<u8> {
        let s = serde_json::to_string(&self).unwrap();
        s.into_bytes()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Clouds {
    all: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Coord {
    lon: f64,
    lat: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Main {
    temp: f64,
    feels_like: f64,
    temp_min: f64,
    temp_max: f64,
    pressure: i64,
    humidity: i64,
    sea_level: i64,
    grnd_level: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sys {
    #[serde(rename = "type")]
    sys_type: i64,
    id: i64,
    country: String,
    sunrise: i64,
    sunset: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Weather {
    id: i64,
    main: String,
    description: String,
    icon: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Wind {
    speed: i64,
    deg: i64,
}

bindings::export!(Component with_types_in bindings);

```

## Build Component

```bash
just wasi-build eth-trigger-reece-weather
```

## Test Component

```bash
# https://openweathermap.org/
# `WAVS_ENGINE_ENV_` prefixed env vars are passed into runtime at start of WAVS. Temp hack for now

export WAVS_ENGINE_ENV_OPEN_WEATHER_API_KEY="XXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
just cli-exec ./components/eth_trigger_reece_weather.wasm Nashville,TN
```

## Create the Sol contract

```bash
cp contracts/LayerServiceManager.sol contracts/ReeceWeatherServiceManager.sol

# rename contract LayerServiceManager -> contract ReeceWeatherServiceManager
sed -i -e 's/contract LayerServiceManager/contract ReeceWeatherServiceManager/g' contracts/ReeceWeatherServiceManager.sol
sed -i -e 's/LayerServiceManager(/ReeceWeatherServiceManager(/g' contracts/ReeceWeatherServiceManager.sol

# add other logic here, like a link from the input -> output data for easy querying, solidity specific logic
```

## Create the deploy script

```bash
cp contracts/script/LayerServiceManager.s.sol contracts/script/ReeceWeatherServiceManager.s.sol
# replace LayerServiceManager to our new contract
sed -i -e 's/LayerServiceManager/ReeceWeatherServiceManager/g' contracts/script/ReeceWeatherServiceManager.s.sol
```

## Build

```bash
just solidity-build
```

## Start

```bash
# if you previously used wavs, clear old data with `rm -rf ~/wavs/`

# start anvil, wavs, and the wavs aggregator
just start-all

## new tab: deploy Eigenlayer
just cli-deploy-core

# Grab deployment information
CONTRACTS=`cat ~/wavs/cli/deployments.json | jq -r .eigen_core.local`
export FOUNDRY_ANVIL_PRIVATE_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
export CLI_EIGEN_CORE_DELEGATION_MANAGER=`echo $CONTRACTS | jq -r '.delegation_manager'`
export CLI_EIGEN_CORE_REWARDS_COORDINATOR=`echo $CONTRACTS | jq -r '.rewards_coordinator'`
export CLI_EIGEN_CORE_AVS_DIRECTORY=`echo $CONTRACTS | jq -r '.avs_directory'`

# deploy smart contract(s)
# - ECDSAStakeRegistry for message signing verification
# - ServiceManager for task submission
forge script ./contracts/script/ReeceWeatherServiceManager.s.sol --rpc-url http://127.0.0.1:8545 --broadcast

# this has to be uploaded first since the service manager depends on it (not required for input)
# ECDSA_STAKE_REGISTRY_ADDRESS=`jq -r .transactions[0].contractAddress < broadcast/ReeceWeatherServiceManager.s.sol/31337/run-latest.json`
SERVICE_MANAGER_ADDRESS=`jq -r '.transactions[1].contractAddress' < broadcast/ReeceWeatherServiceManager.s.sol/31337/run-latest.json`

# deploy
just cli-deploy-service ./components/eth_trigger_reece_weather.wasm ${SERVICE_MANAGER_ADDRESS}

## Add a task
### NOTE: if using JSON use the format: {\"x\":2} or hex encode {"x":2} -> 7b2278223a327d
just cli-add-task 01943d5734e87a22b7e8598fd15bb443 "Nashville,TN"

## View it in the contract
DATA_RESP=`cast call $SERVICE_MANAGER_ADDRESS "signedDataByTriggerId(uint64)(bytes)" 1`; echo $DATA_RESP
cast --to-ascii $DATA_RESP
```


## Future Improvements:
- A template so all of this is created without the need to copy, paste, replace for a new compoonent, .sol contracts, scripts, etc.
