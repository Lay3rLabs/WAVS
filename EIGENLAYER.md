# Eigenlayer

## Requirements

Before using Eigenlayer ensure you have the following tools installed:

1. **Foundry**  
   Foundry is needed for contract development, compilation, and testing. It includes:

   - **Forge**: for building and managing smart contracts.
   - **Anvil**: a local Ethereum-compatible blockchain for testing.

   You can install Foundry by running the following command:

   ```bash
   curl -L https://foundry.paradigm.xyz | bash
   foundryup
   ```

2. **Git Submodules**  
   This project includes a Git submodule for the Eigenlayer contracts and Forge Standard Library. Initialize and update submodules with:

   ```bash
   git submodule update --init --recursive
   ```

## Generating ABI 

Eigenlayer ABI is needed for everything to work correctly. While it's already checked in to the repo, it can be regenerated via:

```bash
./scripts/build_solidity.sh
```

## Source

Example AVS contracts were copied from hello-world avs: https://github.com/Layr-Labs/hello-world-avs/tree/001dc6e944280559dfb44f75faf5102349a61d8e/contracts
