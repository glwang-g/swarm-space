//! Headless match runner for Swarm Space.
//!
//! This crate deliberately knows nothing about Bevy. It is the seam for CLI
//! tournaments, server matches, replay generation, and future WASM adapters.

use swarm_core::{MatchEvent, Scenario, Simulation, Team, WorldSnapshot};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
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

#[derive(Clone, Debug)]
pub enum MatchCommand {
    Step,
    SetRunning(bool),
    SetSpeed(u32),
    Restart { seed: u64, scenario: Scenario },
    Shutdown,
}

#[derive(Clone, Debug)]
pub enum RunnerUpdate {
    Snapshot(WorldSnapshot),
    Event(MatchEvent),
}

/// The Bevy viewer owns this communication handle, never the MatchRunner.
pub struct RunnerHandle {
    commands: Sender<MatchCommand>,
    updates: Mutex<Receiver<RunnerUpdate>>,
}

impl RunnerHandle {
    pub fn send(&self, command: MatchCommand) -> Result<(), mpsc::SendError<MatchCommand>> {
        self.commands.send(command)
    }

    pub fn try_recv(&self) -> Result<RunnerUpdate, TryRecvError> {
        self.updates.lock().expect("runner update lock").try_recv()
    }

    pub fn recv(&self) -> Option<RunnerUpdate> {
        self.updates.lock().expect("runner update lock").recv().ok()
    }
}

pub fn spawn_runner<F>(seed: u64, scenario: Scenario, factory: F) -> RunnerHandle
where
    F: FnMut(Team, usize) -> Box<dyn Bot> + Send + 'static,
{
    let (command_tx, command_rx) = mpsc::channel();
    let (update_tx, update_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut factory = factory;
        let mut runner = MatchRunner::with_bot_factory(seed, scenario, &mut factory);
        let mut running = false;
        let mut speed = 1_u32;
        let _ = update_tx.send(RunnerUpdate::Snapshot(runner.snapshot()));
        loop {
            while let Ok(command) = command_rx.try_recv() {
                match command {
                    MatchCommand::Step => advance(&mut runner, &update_tx),
                    MatchCommand::SetRunning(value) => running = value,
                    MatchCommand::SetSpeed(value) => speed = value.clamp(1, 16),
                    MatchCommand::Restart { seed, scenario } => {
                        runner = MatchRunner::with_bot_factory(seed, scenario, &mut factory);
                        let _ = update_tx.send(RunnerUpdate::Snapshot(runner.snapshot()));
                        running = false;
                    }
                    MatchCommand::Shutdown => return,
                }
            }
            if running && !runner.simulation.finished {
                advance(&mut runner, &update_tx);
                if runner.simulation.finished { running = false; }
                thread::sleep(Duration::from_millis((240 / speed.max(1)) as u64));
            } else {
                match command_rx.recv_timeout(Duration::from_millis(30)) {
                    Ok(command) => match command {
                        MatchCommand::Step => advance(&mut runner, &update_tx),
                        MatchCommand::SetRunning(value) => running = value,
                        MatchCommand::SetSpeed(value) => speed = value.clamp(1, 16),
                        MatchCommand::Restart { seed, scenario } => {
                            runner = MatchRunner::with_bot_factory(seed, scenario, &mut factory);
                            let _ = update_tx.send(RunnerUpdate::Snapshot(runner.snapshot()));
                        }
                        MatchCommand::Shutdown => return,
                    },
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        }
    });
    RunnerHandle { commands: command_tx, updates: Mutex::new(update_rx) }
}

fn advance(runner: &mut MatchRunner, updates: &Sender<RunnerUpdate>) {
    if let Some(event) = runner.step() {
        if let Some(turn) = runner.replay.last() {
            let _ = updates.send(RunnerUpdate::Event(MatchEvent::TurnAdvanced {
                turn: turn.turn,
                explanation: turn.explanation.clone(),
                event: turn.event.clone(),
            }));
        }
        if let MatchEvent::MatchFinished { turn, scores } = event {
            let _ = updates.send(RunnerUpdate::Event(MatchEvent::MatchFinished { turn, scores }));
        }
        let _ = updates.send(RunnerUpdate::Snapshot(runner.snapshot()));
    }
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

    #[test]
    fn command_channel_advances_a_remote_runner() {
        let handle = spawn_runner(7, Scenario::default(), |team, _| {
            Box::new(swarm_core::bots::BaselineBot::new(Scenario::default().strategies[team.index()]))
        });
        let initial = handle.recv().expect("initial snapshot");
        assert!(matches!(initial, RunnerUpdate::Snapshot(snapshot) if snapshot.turn == 0));
        handle.send(MatchCommand::Step).expect("step command");
        let mut saw_turn = false;
        for _ in 0..4 {
            if let Ok(RunnerUpdate::Snapshot(snapshot)) = handle.updates.lock().expect("update lock").try_recv() {
                saw_turn |= snapshot.turn == 1;
            }
            if saw_turn { break; }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(saw_turn, "runner thread did not publish the stepped snapshot");
        handle.send(MatchCommand::Shutdown).expect("shutdown command");
    }
}
