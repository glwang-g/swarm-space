# Swarm Space project anchor

This repository is the canonical working project for Swarm Space. Do not use
`living-world` as the implementation root; that repository is a separate,
older prototype.

## Product direction

Swarm Space is a programmable computational world: agents observe a partial
world, submit intents, and the authoritative engine resolves movement,
conflicts, resources, and history. New work should make rules observable,
leave consequences in the world, and keep a path open for external bots or
automation.

## Repository boundaries

- `crates/swarm-core`: authoritative world rules and observations.
- `crates/swarm-runner`: bot scheduling, turns, replay, and snapshots.
- `src/lib.rs`: small Rust/WASM browser adapter.
- `web/` and `index.html`: Canvas client only; it does not own world state.
- `examples/` and `bots/`: bot authoring surface.

The former Bevy viewer is archived outside this repository at
`/Users/mac/works/github.com/swarm-space-bevy-archive` on the development
machine. It is not part of the production web client.

## Verification

```bash
cargo test --workspace --all-targets
cargo check --workspace --target wasm32-unknown-unknown
trunk build --release
```

Deployment is triggered by pushing `master`; the self-hosted runner builds and
publishes the site to `https://swarm.freexlib.com`.
