# Logging

Logging uses the `tracing` crate and sets filters via the EnvFilter directives

Details on the full syntax are covered in the tracing cargo docs: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#filtering-with-span-field-values

To set the directives, you have a few options:

1. `log_level` in config file
2. `--log-level` via cli arg
3. `WAVS_LOG_LEVEL` env var override (see [example wavs.toml comments](../packages/wavs/wavs.toml) for more info on overrides)
4. `RUST_LOG` env var

The `RUST_LOG` env var is the only way to set the log level for tests, and it takes the _least_ precedence for real execution.
Also, it is less forgiving of spaces in between directives (e.g. `WAVS_LOG_LEVEL="info, wavs=debug"` is ok, but `RUST_LOG="info, wavs=debug"` is not. Must be `RUST_LOG="info,wavs=debug"`.

Some useful example directives:

* "wavs=debug" - show debug level just for wavs
* "\[{subsys=TriggerManager}\]=debug" - show debug level for the TriggerManager subsystem spans

For tests, something like this is often useful:

* RUST_LOG=info,utils=debug cargo test the_test_to_focus_on -- --nocapture

The available subsys values are:

* AppContext
* Dispatcher
* Engine
* EngineRunner
* DbStorage
* CaStorage
* Submission
* TriggerManager
