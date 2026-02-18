# WAVS APP

Goal: make it so easy to run a WAVS node so that anyone mildly technical can do it.

MVP Requirements:
- [x] Wallet Support! Let's go with just porto.sh for now.
- [x] Add UI for health
- [x] Ability to create a POA contract onchain
- [ ] UI to create a service manager contract?
- [ ] Add UI to edit wavs.toml file and managing chains
- [ ] WAVS logo
- [ ] Better aesthetics
- [ ] On PoA registry page, make it easy to copy addresses with an address component

- [ ] Ability to upload new components and manage services
- [ ] Easy way to test (deploy local PoA service, add a component, trigger)
- [ ] Run headless?
- [ ] Figure out what to do with aggregator
- [ ] Rebase on main


One big improvement we can make is to add some UI to deploy and interact with POAStakeRegistry contracts on different chains! I've added `@wavs/poa-middleware` as a dependency to 


Post MVP:
- Start building out proof of reputation system
- WAVS plus ZK sidecar
