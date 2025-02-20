# WAVS Setup and Testing

## Cloning the Repository

To get started, clone the WAVS repository along with its submodules:

```sh
git clone --recurse-submodules git@github.com:Lay3rLabs/WAVS.git
cd WAVS
```

## Environment Configuration

Copy the example environment file and modify it:

```sh
cp .env.example .env
```

Edit `.env` and make the following changes:
- Uncomment **line 1**.
- Uncomment **line 5** and change it to `info`.

## Building the Project

Run the following build commands:

```sh
just solidity-build
just wasi-build
just cosmwasm-build
```

## Modifying Test Configuration

Edit `layer-tests.toml`:
- Comment out **line 2**.
- Change **line 5** to:
  ```toml
  isolated = "eth-echo-data"
  ```

## Running Tests

Navigate to the test package directory and execute tests:

```sh
cd packages/layer-tests
cargo test
```

## Notes
- Ensure you have the required dependencies installed before building and testing.
- If you encounter any issues, verify the modifications made to `.env` and `layer-tests.toml`.

