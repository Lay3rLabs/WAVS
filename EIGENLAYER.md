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

   If the submodule has not been added to your repository yet, you can add it manually:

   ```bash
   git submodule add git@github.com:Layr-Labs/eigenlayer-contracts.git contracts/lib/eigenlayer-contracts
   ```

## Setup

1. **Create Environment Configuration**  
   Copy the example environment file to create your `.env` configuration:

    ```bash
    cp .env.example .env
     ```

2. **Start Anvil**  
   Launch a local Anvil blockchain instance by running:

     ```bash
     anvil
     ```

3. **Configure Deployment Key**  
   - Anvil will display a list of private and public keys upon startup. Choose one of the private keys and add it to your `.env` file under `DEPLOYER_PRIVATE_KEY`.
   - **Note**: The default address provided in `.env.example` is an Anvil-generated address, so it should work without changes if you're using the default Anvil setup.

## Available Scripts

- Deploy EigenLayer: `./scripts/deploy_el.sh`
  - EigenLayer is a set of smart contracts that enable restaking of assets to secure new services called AVS
- Deploy hello-world: `./scripts/deploy_avl.sh`
  - Simplest AVS contract
- Deploy all contracts above: `./scripts/deploy_el_full.sh`

## Source

Contracts were copied from hello-world avs: https://github.com/Layr-Labs/hello-world-avs/tree/001dc6e944280559dfb44f75faf5102349a61d8e/contracts