# Eigenlayer

## Requirements

Before using Eigenlayer, ensure you have the following tools installed:

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