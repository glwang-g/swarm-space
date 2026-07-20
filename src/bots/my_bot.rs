//! Starter bot for users who want to write their own strategy.
//! Replace the body of `decide` and run the arena again.

use swarm_core::{Decision, Intent, Observation, Role, CARGO_CAPACITY};
use swarm_core::bots::Bot;

pub struct MyBot;

impl Bot for MyBot {
    fn decide(&mut self, view: &Observation) -> Decision {
        let me = &view.me;
        if me.position == view.base && me.cargo > 0 {
            return Decision::new(Intent::Deposit, Role::Courier, Some(view.base), "MyBot: deposit cargo");
        }
        if me.cargo == CARGO_CAPACITY {
            return Decision::new(Intent::Move(step_toward(view, view.base)), Role::Courier, Some(view.base), "MyBot: cargo full");
        }
        if view.known_crystals.get(&me.position).copied().unwrap_or(0) > 0 {
            return Decision::new(Intent::Harvest, Role::Harvester, Some(me.position), "MyBot: harvest");
        }
        let target = view.known_crystals.iter()
            .filter(|(_, amount)| **amount > 0)
            .map(|(position, _)| *position)
            .min_by_key(|position| (me.position.distance(*position), position.x, position.y));
        if let Some(target) = target {
            let next = step_toward(view, target);
            return Decision::new(Intent::Move(next), Role::Courier, Some(target), "MyBot: nearest known crystal");
        }
        Decision::new(Intent::Wait, Role::Scout, None, "MyBot: no known crystal yet")
    }
}

fn step_toward(view: &Observation, target: swarm_core::Pos) -> swarm_core::Pos {
    let me = view.me.position;
    me.neighbors().into_iter()
        .filter(|p| p.x >= 0 && p.x < view.width && p.y >= 0 && p.y < view.height && !view.known_walls.contains(p))
        .min_by_key(|p| (p.distance(target), p.x, p.y))
        .unwrap_or(me)
}
