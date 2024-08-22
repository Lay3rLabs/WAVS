# wasmatic

Wasmatic makes use of [wasmtime](https://github.com/bytecodealliance/wasmtime) for registering and running wasm handlers.

## Getting started
To get your wasmatic node running, you can simply run
```
cargo build
wasmatic up
```

You can change the port that it runs on via the environment variable `WASMATIC_PORT`

The example wasm handlers that I used to test registration of handlers and
cron scheduling just have the following wit.

```wit
world example {
    import wasi:http/outgoing-handler@0.2.0;
    export handler: func(input: string) -> string;
}
```

We can make these more interesting as soon as we want so that they match prior 
avs wit interfaces provided.  I recommend trying things out with `gecko.wasm` 
for now, which periodically checks CoinGecko for btc prices, based on the example spin oracle provided to us a couple weeks back.

ENDPOINTS

POST /register

Expects a query parameter "name" that is the name of the handler being registered as
well as a binary to be included in the request body which is the wasm containing the logic

example:
`curl -X POST "localhost:8080/register?name=foobar" --data-binary "@./path/to/foobar.wasm"`
This request writes foobar.wasm in the "registered" folder on the operator filesystem

POST /sched

Expects a request body with the following fields

```
{
  "name": String,
  "cron": String
}
```

Here `name` is the name of the binary that has been registred via `/register`
and `cron` is a valid cronjob string indicating how often you'd like wasm handler to be executed.

example:
```
curl --request POST \
  --url http://localhost:3000/sched \
  --header 'Content-Type: application/json' \
  --data '{
  "name": "gecko",
  "cron": "1/10 * * * * *"
}'
```

This will run the `handler` function in `./registered/gecko.wasm` once every 10 seconds.

## Using a wasm binary as your wasmatic operator server (only supports /register endpoint)
First you'll have to build your operator component.
It's a component so you'll have to use [cargo component](https://github.com/bytecodealliance/cargo-component).
```
cd operator
cargo component build
```
You should see `proxy.wasm` in your `target` folder.

Wasmatic users running operator nodes shouldn't have to build the operator from source like this, but it probably makes sense for the operator source to live here in wasmatic source for now.  When people run wasmatic operator nodes on their machines, it should probably come with a prebuilt operator wasm binary

Then build `wasmatic`.  It's not a component, so it can be built with `cargo`.

```
cd ../wasmatic
cargo build
```

Finally, at the moment `wasmatic` expects the same `WASI` flags as `wasmtime`.

`wasmatic wasm --dir registered::registered -S common -S http path/to/proxy.wasm`

The `--dir` flag in reference to [preopens](https://wa.dev/wasi:filesystem#preopens).
We're mapping the "registered" folder on the host filesystem to be available to write to using the name "registered", for when we register user-provided handlers.

You can also do some minimal configuration via environment variables.

```
export WASMATIC=path/to/proxy.wasm
```
Setting this variable enables running `wasmatic up` without specifying a path to the operator wasm.

```
export WASMATIC_PORT=xxxx
````
This will change the port that the operator is running on.

ENDPOINTS

POST /register

Expects a query parameter "name" that is the name of the handler being registered as
well as a binary to be included in the request body which is the wasm containing the logic

example:
`curl -X POST "localhost:8080/register?name=foobar" --data-binary "@./path/to/foobar.wasm"`
This request writes foobar.wasm in the "registered" folder on the operator filesystem