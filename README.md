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
  "name": "test1",
  "digest": "sha256:ecb1551aa0c4e07082a4fff711f397a68a7ca9ea38b9596ffa0f544310f2704b",
  "trigger": {
    "cron": {
      "schedule": "1/1 * * * * *"
    }
  },
  "permissions": {},
  "wasmUrl": "https://storage.googleapis.com/tmp-bucket-12/wasm/layer/lay3r_gecko%400.1.0.wasm"
}
```

This example will register a new application with name of `test1` that uses a CRON trigger that
executes once every second. The `wasmUrl` is the download URL for the Wasm component. In this case,
the provided Wasm will query for the current `BTCUSD` by making an outbound HTTP request and returning
the result. Currently, it is logged out in debug to the console.


#### Remove an application

`DELETE http://0.0.0.0:8080/app`

with `Content-Type: application/json` request header and a body of the form:

```json
{"apps": ["test1"]}
```

This will deregister the application and remove the application and associated data.
