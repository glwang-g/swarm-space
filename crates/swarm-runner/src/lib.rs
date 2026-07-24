//! Headless match runner for Swarm Space.
//!
//! This crate deliberately knows nothing about Bevy. It is the seam for CLI
//! tournaments, server matches, replay generation, and future WASM adapters.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;
use swarm_core::bots::{AgentMemory, Bot, TickBudget};
use swarm_core::{
    Crystal, Decision, Drone, MatchEvent, Scenario, Simulation, Team, TeamMemory, WorldEvent,
    WorldSnapshot,
};
use swarm_core::{Intent, Role};

pub const RENDER_CARGO_CAPACITY: u8 = swarm_core::CARGO_CAPACITY;
const BOT_PATH_NODE_BUDGET: usize = 10_000;

#[derive(Clone, Debug)]
pub struct ReplayTurn {
    pub turn: u32,
    pub explanation: String,
    pub event: String,
    pub world_events: Vec<WorldEvent>,
}

#[derive(Clone, Debug)]
pub struct MatchResult {
    pub seed: u64,
    pub scores: [u32; 2],
    pub turns: u32,
    pub winner: Option<Team>,
    pub replay: Vec<ReplayTurn>,
}

/// Stable presentation DTO. The viewer never receives `Simulation`, `Drone`,
/// or other authoritative core containers directly.
#[derive(Clone, Debug)]
pub struct RenderSnapshot {
    pub scenario: Scenario,
    pub turn: u32,
    pub scores: [u32; 2],
    pub bases: [swarm_core::Pos; 2],
    pub walls: HashSet<swarm_core::Pos>,
    pub drones: Vec<RenderDrone>,
    pub crystals: Vec<RenderCrystal>,
    pub memories: [RenderMemory; 2],
    pub finished: bool,
    pub last_event: String,
    pub turn_explanation: String,
}

impl RenderSnapshot {
    pub fn currently_visible(&self, team: Team, position: swarm_core::Pos) -> bool {
        self.drones
            .iter()
            .filter(|drone| drone.team == team)
            .any(|drone| drone.position.distance(position) <= swarm_core::SENSOR_RANGE)
    }
}

#[derive(Clone, Debug)]
pub struct RenderDrone {
    pub id: usize,
    pub team: Team,
    pub position: swarm_core::Pos,
    pub cargo: u8,
    pub role: swarm_core::Role,
    pub target: Option<swarm_core::Pos>,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct RenderCrystal {
    pub position: swarm_core::Pos,
    pub amount: u8,
}

#[derive(Clone, Debug, Default)]
pub struct RenderMemory {
    pub explored: HashSet<swarm_core::Pos>,
    pub known_walls: HashSet<swarm_core::Pos>,
    pub known_crystals: HashMap<swarm_core::Pos, u8>,
}

impl From<WorldSnapshot> for RenderSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            scenario: snapshot.scenario,
            turn: snapshot.turn,
            scores: snapshot.scores,
            bases: snapshot.bases,
            walls: snapshot.walls,
            drones: snapshot.drones.into_iter().map(RenderDrone::from).collect(),
            crystals: snapshot
                .crystals
                .into_iter()
                .map(RenderCrystal::from)
                .collect(),
            memories: snapshot
                .memories
                .into_iter()
                .map(RenderMemory::from)
                .collect::<Vec<_>>()
                .try_into()
                .expect("two team memories"),
            finished: snapshot.finished,
            last_event: snapshot.last_event,
            turn_explanation: snapshot.turn_explanation,
        }
    }
}

impl From<Drone> for RenderDrone {
    fn from(drone: Drone) -> Self {
        Self {
            id: drone.id,
            team: drone.team,
            position: drone.position,
            cargo: drone.cargo,
            role: drone.role,
            target: drone.target,
            reason: drone.reason,
        }
    }
}

impl From<Crystal> for RenderCrystal {
    fn from(crystal: Crystal) -> Self {
        Self {
            position: crystal.position,
            amount: crystal.amount,
        }
    }
}

