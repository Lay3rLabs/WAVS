## One-time setup

First, copy the `.example.env` file to `.env`, and edit as needed

```bash
cp packages/aggregator/.env.example packages/aggregator/.env
```

## Up and running

Next, open up two terminals. One will be for running all the background servers, the other will be for executing commands on them.

Keep in mind that on the very first run, things may take a while since they're compiling the tools. Subsequent runs will be much faster.

### Terminal 1 (servers)

```bash
just start-all
```

### Terminal 2 (client)

1. Deploy the core eigenlayer contracts 

This needs to be done each time you (re)start the servers

```bash
just cli-deploy-core
```

2. Deploy a service with one of the provided components

Do this each time you want to deploy a new service, with those core contracts deployed above:

```bash
just cli-deploy-service examples/build/components/echo_data.wasm
```

This will output a bunch of info, and finally the `Service ID` - copy that for the next step

3. Add a task for that service

```bash
just cli-add-task {Service ID} {Hex encoded data}
```

For example, if our Service ID is "01948ead04277a81ad84dcf6b3390912", we'd run the following to send a task with data of "hello world":

```bash
just cli-add-task 01948ead04277a81ad84dcf6b3390912 68656C6C6F20776F726C64
```

Or you can point to a file on disk with the `@` prefix on the input:

```bash
just cli-add-task 01948ead04277a81ad84dcf6b3390912 @~/my-file.txt
```

# Installing the CLI system-wide

You can install the `wavs-cli` tool anywhere on your system, most of those `just` commands above are just shorthand for executing this tool.

1. `cargo install --path ./packages/cli`

Next, setup your `.env` file or however you like to populate environment variables in your system, based on all the vars in [.env.example](.env.example)

Now you have `wavs-cli` and can run it from anywhere (but certain things like auto-deploy the example contracts won't work outside the repo)

# Custom services

The simple commands above all use the provided contracts and components. For developing new services, you'll want to pass different parameters and change some defaults. 

Run `wavs-cli --help` or `wavs-cli [subcommand] --help` for all the options, but here are a few common ones:

### Deploying a service

* --trigger: either `eth-contract-event` or `cosmos-contract-event`
* --trigger-event-name: the event hash (hex encoded) for ethereum, or event type for cosmos
* --trigger-chain: the chain name to send the trigger on
* --trigger-address: the address of a previously deployed trigger contract

### Tasks

The `add-task` command assumes a specific "example trigger" format for the trigger contract and payload data.

This won't work with custom contracts, as of right now you'll need to write separate tooling for that.

# Chains

Edit [packages/wavs/wavs.toml](packages/wavs/wavs.toml) to change the active trigger chains, adjust chain configs, etc.

If targetting something more than `local`, such as a Cosmos chain, make sure you're running this separately, it won't be launched automatically with the `just start-all` command above

### Debugging

One common problem is that the wavs data directories are set to a place that requires superuser permissions or does not exist.

Make sure the `*_DATA` set in your env vars point to a valid location, and it's not unusual to need to delete these directories as new updates to WAVS are released.

### Aggregator

The default `local` chain does not use the Aggregator service. If you want to enable this, you can change the submission chain to `local-aggregator`, but make sure that you aren't expecting tasks to immediately propogate by passing `--result-timeout-ms 0` to the `add-task` subcommand

### Running with Docker

_TODO: document this :)_

### Running natively

1. Install the binaries on your system

You need to decide where the configuration and runtime-data files should be.

For example, this will install everything and use `~/wavs-data` as the files path:

```bash
just install-native ~/wavs-data
```

When the command succeeds, it'll tell you to add the `WAVS_HOME` and `WAVS_DOTENV` vars to your system environment. Make sure to do that!

2. Now you're off to the races! 

Start the servers in different terminals:

```bash
wavs
```

```bash
wavs-aggregator
```

Start anvil for Ethereum:

```bash
anvil
```

Run CLI commands:

```bash
wavs-cli deploy-eigen-core
```

Now, since we're using the CLI outside of the repo, there are no defaults for the component or trigger, we need to specify them, e.g.:

```bash
wavs-cli deploy-service --component ~/path/to/my/component.wasm --trigger eth-contract-event --trigger-event-name 86eacd23610d81706516de1ed0476c87772fdf939c7c771fbbd7f0230d619e68
```