//! Browser adapter for the renderer-independent Swarm Space simulation.
//!
//! This crate deliberately exports a small JSON snapshot boundary. The web
//! client owns presentation and input; `swarm-core` remains the only world
//! authority and `swarm-runner` remains the only bot scheduler.

use serde::Serialize;
use swarm_core::{Pos, Role, Scenario, Team};
use swarm_runner::{MatchRunner, RenderSnapshot};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WebMatch {
    seed: u64,
    runner: MatchRunner,
}

#[wasm_bindgen]
impl WebMatch {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Self {
        let seed = u64::from(seed);
        Self {
            seed,
            runner: MatchRunner::new(seed, Scenario::default()),
        }
    }

    pub fn restart(&mut self, seed: u32) {
        self.seed = u64::from(seed);
        self.runner = MatchRunner::new(self.seed, Scenario::default());
    }

    pub fn step(&mut self) -> bool {
        self.runner.step().is_some()
    }

    pub fn run_steps(&mut self, count: u32) -> u32 {
        let mut advanced = 0;
        for _ in 0..count.min(64) {
            if !self.step() {
                break;
            }
            advanced += 1;
        }
        advanced
    }

    pub fn is_finished(&self) -> bool {
        self.runner.snapshot().finished
    }

    pub fn snapshot_json(&self) -> String {
        serde_json::to_string(&SnapshotDto::from_snapshot(
            self.seed,
            self.runner.snapshot(),
        ))
        .expect("render snapshot must serialize")
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotDto {
    seed: u64,
    width: i32,
    height: i32,
    turn: u32,
    max_turns: u32,
    scores: [u32; 2],
    strategies: [&'static str; 2],
    bases: [PointDto; 2],
    walls: Vec<PointDto>,
    drones: Vec<DroneDto>,
    crystals: Vec<CrystalDto>,
    explored: [Vec<PointDto>; 2],
    finished: bool,
    last_event: String,
    turn_explanation: String,
}

impl SnapshotDto {
    fn from_snapshot(seed: u64, snapshot: RenderSnapshot) -> Self {
        let drones = snapshot
            .drones
            .iter()
            .cloned()
            .map(|drone| {
                let visible_to = Team::ALL.map(|team| {
                    drone.team == team || snapshot.currently_visible(team, drone.position)
                });
                DroneDto::from_render(drone, visible_to)
            })
            .collect();
        let mut walls = snapshot
            .walls
            .into_iter()
            .map(PointDto::from)
            .collect::<Vec<_>>();
        walls.sort_by_key(|point| (point.y, point.x));

        let mut crystals = snapshot
            .crystals
            .into_iter()
            .map(|crystal| CrystalDto {
                x: crystal.position.x,
                y: crystal.position.y,
                amount: crystal.amount,
            })
            .collect::<Vec<_>>();
        crystals.sort_by_key(|crystal| (crystal.y, crystal.x));

        let explored = snapshot.memories.map(|memory| {
            let mut points = memory
                .explored
                .into_iter()
                .map(PointDto::from)
                .collect::<Vec<_>>();
            points.sort_by_key(|point| (point.y, point.x));
            points
        });

        Self {
            seed,
            width: snapshot.scenario.width,
            height: snapshot.scenario.height,
            turn: snapshot.turn,
            max_turns: snapshot.scenario.max_turns,
            scores: snapshot.scores,
            strategies: snapshot
                .scenario
                .strategies
                .map(|strategy| strategy.label()),
            bases: snapshot.bases.map(PointDto::from),
            walls,
            drones,
            crystals,
            explored,
            finished: snapshot.finished,
            last_event: snapshot.last_event,
            turn_explanation: snapshot.turn_explanation,
        }
    }
}

#[derive(Clone, Copy, Serialize)]
struct PointDto {
    x: i32,
    y: i32,
}

impl From<Pos> for PointDto {
    fn from(position: Pos) -> Self {
        Self {
            x: position.x,
            y: position.y,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DroneDto {
    id: usize,
    team: &'static str,
    x: i32,
    y: i32,
    cargo: u8,
    role: &'static str,
    target: Option<PointDto>,
    reason: String,
    visible_to: [bool; 2],
}

impl DroneDto {
    fn from_render(drone: swarm_runner::RenderDrone, visible_to: [bool; 2]) -> Self {
        Self {
            id: drone.id,
            team: team_name(drone.team),
            x: drone.position.x,
            y: drone.position.y,
            cargo: drone.cargo,
            role: role_name(drone.role),
            target: drone.target.map(PointDto::from),
            reason: drone.reason,
            visible_to,
        }
    }
}

#[derive(Serialize)]
struct CrystalDto {
    x: i32,
    y: i32,
    amount: u8,
}

const fn team_name(team: Team) -> &'static str {
    match team {
        Team::Azure => "azure",
        Team::Amber => "amber",
    }
}

const fn role_name(role: Role) -> &'static str {
    match role {
        Role::Courier => "courier",
        Role::Scout => "scout",
        Role::Harvester => "harvester",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_snapshot_is_stable_json() {
        let game = WebMatch::new(42);
        let json = game.snapshot_json();
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid snapshot JSON");
        assert_eq!(value["turn"], 0);
        assert_eq!(value["width"], 24);
        assert_eq!(value["drones"].as_array().map(Vec::len), Some(6));
    }

    #[test]
    fn browser_adapter_advances_the_authoritative_runner() {
        let mut game = WebMatch::new(7);
        assert!(game.step());
        let value: serde_json::Value =
            serde_json::from_str(&game.snapshot_json()).expect("valid snapshot JSON");
        assert_eq!(value["turn"], 1);
    }
}
