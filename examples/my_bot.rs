//! Minimal custom Bot entry point.
//!
//! Run with `cargo run --example my_bot`. The browser client intentionally
//! ships only built-in baselines; tournament adapters can load custom bots
//! through the same `Bot` trait without granting them world authority.

use swarm_core::bots::{BaselineBot, Bot};
use swarm_core::{CARGO_CAPACITY, Decision, Intent, Observation, Role, Scenario, Team};
use swarm_runner::MatchRunner;

struct MyBot;

impl Bot for MyBot {
    fn decide(&mut self, view: &Observation) -> Decision {
        let me = &view.me;
        if me.position == view.base && me.cargo > 0 {
            return Decision::new(
                Intent::Deposit,
                Role::Courier,
                Some(view.base),
                "MyBot: deposit cargo",
            );
        }
        if me.cargo == CARGO_CAPACITY {
            return Decision::new(
                Intent::Move(step_toward(view, view.base)),
                Role::Courier,
                Some(view.base),
                "MyBot: cargo full",
            );
        }
        if view.known_crystals.get(&me.position).copied().unwrap_or(0) > 0 {
            return Decision::new(
                Intent::Harvest,
                Role::Harvester,
                Some(me.position),
                "MyBot: harvest",
            );
        }
        let target = view
            .known_crystals
            .iter()
            .filter(|(_, amount)| **amount > 0)
            .map(|(position, _)| *position)
            .min_by_key(|position| (me.position.distance(*position), position.x, position.y));
        if let Some(target) = target {
            return Decision::new(
                Intent::Move(step_toward(view, target)),
                Role::Courier,
                Some(target),
                "MyBot: nearest known crystal",
            );
        }
        Decision::new(Intent::Wait, Role::Scout, None, "MyBot: waiting for intel")
    }
}

fn step_toward(view: &Observation, target: swarm_core::Pos) -> swarm_core::Pos {
    let current = view.me.position;
    current
        .neighbors()
        .into_iter()
        .filter(|position| {
            position.x >= 0
                && position.x < view.width
                && position.y >= 0
                && position.y < view.height
                && !view.known_walls.contains(position)
        })
        .min_by_key(|position| (position.distance(target), position.x, position.y))
        .unwrap_or(current)
}

fn main() {
    let scenario = Scenario::default();
    let mut runner = MatchRunner::with_bot_factory(42, scenario, move |team, _| {
        if team == Team::Azure {
            Box::new(MyBot)
        } else {
            Box::new(BaselineBot::new(scenario.strategies[team.index()]))
        }
    });
    let result = runner.run_to_end();
    println!(
        "seed={} turns={} azure={} amber={}",
        result.seed, result.turns, result.scores[0], result.scores[1]
    );
}
