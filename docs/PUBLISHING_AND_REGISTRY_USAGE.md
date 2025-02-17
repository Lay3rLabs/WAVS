# Publishing and Wasm & WIT Registry Usage

## WIT Packaging and Distribution

The WIT IDL is designed to be authored as `.wit` text files,
which is documented
[here](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md)
and [here](https://component-model.bytecodealliance.org/design/wit.html),
and then compiled into a `.wasm` binary (that is only type info) for distribution
and publishing to registries. It is easy to roundtrip between the binary and text files,
preserving code comments but not formatting and file / directory structure.

It is quite common for WIT to import types from other packages, either WASI ones or custom ones.
WIT binary packages `.wasm` are completely self-contained with all imported types from other packages
inlined. So instead of copying around a bunch of text files with a `wit/deps` directory of imported WIT packages,
we can distribute a single file.

Not all the tooling supports registry workflows yet. But for authoring Wasm components in Rust,
[`cargo-component`](https://crates.io/crates/cargo-component) is quite helpful.
For instance, you can scaffold out a new Rust project (similar to `cargo-generate`)
by just specifying a registry-published WIT package and the target world name (which determines the
imports / exports of a component). This makes it really easy to iterate on WIT and implementations.

```bash
cargo component new --lib --target <published-package-name>/<target-world-name> <new-project-dir>
```

To download a WIT or Component published to a registry:
```bash
wkg get <published-package-name>
```

By default, WIT packages will be written as text file. But you can change that with `--format wasm` arg.

If you'd like to setup a `wit` directory with all the imported package dependencies as WIT text files, which
may be expected for tooling that is not registry-aware like `wit-bindgen` or `componentize-py`:

```bash
mkdir wit && wkg get <published-package-name> -o wit/ && wkg wit fetch
```

This will create a `wit` dir, download the WIT package into the `wit` dir, and then create `wit/deps/` with
the other imported package dependencies.


## Publishing to a Warg Registry

Currently, the [`wavs:worker`](https://wa.dev/wavs:worker) WIT package is published on [wa.dev](https://wa.dev).
In order to use the `wkg publish` command, we need to first authenticate and configure with the `warg` CLI.

```bash
cargo install warg-cli wkg
```

And then follow the [account setup and authentication instructions](https://wa.dev/account/credentials/new).

If your `wkg` default registry is not yet configured:

```bash
wkg config --default-registry wa.dev
```

To publish new versions of the `wavs` packages, you may need to [manage account permissions](https://wa.dev/config/wavs).

When you modify the WIT package, you will need to version bump by changing the version at the top of the `.wit` file.

Then you will need to build the package (from the parent dirctory of the `wit` dir):
```bash
wkg wit build
```

And then:
```bash
wkg publish <file-name>
```

These two commands are likely to be combined soon into a single command.

If you need to `yank` a previous version, you can use the `warg` CLI, but do this carefully as it is irreversible:

```bash
warg publish yank --name <package-name> --version <version-to-yank>
```

Also, if you'd like to easily do a release build and publish with `cargo-component`, there's a command for that:

```bash
cargo component publish
```

But you may want to double check your `Cargo.toml` file and set the package name and namespace that you would like to
publish on the registry. Modify:
```toml
[package.metadata.component]
package = "my-namespace:my-package-name"
```

The `cargo component new` command generates the `Cargo.toml` with the default namespace of `component`, but you can
specify a different namespace with `cargo component new --namespace <my-namespace>` arg.
