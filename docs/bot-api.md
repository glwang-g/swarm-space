# Swarm Space Bot API (v0)

Swarm Space runs the map, resources, scoring and traffic referee. A bot only
receives an `Observation` and returns one `Decision` for its own drone.

```rust
use swarm_core::{bots::Bot, Decision, Intent, Observation, Role};

struct WaitBot;

impl Bot for WaitBot {
    fn decide(&mut self, view: &Observation) -> Decision {
        Decision::new(Intent::Wait, Role::Scout, None, "Waiting for a plan")
    }
}
```

Available intents are `Move(adjacent_pos)`, `Harvest`, `Deposit`, and `Wait`.
The engine rejects illegal moves, resolves simultaneous movement, and applies
harvest/deposit rules after every bot has submitted its intent.

`Observation` intentionally includes only the drone's own state, its base,
allied shared discoveries (`explored`, `known_walls`, `known_crystals`) and
allied positions. It does not include the authoritative map, untouched hidden
crystals, opponent state, scores, or the simulation object.

To run a custom bot for every drone, construct a simulation with a factory:

```rust
let sim = Simulation::with_bot_factory(seed, scenario, |team, id| {
    Box::new(MyBot::new(team, id))
});
```

The built-in `Autonomous`, `DedicatedScout`, and `HybridScout` strategies are
implemented as ordinary `BaselineBot`s through this same interface. They are
baselines, not privileged engine behavior.
