# wasmatic

**PRE-PRODUCTION PROTOTYPE, USE WITH CARE**

### Build and run instructions

Run `cargo run up` to start the Wasmatic server node with the default configuration options.
By default, the operator API will be listen on `http://0.0.0.0:8080`.


### Using the Operator API

#### List active applications

`GET http://0.0.0.0:8080/app`

#### Add an application

`POST http://0.0.0.0:8080/app`

with `Content-Type: application/json` request header and a body of the form:

```json
{
  "name": "test-btc",
  "digest": "sha256:ef6d59d19b678f36c887fd4734fae827b343368de8df08faeb25c4b7fd4d4d00",
  "trigger": {
    "cron": {
      "schedule": "1/15 * * * * *"
    }
  },
  "permissions": {},
  "envs": [
    ["API_KEY", "x-cg-demo-api-key=CG-PsTvxDqXZP3RD4TWNxPFamcW"]
  ],
  "wasmUrl": "https://storage.googleapis.com/tmp-bucket-12/wasm/layer/btc_avg4.wasm"
}
```

This example will register a new application with name of `test1` that uses a CRON trigger that
executes once every 15 seconds. The `wasmUrl` is the download URL for the Wasm component. In this case,
the provided Wasm will query for the current `BTCUSD` by making an outbound HTTP request and returning
the result. Also, computes the average price over past minute and past hour. It uses the app file system
cache for state. Currently, the response is logged out in debug to the console.

#### Remove an application

`DELETE http://0.0.0.0:8080/app`

with `Content-Type: application/json` request header and a body of the form:

```json
{"apps": ["test-btc"]}
```

This will deregister the application and remove the application and associated data.
