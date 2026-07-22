//! Headless match runner for Swarm Space.
//!
//! This crate deliberately knows nothing about Bevy. It is the seam for CLI
//! tournaments, server matches, replay generation, and future WASM adapters.

use swarm_core::{MatchEvent, Scenario, Simulation, Team, WorldSnapshot};
use swarm_core::bots::Bot;

#[derive(Clone, Debug)]
pub struct ReplayTurn {
    pub turn: u32,
    pub explanation: String,
    pub event: String,
}

#[derive(Clone, Debug)]
pub struct MatchResult {
    pub seed: u64,
    pub scores: [u32; 2],
    pub turns: u32,
    pub winner: Option<Team>,
    pub replay: Vec<ReplayTurn>,
}

impl MatchResult {
    pub fn replay_text(&self) -> String {
        self.replay.iter().map(|turn| {
            format!("turn {}\n{}\nEVENT: {}", turn.turn, turn.explanation, turn.event)
        }).collect::<Vec<_>>().join("\n\n")
    }
}

pub struct MatchRunner {
    pub seed: u64,
    pub simulation: Simulation,
    pub replay: Vec<ReplayTurn>,
}

impl MatchRunner {
    pub fn new(seed: u64, scenario: Scenario) -> Self {
        Self::with_bot_factory(seed, scenario, |team, _id| {
            Box::new(swarm_core::bots::BaselineBot::new(scenario.strategies[team.index()]))
        })
    }

    pub fn with_bot_factory<F>(seed: u64, scenario: Scenario, factory: F) -> Self
    where
        F: FnMut(Team, usize) -> Box<dyn Bot>,
    {
        Self { seed, simulation: Simulation::with_bot_factory(seed, scenario, factory), replay: Vec::new() }
    }

    pub fn step(&mut self) -> Option<MatchEvent> {
        if self.simulation.finished { return None; }
        self.simulation.step();
        let turn = ReplayTurn {
            turn: self.simulation.turn,
            explanation: self.simulation.turn_explanation.clone(),
            event: self.simulation.last_event.clone(),
        };
        self.replay.push(turn.clone());
        Some(if self.simulation.finished {
            MatchEvent::MatchFinished { turn: turn.turn, scores: self.simulation.scores }
        } else {
            MatchEvent::TurnAdvanced { turn: turn.turn, explanation: turn.explanation, event: turn.event }
        })
    }

    pub fn snapshot(&self) -> WorldSnapshot { self.simulation.snapshot() }

    pub fn run_to_end(&mut self) -> MatchResult {
        while !self.simulation.finished { self.step(); }
        self.result()
    }

    pub fn result(&self) -> MatchResult {
        let winner = match self.simulation.scores[0].cmp(&self.simulation.scores[1]) {
            std::cmp::Ordering::Less => Some(Team::Amber),
            std::cmp::Ordering::Greater => Some(Team::Azure),
            std::cmp::Ordering::Equal => None,
        };
        MatchResult {
            seed: self.seed,
            scores: self.simulation.scores,
            turns: self.simulation.turn,
            winner,
            replay: self.replay.clone(),
        }
    }

    pub fn write_replay(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::write(path, self.result().replay_text())
    }
}

pub fn run_baseline(seed: u64, scenario: Scenario) -> MatchResult {
    MatchRunner::new(seed, scenario).run_to_end()
}

pub fn run_baseline_batch(seeds: impl IntoIterator<Item = u64>, scenario: Scenario) -> Vec<MatchResult> {
    seeds.into_iter().map(|seed| run_baseline(seed, scenario)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_finishes_without_bevy() {
        let result = run_baseline(42, Scenario::default());
        assert!(result.turns > 0);
        assert!(!result.replay.is_empty());
        assert_eq!(result.scores.len(), 2);
    }
}
