# Wit Authoring Meeting Notes from Dec 12, 2024

## Wit deps and imports

The scoping of a `wit` package is determined by the package declaration at the top of a `wit` file.  
Using the `include` and `use` keywords, one can reference types defined in other `wit` packages.  
[wasm-tools](https://github.com/bytecodealliance/wasm-tools/tree/main) is a great resource for determining if your package has been properly informed about its dependencies.  `wasm-tools component wit` can be used to turn wit into binary or vice versa with various flags, and will fail to roundtrip if “nonexistent” dependencies are being referenced. The `wkg` CLI can be used to do that as well, building a `wit` binary as a `.wasm` while resolving package dependencies published to a registry.

[https://component-model.bytecodealliance.org/design/wit.html](https://component-model.bytecodealliance.org/design/wit.html)  
[https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md)

## `wkg` (pronounced “wackage”) replaces the `wit` CLI 

WAVS appears to still be using the wit CLI, which is defined in the cargo-component repo.  Since then, folks have migrated to using the wkg tool.  It currently supports usage of both the OCI and Warg protocols, though there are plans to deprecate and remove support for Warg in the near future.  What is nice about wkg in comparison to the wit CLI is that you no longer need to specify your dependencies in a .toml, but instead they are parsed directly from the wit files.  When running wkg wit build —-wit-dir \<some-path\> you will get a binary.wasm that includes all of the types needed from any of the transitive dependencies of your wit file.  You can also do wkg wit fetch from the parent directory of a wit file and it will put all of the transitive dependencies in a deps directory.

[https://crates.io/crates/wkg](https://crates.io/crates/wkg)

## Wit Binary vs Wit `deps` Directory in Text Format

There are two equivalent ways of giving a package access to its dependencies.  The `deps` dir predates the existence of registries.  One of the most tedious and frustrating parts of authoring components became copying `wit` dirs around from project to project.  So people are pretty eager to use registry tooling given the frustration that `deps` dirs have given them in the past.

There are also “nested packages” that have scopes in a single wit file.  These were added recently, and their first primary use case has been when using `wkg` to build a file.wasm from a wit package.  Using `wasm-tools wit component` to round trip the wasm back into a wit file will show a single wit file where instead of package dependencies in a wit directory, you have for example `package wasi:io { … }`  in the same file as the consumer of the types from `wasi:io`

## `wit-bindgen` and `cargo-component`

There are two ways to author components in Rust: (1) `cargo-component` and (2) `wit-bindgen`. Under the hood, `cargo-component` uses `wit-bindgen` but adds registry support. If you use `wit-bindgen` directly, you will be using a Rust macro and pointing to a binary `.wasm` file `wit` package or `wit` directory (with `wit/deps` folder) that contains the text `.wit` files. You can use `wkg` CLI to manually setup your local directories and resolve registry deps and then use `wit-bindgen`. Alternatively, you can use `cargo-component` that uses both `wkg` and `wit-bindgen` under the hood and aims to be more streamlined for authoring components.

## `cargo-component` Cargo.toml fields

Here too there are fields that predate the existence of registries, and as a result some of them feel counterintuitive.  Personally I think the easiest work flow is to publish packages often and then create projects with cargo component new –lib –target \<namespace:pkg/world\> and then version bumps should occur automatically with new releases.  If you want to avoid a needless publish though, there are flows that you can use.  While it does work today, there are planned changes that should make things more intuitive.  There is a pending PR at this moment to add the wkg.lock to cargo-component so that it uses that rather than the Cargo-component.lock which should be a nice step towards making things work more intuitively.

If you want to link to a local `wit` package for `cargo-component` you can add the following to your `Cargo.toml`

`[package.metadata.component.target]`  
`path = "../path/to/wit"`

Note that this is not the best at picking up items from your deps dir.  Though if you simply create a new project with `cargo component new` then you should have a wit folder in your existing repo where a deps dir will work, and in either case, using the nested package syntax above would work as well.  The syntax above assumes the wit dir has a single world in it.  If there are multiple worlds in the `wit` project, then you’ll need to specify which world you’d like your project to target, via the following underneath the above declaration in `Cargo.toml`

`world = “<world>"`

There is also a confusing distinction between dependencies and “target” dependencies.  Non-target dependencies are basically unusable today.  `cargo component add` adds a dependency and `cargo component add —-target` adds a target dependency.  Obviously it’s not great that omitting the flag is basically discouraged today.  The following fields are added to your `Cargo.toml` respectively when using these commands.

`[package.metadata.component.dependencies]`  
`[package.metadata.component.target.dependencies]`

The idea is that eventually `dependencies` will refer to concrete components that implement/target worlds rather than specifying dependencies on types that are provided via `wit` packages.  Both dependencies should be commonplace in the future, though the concrete dependencies are not really usable today.

You can also either use semver or `path = “path/to/wit”` when using target dependencies.  If you struggle to pick up your transitive `wit` dependencies when using a path, it is often easiest to get your transitive dependencies from a registry, rather than using a path, and then only use a path for your top level `wit` package definition when following the workflow of editing a local `wit` manually.

