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
- [ ] Better aesthetics
- [ ] Better wallet (show balances on chains?)
- [ ] Leverage touch id for key management? (need macOS code signing)
- [ ] Test submission with an actual solidity contract

Post MVP:
- Start building out proof of reputation system
- Update docs
- WAVS plus ZK sidecar
- Commonware
