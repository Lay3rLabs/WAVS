# wasmatic

**PRE-PRODUCTION PROTOTYPE, USE WITH CARE**

### Build and run instructions

Run `cargo run up` to start the Wasmatic server node with the default configuration options.
By default, the operator API will be listen on `http://0.0.0.0:8081`.


### Using the Operator API

#### List active applications

`GET http://0.0.0.0:8081/app`

```bash
curl http://0.0.0.0:8081/app | jq .
```

#### Add an application

`POST http://0.0.0.0:8081/app`

with `Content-Type: application/json` request header and a body of the form:

```json
{
  "name": "test-btc",
  "trigger": {
    "cron": {
      "schedule": "1/15 * * * * *"
    }
  },
  "permissions": {},
  "envs": [
    ["API_KEY", "x-cg-demo-api-key=CG-PsTvxDqXZP3RD4TWNxPFamcW"]
  ],
  "url": {
    "url": "https://storage.googleapis.com/tmp-bucket-12/wasm/layer/examples/lay3r_btc-avg%400.6.0.wasm",
    "digest": "sha256:60c28bba3b74e102eb66970fadb80f982d45971e944a615343ece29758172f8e"
  }
}
```

```bash
read -d '' BODY << "EOF"
{
  "name": "test-btc",
  "trigger": {
    "cron": {
      "schedule": "1/15 * * * * *"
    }
  },
  "permissions": {},
  "envs": [
    ["API_KEY", "x-cg-demo-api-key=CG-PsTvxDqXZP3RD4TWNxPFamcW"]
  ],
  "url": {
    "url": "https://storage.googleapis.com/tmp-bucket-12/wasm/layer/examples/lay3r_btc-avg%400.6.0.wasm",
    "digest": "sha256:60c28bba3b74e102eb66970fadb80f982d45971e944a615343ece29758172f8e"
  }
}
EOF

curl -X POST -H "Content-Type: application/json" http://0.0.0.0:8081/app -d "$BODY"
```

This example will register a new application with name of `test1` that uses a CRON trigger that
executes once every 15 seconds. The `wasmUrl` is the download URL for the Wasm component. In this case,
the provided Wasm will query for the current `BTCUSD` by making an outbound HTTP request and returning
the result. Also, computes the average price over past minute and past hour. It uses the app file system
cache for state. Currently, the response is logged out in debug to the console.

As another example, the request body below will register a new application with name `square-queue` that uses a `QUEUE` trigger that polls the lay3r sdk for tasks every 5 seconds.

```json
{
  "name": "square-queue",
  "trigger": {
    "queue": {
      "taskQueueAddr": "slay3r1t9pkt8r25yml6cmhfelx8j9reelthwgml2mqdf53wkvp0wca6systd9gfn",
      "hdIndex": 1,
      "pollInterval": 5
    }
  },
  "permissions": {},
  "envs": [],
  "url": {
    "url": "https://storage.googleapis.com/tmp-bucket-12/wasm/layer/examples/lay3r_square%400.6.0.wasm",
    "digest": "sha256:97ca799dd6e4e0b4088e02cc2de5fc72a6cec636ea1fad98789a0b44bc3fe75a"
  }
}
```

```bash
read -d '' BODY << "EOF"
{
  "name": "square-queue",
  "trigger": {
    "queue": {
      "taskQueueAddr": "slay3r1t9pkt8r25yml6cmhfelx8j9reelthwgml2mqdf53wkvp0wca6systd9gfn",
      "hdIndex": 1,
      "pollInterval": 5
    }
  },
  "permissions": {},
  "envs": [],
  "url": {
    "url": "https://storage.googleapis.com/tmp-bucket-12/wasm/layer/examples/lay3r_square%400.6.0.wasm",
    "digest": "sha256:97ca799dd6e4e0b4088e02cc2de5fc72a6cec636ea1fad98789a0b44bc3fe75a"
  }
}
EOF

curl -X POST -H "Content-Type: application/json" http://0.0.0.0:8081/app -d "$BODY"
```


#### Adding an application published to a registry

`POST http://0.0.0.0:8081/app`

with `Content-Type: application/json` request header and a body of the form:

```json
{
  "name": "test-btc",
  "trigger": {
    "cron": {
      "schedule": "1/15 * * * * *"
    }
  },
  "permissions": {},
  "envs": [
    ["API_KEY", "x-cg-demo-api-key=CG-PsTvxDqXZP3RD4TWNxPFamcW"]
  ],
  "registry": {
    "package": "lay3r:btc-avg",
    "registry": "lay3r.preview.wa.dev"
  }
}
```

```bash
read -d '' BODY << "EOF"
{
  "name": "test-btc",
  "trigger": {
    "cron": {
      "schedule": "1/15 * * * * *"
    }
  },
  "permissions": {},
  "envs": [
    ["API_KEY", "x-cg-demo-api-key=CG-PsTvxDqXZP3RD4TWNxPFamcW"]
  ],
  "registry": {
    "package": "lay3r:btc-avg",
    "registry": "lay3r.preview.wa.dev"
  }
}
EOF

curl -X POST -H "Content-Type: application/json" http://0.0.0.0:8081/app -d "$BODY"
```


#### Upload a wasm endpoint

If the wasm binary that you'd like to register is not behind a url, you can upload it via the `/upload` endpoint before you register, and then omit the `wasmUrl` field in your request body, as exemplified below.

```curl -X POST "localhost:8081/upload" --data-binary "@./path/to/binary.wasm"```
#### Remove an application

`DELETE http://0.0.0.0:8081/app`

with `Content-Type: application/json` request header and a body of the form:

```json
{"apps": ["test-btc"]}
```

This will deregister the application and remove the application and associated data.

```bash
BODY='{"apps": ["test-btc"]}'
curl -X DELETE -H "Content-Type: application/json" http://0.0.0.0:8081/app -d "$BODY"
```
