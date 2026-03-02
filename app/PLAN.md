# WAVS APP

Goal: make it so easy to run a WAVS node so that anyone mildly technical can do it.

MVP Requirements:
- [x] Wallet Support! Let's go with just porto.sh for now.
- [x] Add UI for health
- [x] Ability to create a POA contract onchain
- [x] Add UI to edit wavs.toml file and managing chains
- [x] State management (persist services and registries)
- [x] Ability to upload new components and manage services
- [x] Easy way to test (deploy local PoA service, add a component, trigger)
- [x] Better logging
- [x] Rebase on main
- [x] Figure out what is actually being used in the gui package
- [x] Health page should have better indicator (not a normal button)
- [x] Ability to pause / unpause service?
- [x] Rearrange actions on service detail page (rename some)
- [X] Bug: Some log entries are showing up twice?
- [x] Add toasts and replace annoying modals you have to close
- [x] Better wallet (show balances on chains?)
- [x] Make it easier to copy seed phrase
- [x] Figure out how to use the aggregator
- [x] Fix auto start MCP when app starts
- [x] Build an MCP
- [ ] Select file / folder UX
- [x] Security (make sure it works with environment variable, update MCP.md to recommend putting in .env, make sure .env.example is up to date)
- [x] Rename chain_write_credential to wavs_mcp_signer_mnemonic
- [x] Test submission with an actual solidity contract
- [x] Test DevEx in new repo (component development, deploying contracts, etc.)
- [-] Submissions not showing up
- [ ] Services not loading on app restart
- [ ] Better aesthetics
- [ ] App restart is broken in dev mode

Post MVP:
- LLM Config (makes those LLMs available to WASM components)
- P2P Page
- WAVS Service Registry
- WAVS Trust Graph (Start building out proof of reputation system)
- WAVS plus ZK sidecar
- Commonware
- Maybe consider making the MCP a WAVS Component?