impl From<TeamMemory> for RenderMemory {
    fn from(memory: TeamMemory) -> Self {
        Self {
            explored: memory.explored,
            known_walls: memory.known_walls,
            known_crystals: memory.known_crystals,
        }
    }
}

impl MatchResult {
    pub fn replay_text(&self) -> String {
        self.replay
            .iter()
            .map(|turn| {
                let structured = turn
                    .world_events
                    .iter()
                    .map(format_world_event)
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "turn {}\n{}\nEVENT: {}{}",
                    turn.turn,
                    turn.explanation,
                    turn.event,
                    if structured.is_empty() {
                        String::new()
                    } else {
                        format!("\n{}", structured)
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

fn format_world_event(event: &WorldEvent) -> String {
    match event {
        WorldEvent::Moved {
            drone_id,
            team,
            from,
            to,
        } => format!(
            "MOVE {}-{} {} -> {}",
            team.label(),
            drone_id + 1,
            from.board_label(),
            to.board_label()
        ),
        WorldEvent::Harvested {
            drone_id,
            team,
            position,
            amount,
        } => format!(
            "HARVEST {}-{} {} +{}",
            team.label(),
            drone_id + 1,
            position.board_label(),
            amount
        ),
        WorldEvent::Deposited {
            drone_id,
            team,
            amount,
        } => format!("DEPOSIT {}-{} +{}", team.label(), drone_id + 1, amount),
    }
}

pub struct MatchRunner {
    seed: u64,
    simulation: Simulation,
    bots: Vec<Box<dyn Bot>>,
    memories: Vec<AgentMemory>,
    replay: Vec<ReplayTurn>,
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
    Snapshot(RenderSnapshot),
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

impl Drop for RunnerHandle {
    fn drop(&mut self) {
        let _ = self.commands.send(MatchCommand::Shutdown);
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
            if running && !runner.simulation.is_finished() {
                advance(&mut runner, &update_tx);
                if runner.simulation.is_finished() {
                    running = false;
                }
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
    RunnerHandle {
        commands: command_tx,
        updates: Mutex::new(update_rx),
    }
}

fn advance(runner: &mut MatchRunner, updates: &Sender<RunnerUpdate>) {
    if let Some(event) = runner.step() {
        if let Some(turn) = runner.replay.last() {
            let _ = updates.send(RunnerUpdate::Event(MatchEvent::TurnAdvanced {
                turn: turn.turn,
                explanation: turn.explanation.clone(),
                event: turn.event.clone(),
                world_events: turn.world_events.clone(),
            }));
        }
        if let MatchEvent::MatchFinished { turn, scores } = event {
            let _ = std::fs::create_dir_all("replays");
            let _ = runner.write_replay(format!("replays/seed-{}.log", runner.seed));
            let _ = updates.send(RunnerUpdate::Event(MatchEvent::MatchFinished {
                turn,
                scores,
            }));
        }
        let _ = updates.send(RunnerUpdate::Snapshot(runner.snapshot()));
    }
}

impl MatchRunner {
    pub fn new(seed: u64, scenario: Scenario) -> Self {
        Self::with_bot_factory(seed, scenario, |team, _id| {
            Box::new(swarm_core::bots::BaselineBot::new(
                scenario.strategies[team.index()],
            ))
        })
    }

    pub fn with_bot_factory<F>(seed: u64, scenario: Scenario, mut factory: F) -> Self
    where
        F: FnMut(Team, usize) -> Box<dyn Bot>,
    {
        let simulation = Simulation::without_bots(seed, scenario);
        let bots: Vec<Box<dyn Bot>> = simulation
            .bot_slots()
            .into_iter()
            .map(|(team, id)| factory(team, id))
            .collect();
        let memories = (0..bots.len()).map(|_| AgentMemory::default()).collect();
        Self {
            seed,
            simulation,
            bots,
            memories,
            replay: Vec::new(),
        }
    }

    pub fn step(&mut self) -> Option<MatchEvent> {
        if self.simulation.is_finished() {
            return None;
        }
        self.simulation.refresh_observations();
        let decisions: Vec<Decision> = (0..self.bots.len())
            .map(|index| {
                let observation = self.simulation.observation_for(index);
                let mut budget = TickBudget::new(BOT_PATH_NODE_BUDGET);
                let memory = &mut self.memories[index];
                let output =
                    self.bots[index].decide_with_context(&observation, memory, &mut budget);
                memory.apply(output.memory);
                memory.ticks += 1;
                if budget.exhausted() {
                    Decision::new(Intent::Wait, Role::Scout, None, "Bot exceeded tick budget")
                } else {
                    output.decision
                }
            })
            .collect();
        self.simulation.record_decisions(&decisions);
        self.simulation.resolve_tick(&decisions);
        let turn = ReplayTurn {
            turn: self.simulation.turn(),
            explanation: self.simulation.turn_explanation().to_owned(),
            event: self.simulation.last_event().to_owned(),
            world_events: self.simulation.last_world_events().to_vec(),
        };
        self.replay.push(turn.clone());
        Some(if self.simulation.is_finished() {
            MatchEvent::MatchFinished {
                turn: turn.turn,
                scores: self.simulation.scores(),
            }
        } else {
            MatchEvent::TurnAdvanced {
                turn: turn.turn,
                explanation: turn.explanation,
                event: turn.event,
                world_events: turn.world_events,
            }
        })
    }

    pub fn snapshot(&self) -> RenderSnapshot {
        self.simulation.snapshot().into()
    }

    pub fn run_to_end(&mut self) -> MatchResult {
        while !self.simulation.is_finished() {
            self.step();
        }
        self.result()
    }

    pub fn result(&self) -> MatchResult {
        let scores = self.simulation.scores();
        let winner = match scores[0].cmp(&scores[1]) {
            std::cmp::Ordering::Less => Some(Team::Amber),
            std::cmp::Ordering::Greater => Some(Team::Azure),
            std::cmp::Ordering::Equal => None,
        };
        MatchResult {
            seed: self.seed,
            scores: self.simulation.scores(),
            turns: self.simulation.turn(),
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

pub fn run_baseline_batch(
    seeds: impl IntoIterator<Item = u64>,
    scenario: Scenario,
) -> Vec<MatchResult> {
    seeds
        .into_iter()
        .map(|seed| run_baseline(seed, scenario))
        .collect()
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
            Box::new(swarm_core::bots::BaselineBot::new(
                Scenario::default().strategies[team.index()],
            ))
        });
        let initial = handle.recv().expect("initial snapshot");
        assert!(matches!(initial, RunnerUpdate::Snapshot(snapshot) if snapshot.turn == 0));
        handle.send(MatchCommand::Step).expect("step command");
        let mut saw_turn = false;
        for _ in 0..4 {
            if let Ok(RunnerUpdate::Snapshot(snapshot)) =
                handle.updates.lock().expect("update lock").try_recv()
            {
                saw_turn |= snapshot.turn == 1;
            }
            if saw_turn {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            saw_turn,
            "runner thread did not publish the stepped snapshot"
        );
        handle
            .send(MatchCommand::Shutdown)
            .expect("shutdown command");
    }

    struct MemoryBot;

    impl Bot for MemoryBot {
        fn decide(&mut self, _observation: &swarm_core::Observation) -> Decision {
            Decision::new(Intent::Wait, Role::Scout, None, "memory test")
        }

        fn decide_with_context(
            &mut self,
            _observation: &swarm_core::Observation,
            _memory: &AgentMemory,
            _budget: &mut TickBudget,
        ) -> swarm_core::bots::BotOutput {
            let mut output = swarm_core::bots::BotOutput::new(Decision::new(
                Intent::Wait,
                Role::Scout,
                None,
                "memory test",
            ));
            output.memory.assigned_target = Some(Some(swarm_core::Pos::new(2, 2)));
            output
        }
    }

    #[test]
    fn runner_persists_bot_memory_between_ticks() {
        let mut runner =
            MatchRunner::with_bot_factory(11, Scenario::default(), |_, _| Box::new(MemoryBot));
        runner.step();
        runner.step();
        assert_eq!(runner.memories[0].ticks, 2);
        assert_eq!(
            runner.memories[0].assigned_target,
            Some(swarm_core::Pos::new(2, 2))
        );
    }
}
