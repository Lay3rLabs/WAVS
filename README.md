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

`wasmatic up -S common -S http path/to/proxy.wasm`

You can also do some minimal configuration via environment variables.

```
export WASMATIC=path/to/proxy.wasm
```
Setting this variable enables running `wasmatic up` without specifying a path to the operator wasm.

```
export WASMATIC_PORT=xxxx
````
This will change the port that the operator is running on.