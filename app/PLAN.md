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
- [x] Security (make sure it works with environment variable, update MCP.md to recommend putting in .env, make sure .env.example is up to date)
- [x] Rename chain_write_credential to wavs_mcp_signer_mnemonic
- [x] Test submission with an actual solidity contract
- [x] Test DevEx in new repo (component development, deploying contracts, etc.)
- [x] Submissions not showing up
- [x] Services not loading on app restart
- [x] Better aesthetics
- [x] Environment variables
- [x] Visualize Component KV store, config, environment variables
- [x] App branding
- [x] Select file / folder UX
- [ ] Get MCP working with claude desktop
- [ ] Improve skills and MCP installation DX. gsd shows how to install things globally.

Let's work on some small bug fixes and UX improvements:
- Improve the UX for activity cards. I shouldn't have to expand to see submission info. For onchain submissions, it would be nice to showcase tx hash if possible. Activity should showcase events which have triggers, results, and submission info
- In the activity feed, it would be nice to be smarter about decoding results. Right now, every result just shows as a vec which is not human readable.
- Bug: Services often don't restart correctly when reloading WAVS.
- UX nit, in the settings, for the wallet actions (reset, reveal seed) the should behind a kabab menu and open as a dropdown, they are uncommon actions so require less verticle space.



Clean up:
- wavs.toml has unnecessary things in it, maybe those should be there and documented but commented out?

Post MVP:
- Stats (CPU, Memory, etc.)
- LLM Config (makes those LLMs available to WASM components)?
- P2P Page
- WAVS Service Registry
- WAVS Trust Graph (Start building out proof of reputation system)
- WAVS plus ZK sidecar
- Commonware
- Maybe consider making the MCP a WAVS Component?

Minor bugs:
- [ ] App restart is broken in dev mode
