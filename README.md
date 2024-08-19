# wasmatic

Wasmatic makes use of [wasmtime](https://github.com/bytecodealliance/wasmtime) for registering and running wasm handlers.

## Getting started

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

`wasmatic up --dir registered::registered -S common -S http path/to/proxy.wasm`

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
`curl -X POST "localhost:8080/register?name=foobar" --data-binary "@./task-acl.wasm"`
write `foobar.wasm` in the "registered" folder on the operator filesystem