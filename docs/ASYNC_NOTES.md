# Async Discussion Notes

## Intro

- Guests are the Wasm code and Hosts are the thing that executes the Wasm code. A good analogy is that of a hotel. The hotel hosts the guests. Guests have their own space and limited, restricted access to the property. Also, the guests pay to use resources and don’t have free reign.
- Async and concurrency is about doing things while you wait. For example, making multiple outgoing HTTP requests at the same time and processing the responses as they come back. You will probably want to use async for anything that involves IO.

## Async for the Guests

[https://blog.yoshuawuyts.com/building-an-async-runtime-for-wasi](https://blog.yoshuawuyts.com/building-an-async-runtime-for-wasi)
[https://wa.dev/wasi:io](https://wa.dev/wasi:io)

WASI 0.2 is readiness-based rather than completion-based. Similar to `epoll`. Makes heavy use of the `pollable` resource type.

```wit
resource pollable {
    ready: func() -> bool;
    block: func();
}
```

The guest needs an async executor. For Rust, the plan is to upstream support into Tokio, Reqwest, etc. However, it takes time to work with the crate owners. A stop gap attempt [https://docs.rs/wstd/0.4.0/wstd/runtime/index.html](https://docs.rs/wstd/0.4.0/wstd/runtime/index.html)

## Known Composition Problems

One of the main benefits of the Wasm Component Model is the ability to compose Wasm components together. Plugging imports with the exports of other components. For example, virtualizing a file system with a component instead of using a “real” one from the host. Another example, importing and exporting the same interface while modifying / expanding its behavior.

However, there can’t be multiple implementations of the same interface in a component’s composition. For example, you can’t have a component use a host implementation of `wasi:io/poll` for `wasi:http/types` while also using another implementation of `wasi:io/poll` provided by another component that exports `wasi:filesystem/types` for a virtualized in-memory file system.

Thus, if you have an interface like `wasi:io/poll` that is imported in many other packages, you have a “blast radius” problem. If you have a component that provides an implementation of any one of these packages and you want to use the host provided implementation for the rest, you will need to re-implement all of the packages.

You can see this problem play out in [https://github.com/bytecodealliance/wasi-virt](https://github.com/bytecodealliance/wasi-virt)

The other problem is that WASI 0.2 does not perform well with deeply nested asynchronous components.

## Problems to be solved with WASI 0.3

WASI 0.3 solves these problems with “deleting” `wasi:io` by implementing async and streams in the ABI. This largely solves both problems of WASI 0.2 composition. WASI 0.3 is tracking for Q1 or Q2 2025, so should start planning for it.

[https://youtu.be/hgWkiWeGEzk?si=vzWCA3FRKG6Y0oXy](https://youtu.be/hgWkiWeGEzk?si=vzWCA3FRKG6Y0oXy)
[https://youtu.be/y3x4-nQeXxc?si=mKl0napmyn5w3q3l](https://youtu.be/y3x4-nQeXxc?si=mKl0napmyn5w3q3l)

## Async / Sync in the Host

Wasmtime can be used in both async and sync mode. Tokio shouldn’t be a requirement.

[https://docs.rs/wasmtime/27.0.0/wasmtime/struct.Config.html\#method.async\_support](https://docs.rs/wasmtime/27.0.0/wasmtime/struct.Config.html#method.async_support)

However, the `wasmtime-wasi` and `wasmtime-wasi-http` implementations are wired up with Tokio. To use an alternative runtime, the host implementation for the WASI interfaces would need to be rewritten.

[https://docs.rs/wasmtime-wasi/27.0.0/wasmtime\_wasi/](https://docs.rs/wasmtime-wasi/27.0.0/wasmtime_wasi/)
[https://docs.rs/wasmtime-wasi-http/27.0.0/wasmtime\_wasi\_http/](https://docs.rs/wasmtime-wasi-http/27.0.0/wasmtime_wasi_http/)
