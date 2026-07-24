use crate::bots::{BaselineBot, Bot};
use std::collections::{HashMap, HashSet, VecDeque};

pub const WIDTH: i32 = 24;
pub const HEIGHT: i32 = 16;
pub const MAX_TURNS: u32 = 300;
pub const SENSOR_RANGE: i32 = 5;
pub const CARGO_CAPACITY: u8 = 3;
// Legacy internal helpers are retained until the next source-file split; the
// active built-in bots define the same policy in `bots.rs`.
#[allow(dead_code)]
const SCOUT_SOURCE_GOAL: usize = 2;
#[allow(dead_code)]
const SCOUT_COVERAGE_LIMIT_PERCENT: usize = 35;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    Autonomous,
    DedicatedScout,
    HybridScout,
}

impl Strategy {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Autonomous => "Autonomous",
            Self::DedicatedScout => "Dedicated scout",
            Self::HybridScout => "Hybrid scout",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Scenario {
    pub width: i32,
    pub height: i32,
    pub drones_per_team: usize,
    pub wall_chance_percent: u32,
    pub crystal_sites: usize,
    pub max_turns: u32,
    pub strategies: [Strategy; 2],
}

impl Default for Scenario {
    fn default() -> Self {
        Self {
            width: WIDTH,
            height: HEIGHT,
            drones_per_team: 3,
            wall_chance_percent: 16,
            crystal_sites: 9,
            max_turns: MAX_TURNS,
            strategies: [Strategy::Autonomous, Strategy::HybridScout],
        }
    }
}

impl Scenario {
    pub fn scaled(drones_per_team: usize, strategies: [Strategy; 2]) -> Self {
        let drones_per_team = drones_per_team.clamp(2, 16);
        let linear = (drones_per_team as f32 / 3.0).sqrt();
        let width = ((WIDTH as f32 * linear).round() as i32).max(16);
        let height = ((HEIGHT as f32 * linear).round() as i32).max(12);
        let area_scale = width as f32 * height as f32 / (WIDTH * HEIGHT) as f32;
        Self {
            width,
            height,
            drones_per_team,
            wall_chance_percent: 16,
            crystal_sites: ((9.0 * area_scale).round() as usize).max(5),
            max_turns: (MAX_TURNS as f32 * linear).round() as u32,
            strategies,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BenchmarkRow {
    pub drones_per_team: usize,
    pub dedicated_delta: f32,
    pub dedicated_win_rate: f32,
    pub hybrid_delta: f32,
    pub hybrid_win_rate: f32,
}

pub fn benchmark_leadership(max_drones: usize, seeds: u64) -> Vec<BenchmarkRow> {
    benchmark_leadership_with_progress(max_drones, seeds, |_, _| {})
}

/// Runs the same AB experiment as [`benchmark_leadership`] and reports after
/// every simulated match. The callback is deliberately small so the UI can
/// update a progress indicator without putting simulation work on its thread.
pub fn benchmark_leadership_with_progress<F: FnMut(u64, u64)>(
    max_drones: usize,
    seeds: u64,
    mut progress: F,
) -> Vec<BenchmarkRow> {
    let max_drones = max_drones.clamp(2, 16);
    let total = (max_drones as u64 - 1) * seeds * 4;
    let mut completed = 0_u64;
    (2..=max_drones.clamp(2, 16))
        .map(|drones_per_team| {
            let mut evaluate = |strategy| {
                let mut contender_total = 0_i32;
                let mut autonomous_total = 0_i32;
                let mut wins = 0_u32;
                for seed in 0..seeds {
                    // Run both left/right assignments. This protects the comparison
                    // against any residual asymmetry in a generated map.
                    for strategies in [
                        [Strategy::Autonomous, strategy],
                        [strategy, Strategy::Autonomous],
                    ] {
                        let mut sim = Simulation::with_scenario(
                            seed,
                            Scenario::scaled(drones_per_team, strategies),
                        );
                        while !sim.finished {
                            sim.step();
                        }
                        completed += 1;
                        progress(completed, total);
                        let (contender, autonomous) = if strategies[0] == strategy {
                            (sim.scores[0], sim.scores[1])
                        } else {
                            (sim.scores[1], sim.scores[0])
                        };
                        contender_total += contender as i32;
                        autonomous_total += autonomous as i32;
                        wins += u32::from(contender > autonomous);
                    }
                }
                let rounds = (seeds * 2) as f32;
                (
                    (contender_total - autonomous_total) as f32 / rounds,
                    wins as f32 / rounds * 100.0,
                )
            };
            let (dedicated_delta, dedicated_win_rate) = evaluate(Strategy::DedicatedScout);
            let (hybrid_delta, hybrid_win_rate) = evaluate(Strategy::HybridScout);
            BenchmarkRow {
                drones_per_team,
                dedicated_delta,
                dedicated_win_rate,
                hybrid_delta,
                hybrid_win_rate,
            }
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn distance(self, other: Self) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
    pub fn board_label(self) -> String {
        let mut value = self.x + 1;
        let mut column = String::new();
        while value > 0 {
            let digit = ((value - 1) % 26) as u8;
            column.insert(0, char::from(b'A' + digit));
            value = (value - 1) / 26;
        }
        format!("{column}{}", self.y)
    }
    pub fn neighbors(self) -> [Self; 4] {
        [
            Self::new(self.x + 1, self.y),
            Self::new(self.x - 1, self.y),
            Self::new(self.x, self.y + 1),
            Self::new(self.x, self.y - 1),
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Team {
    Azure,
    Amber,
}

impl Team {
    pub const ALL: [Self; 2] = [Self::Azure, Self::Amber];
    pub const fn index(self) -> usize {
        match self {
            Self::Azure => 0,
            Self::Amber => 1,
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Azure => "AZURE",
            Self::Amber => "AMBER",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Courier,
    Scout,
    Harvester,
}

impl Role {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Courier => "Courier",
            Self::Scout => "Scout",
            Self::Harvester => "Harvester",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Drone {
    pub id: usize,
    pub team: Team,
    pub position: Pos,
    pub cargo: u8,
    pub role: Role,
    pub target: Option<Pos>,
    pub reason: String,
    /// A movement request persists while the drone is waiting for the same cell.
    /// It provides deterministic, starvation-free traffic priority.
    move_request: Option<Pos>,
    request_since: u32,
    blocked_turns: u32,
    last_position: Option<Pos>,
    yield_cooldown: u8,
}

#[derive(Clone, Debug)]
pub struct Crystal {
    pub position: Pos,
    pub amount: u8,
}

/// The only authority a bot has over the arena for a turn. The engine checks
/// adjacency, passability, cargo rules, and simultaneous traffic afterwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    Move(Pos),
    Harvest,
    Deposit,
    Wait,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorldEvent {
    Moved {
        drone_id: usize,
        team: Team,
        from: Pos,
        to: Pos,
    },
    Harvested {
        drone_id: usize,
        team: Team,
        position: Pos,
        amount: u8,
    },
    Deposited {
        drone_id: usize,
        team: Team,
        amount: u8,
    },
}

// Kept temporarily for private legacy helpers below while the renderer-facing
// simulation file is migrated; the active decision path uses `Intent` through
// `Bot::decide` above.
#[allow(dead_code)]
type Action = Intent;

#[derive(Clone, Debug)]
pub struct Decision {
    pub intent: Intent,
    pub role: Role,
    pub target: Option<Pos>,
    pub reason: &'static str,
}

impl Decision {
    pub const fn new(
        intent: Intent,
        role: Role,
        target: Option<Pos>,
        reason: &'static str,
    ) -> Self {
        Self {
            intent,
            role,
            target,
            reason,
        }
    }
}

/// Information available to one bot at decision time. Hidden world state is
/// deliberately absent: a bot sees its team's shared discoveries, not the
/// authoritative terrain, untouched crystals, or opponent positions.
#[derive(Clone, Debug)]
pub struct Observation {
    pub turn: u32,
    pub width: i32,
    pub height: i32,
    pub me: DroneView,
    pub base: Pos,
    pub allies: Vec<DroneView>,
    pub explored: HashSet<Pos>,
    pub known_walls: HashSet<Pos>,
    pub known_crystals: HashMap<Pos, u8>,
}

#[derive(Clone, Debug)]
pub struct DroneView {
    pub id: usize,
    pub team: Team,
    pub position: Pos,
    pub cargo: u8,
    pub target: Option<Pos>,
    pub blocked_turns: u32,
    pub last_position: Option<Pos>,
    pub yield_cooldown: u8,
}

#[derive(Clone, Debug, Default)]
pub struct TeamMemory {
    pub explored: HashSet<Pos>,
    pub known_walls: HashSet<Pos>,
    pub known_crystals: HashMap<Pos, u8>,
}

pub struct Simulation {
    scenario: Scenario,
    turn: u32,
    scores: [u32; 2],
    bases: [Pos; 2],
    walls: HashSet<Pos>,
    drones: Vec<Drone>,
    crystals: Vec<Crystal>,
    memories: [TeamMemory; 2],
    finished: bool,
    last_event: String,
    turn_explanation: String,
    last_world_events: Vec<WorldEvent>,
    bots: Vec<Box<dyn Bot>>,
}

/// Renderer- and transport-friendly copy of the authoritative world state.
/// Consumers should use this instead of depending on `Simulation` internals.
#[derive(Clone, Debug)]
pub struct WorldSnapshot {
    pub scenario: Scenario,
    pub turn: u32,
    pub scores: [u32; 2],
    pub bases: [Pos; 2],
    pub walls: HashSet<Pos>,
    pub drones: Vec<Drone>,
    pub crystals: Vec<Crystal>,
    pub memories: [TeamMemory; 2],
    pub finished: bool,
    pub last_event: String,
    pub turn_explanation: String,
}

#[derive(Clone, Debug)]
pub enum MatchEvent {
    TurnAdvanced {
        turn: u32,
        explanation: String,
        event: String,
        world_events: Vec<WorldEvent>,
    },
    MatchFinished {
        turn: u32,
        scores: [u32; 2],
    },
}

impl Simulation {
    pub fn new(seed: u64) -> Self {
        Self::with_scenario(seed, Scenario::default())
    }

    pub fn snapshot(&self) -> WorldSnapshot {
        WorldSnapshot {
            scenario: self.scenario,
            turn: self.turn,
            scores: self.scores,
            bases: self.bases,
            walls: self.walls.clone(),
            drones: self.drones.clone(),
            crystals: self.crystals.clone(),
            memories: self.memories.clone(),
            finished: self.finished,
            last_event: self.last_event.clone(),
            turn_explanation: self.turn_explanation.clone(),
        }
    }

    pub fn scenario(&self) -> Scenario {
        self.scenario
    }
    pub fn turn(&self) -> u32 {
        self.turn
    }
    pub fn scores(&self) -> [u32; 2] {
        self.scores
    }
    pub fn is_finished(&self) -> bool {
        self.finished
    }
    pub fn last_event(&self) -> &str {
        &self.last_event
    }
    pub fn turn_explanation(&self) -> &str {
        &self.turn_explanation
    }
    pub fn last_world_events(&self) -> &[WorldEvent] {
        &self.last_world_events
    }
    pub fn bot_slots(&self) -> Vec<(Team, usize)> {
        self.drones
            .iter()
            .map(|drone| (drone.team, drone.id))
            .collect()
    }
    pub fn refresh_observations(&mut self) {
        self.observe();
    }

    pub fn with_scenario(seed: u64, scenario: Scenario) -> Self {
        let strategies = scenario.strategies;
        Self::with_bot_factory(seed, scenario, move |team, _id| {
            Box::new(BaselineBot::new(strategies[team.index()]))
        })
    }

    /// Creates a world without registering any Bot instances.
    ///
    /// This is the preferred boundary for runners that own Bot scheduling.
    pub fn without_bots(seed: u64, scenario: Scenario) -> Self {
        Self::build_world(seed, scenario)
    }

    /// Creates a match with one isolated bot instance per drone. This is the
    /// compatibility entry point for callers that want the core convenience
    /// `step()` loop to own Bot scheduling.
    pub fn with_bot_factory<F>(seed: u64, scenario: Scenario, mut factory: F) -> Self
    where
        F: FnMut(Team, usize) -> Box<dyn Bot>,
    {
        let mut sim = Self::build_world(seed, scenario);
        sim.bots = sim
            .drones
            .iter()
            .map(|drone| factory(drone.team, drone.id))
            .collect();
        sim
    }

    fn build_world(seed: u64, scenario: Scenario) -> Self {
        let bases = [
            Pos::new(1, scenario.height / 2),
            Pos::new(scenario.width - 2, scenario.height / 2),
        ];
        let mut rng = Lcg::new(seed);
        let mut walls = HashSet::new();

        // Mirrored cloud gaps make each seed fair while keeping every match distinct.
        for x in 3..scenario.width / 2 {
            for y in 1..scenario.height - 1 {
                if rng.chance(scenario.wall_chance_percent.into()) {
                    let a = Pos::new(x, y);
                    let b = Pos::new(scenario.width - 1 - x, scenario.height - 1 - y);
                    if a.distance(bases[0]) > scenario.drones_per_team as i32 / 2 + 3
                        && b.distance(bases[1]) > scenario.drones_per_team as i32 / 2 + 3
                    {
                        walls.insert(a);
                        walls.insert(b);
                    }
                }
            }
        }

        // Keep a broad central shipping lane open.
        walls.retain(|p| p.y != scenario.height / 2 && p.y != scenario.height / 2 - 1);

        let mut crystals = Vec::new();
        let mut occupied = walls.clone();
        occupied.extend(bases);
        // Tutorial pair: each fleet can see a nearby source immediately, so
        // the first match demonstrates the harvest → return → deposit loop.
        let tutorial_left = Pos::new(4.min(scenario.width / 2 - 2), scenario.height / 2);
        let tutorial_right = Pos::new(
            scenario.width - 1 - tutorial_left.x,
            scenario.height - 1 - tutorial_left.y,
        );
        crystals.push(Crystal {
            position: tutorial_left,
            amount: 10,
        });
        crystals.push(Crystal {
            position: tutorial_right,
            amount: 10,
        });
        occupied.insert(tutorial_left);
        occupied.insert(tutorial_right);
        let paired_sites = scenario.crystal_sites.saturating_sub(1).max(4) & !1;
        while crystals.len() < paired_sites {
            let p = Pos::new(
                rng.range(4, scenario.width / 2),
                rng.range(2, scenario.height - 2),
            );
            let mirror = Pos::new(scenario.width - 1 - p.x, scenario.height - 1 - p.y);
            if !occupied.contains(&p) && !occupied.contains(&mirror) {
                let amount = 6 + rng.range(0, 5) as u8;
                crystals.push(Crystal {
                    position: p,
                    amount,
                });
                crystals.push(Crystal {
                    position: mirror,
                    amount,
                });
                occupied.insert(p);
                occupied.insert(mirror);
            }
        }

        // A rich neutral objective creates interaction in the middle.
        let center = Pos::new(scenario.width / 2 - 1, scenario.height / 2);
        crystals.push(Crystal {
            position: center,
            amount: 16,
        });

        let mut drones = Vec::new();
        for team in Team::ALL {
            for slot in 0..scenario.drones_per_team {
                let offset = slot as i32 - scenario.drones_per_team as i32 / 2;
                let base = bases[team.index()];
                drones.push(Drone {
                    id: slot,
                    team,
                    position: Pos::new(base.x, base.y + offset),
                    cargo: 0,
                    role: Role::Courier,
                    target: None,
                    reason: "Booting flight plan".into(),
                    move_request: None,
                    request_since: 0,
                    blocked_turns: 0,
                    last_position: None,
                    yield_cooldown: 0,
                });
            }
        }

        let mut sim = Self {
            scenario,
            turn: 0,
            scores: [0, 0],
            bases,
            walls,
            drones,
            crystals,
            memories: [TeamMemory::default(), TeamMemory::default()],
            finished: false,
            last_event: "Telemetry online — both fleets launched".into(),
            turn_explanation: "Both fleets are choosing their first assignments.".into(),
            bots: Vec::new(),
            last_world_events: Vec::new(),
        };
        sim.observe();
        sim
    }

    /// Collects one decision per drone from the currently registered bots.
    ///
    /// This is deliberately separate from [`Self::resolve_tick`], so a runner
    /// can provide decisions from another process, thread, or WASM host while
    /// keeping world-rule resolution in the core.
    pub fn collect_decisions(&mut self) -> Vec<Decision> {
        if self.finished || self.bots.len() != self.drones.len() {
            return Vec::new();
        }
        self.observe();
        let decisions: Vec<Decision> = (0..self.drones.len())
            .map(|index| {
                let observation = self.observation_for(index);
                self.bots[index].decide(&observation)
            })
            .collect();
        self.record_decisions(&decisions);
        decisions
    }

    /// Records decision metadata for rendering and diagnostics without
    /// applying any intent to the world.
    pub fn record_decisions(&mut self, decisions: &[Decision]) {
        assert_eq!(
            decisions.len(),
            self.drones.len(),
            "one decision is required for every drone"
        );
        for (index, decision) in decisions.iter().enumerate() {
            self.drones[index].role = decision.role;
            self.drones[index].target = decision.target;
            self.drones[index].reason = decision.reason.into();
        }
        self.turn_explanation = self
            .drones
            .iter()
            .map(|drone| {
                let target = drone.target.map_or("—".to_string(), |p| p.board_label());
                format!(
                    "{}{} {} → {}",
                    if drone.team == Team::Azure { "A" } else { "B" },
                    drone.id + 1,
                    drone.reason,
                    target
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    /// Resolves a complete set of intents as one atomic world tick.
    ///
    /// The caller must provide exactly one decision for every drone, in the
    /// same order as [`Self::drones`]. All movement conflicts are resolved
    /// before any harvest or deposit side effects are applied.
    pub fn resolve_tick(&mut self, decisions: &[Decision]) {
        if self.finished {
            return;
        }
        assert_eq!(
            decisions.len(),
            self.drones.len(),
            "one decision is required for every drone"
        );
        self.last_world_events.clear();

        let planned_moves: Vec<Option<Pos>> = decisions
            .iter()
            .enumerate()
            .map(|(index, decision)| match decision.intent {
                Intent::Move(next)
                    if self.drones[index].position.distance(next) == 1 && self.passable(next) =>
                {
                    Some(next)
                }
                _ => None,
            })
            .collect();
        // Register persistent requests before selecting winners. A request keeps
        // its original timestamp until it moves or changes destination: first
        // request wins; requests made in the same turn use the stable drone ID.
        for (index, request) in planned_moves.iter().enumerate() {
            match request {
                Some(next) if self.drones[index].move_request == Some(*next) => {}
                Some(next) => {
                    self.drones[index].move_request = Some(*next);
                    self.drones[index].request_since = self.turn;
                }
                None => {
                    self.drones[index].move_request = None;
                    self.drones[index].blocked_turns = 0;
                }
            }
        }

        // Each cell grants exactly one reservation. This is free negotiation,
        // not a fixed team schedule: priority belongs to the oldest request.
        let mut winner_for_destination: HashMap<Pos, usize> = HashMap::new();
        for (index, next) in planned_moves.iter().enumerate() {
            let Some(next) = next else { continue };
            winner_for_destination
                .entry(*next)
                .and_modify(|winner| {
                    let candidate_key = self.traffic_priority(index);
                    let winner_key = self.traffic_priority(*winner);
                    if candidate_key < winner_key {
                        *winner = index;
                    }
                })
                .or_insert(index);
        }
        let selected: Vec<bool> = planned_moves
            .iter()
            .enumerate()
            .map(|(index, next)| {
                next.is_some_and(|p| winner_for_destination.get(&p) == Some(&index))
            })
            .collect();
        let occupants: HashMap<Pos, usize> = self
            .drones
            .iter()
            .enumerate()
            .map(|(index, drone)| (drone.position, index))
            .collect();
        let mut resolution = vec![0_u8; self.drones.len()]; // 0 unknown, 1 visiting, 2 yes, 3 no
        let executable: Vec<bool> = (0..self.drones.len())
            .map(|index| {
                Self::can_execute_move(
                    index,
                    &planned_moves,
                    &selected,
                    &occupants,
                    &mut resolution,
                )
            })
            .collect();

        for (index, decision) in decisions.iter().enumerate() {
            match decision.intent {
                Intent::Move(next) if executable[index] => {
                    let previous = self.drones[index].position;
                    self.drones[index].position = next;
                    self.drones[index].move_request = None;
                    self.drones[index].blocked_turns = 0;
                    self.drones[index].last_position = Some(previous);
                    self.drones[index].yield_cooldown =
                        self.drones[index].yield_cooldown.saturating_sub(1);
                    self.last_world_events.push(WorldEvent::Moved {
                        drone_id: self.drones[index].id,
                        team: self.drones[index].team,
                        from: previous,
                        to: next,
                    });
                }
                Intent::Move(_) => {
                    self.drones[index].blocked_turns += 1;
                    if self.drones[index].blocked_turns >= 2 {
                        self.drones[index].yield_cooldown = 3;
                    }
                }
                Intent::Harvest => {
                    if self.drones[index].cargo < CARGO_CAPACITY {
                        if let Some(crystal) = self
                            .crystals
                            .iter_mut()
                            .find(|c| c.position == self.drones[index].position && c.amount > 0)
                        {
                            crystal.amount -= 1;
                            self.drones[index].cargo += 1;
                            self.last_world_events.push(WorldEvent::Harvested {
                                drone_id: self.drones[index].id,
                                team: self.drones[index].team,
                                position: self.drones[index].position,
                                amount: 1,
                            });
                            self.last_event = format!(
                                "{}-{} harvested a sky crystal",
                                self.drones[index].team.label(),
                                self.drones[index].id + 1
                            );
                        }
                    }
                }
                Intent::Deposit => {
                    let drone = &mut self.drones[index];
                    if drone.position == self.bases[drone.team.index()] && drone.cargo > 0 {
                        let delivered = drone.cargo as u32;
                        self.scores[drone.team.index()] += delivered;
                        drone.cargo = 0;
                        self.last_world_events.push(WorldEvent::Deposited {
                            drone_id: drone.id,
                            team: drone.team,
                            amount: delivered as u8,
                        });
                        self.last_event = format!(
                            "{}-{} delivered {delivered} energy",
                            drone.team.label(),
                            drone.id + 1
                        );
                    }
                }
                _ => {}
            }
        }

        self.turn += 1;
        self.observe();
        self.finished = self.turn >= self.scenario.max_turns
            || self.crystals.iter().all(|c| c.amount == 0)
                && self.drones.iter().all(|d| d.cargo == 0);
        if self.finished {
            self.last_event = match self.scores[0].cmp(&self.scores[1]) {
                std::cmp::Ordering::Greater => "Match complete — Azure wins".into(),
                std::cmp::Ordering::Less => "Match complete — Amber wins".into(),
                std::cmp::Ordering::Equal => "Match complete — draw".into(),
            };
        }
    }

    /// Runs the built-in bot loop and resolves the resulting intents.
    ///
    /// Kept as a convenience API for headless callers and existing clients;
    /// runners that own Bot scheduling should call the two phases explicitly.
    pub fn step(&mut self) {
        let decisions = self.collect_decisions();
        if decisions.len() == self.drones.len() {
            self.resolve_tick(&decisions);
        }
    }

    fn drone_order(&self, index: usize) -> usize {
        self.drones[index].team.index() * self.scenario.drones_per_team + self.drones[index].id
    }

    fn traffic_priority(&self, index: usize) -> (u8, u32, usize) {
        let drone = &self.drones[index];
        let returning_home = drone.target == Some(self.bases[drone.team.index()]);
        let final_delivery = returning_home
            && drone.cargo > 0
            && self.crystals.iter().all(|crystal| crystal.amount == 0);
        // Normal traffic remains first-request-first. The only override is the
        // final undelivered cargo: letting idle units trap it can turn a won
        // match into a timeout draw.
        let class = if final_delivery { 0 } else { 1 };
        (class, drone.request_since, self.drone_order(index))
    }

    /// A selected move into an occupied cell executes only if the current
    /// occupant also has a selected, executable departure. Closed rotations are
    /// safe simultaneous moves; an interrupted chain is denied as a whole.
    fn can_execute_move(
        index: usize,
        planned_moves: &[Option<Pos>],
        selected: &[bool],
        occupants: &HashMap<Pos, usize>,
        resolution: &mut [u8],
    ) -> bool {
        match resolution[index] {
            2 => return true,
            3 => return false,
            1 => return true,
            _ => {}
        }
        if !selected[index] {
            resolution[index] = 3;
            return false;
        }
        resolution[index] = 1;
        let next = planned_moves[index].expect("selected moves have a destination");
        let allowed = occupants.get(&next).is_none_or(|occupant| {
            Self::can_execute_move(*occupant, planned_moves, selected, occupants, resolution)
        });
        resolution[index] = if allowed { 2 } else { 3 };
        allowed
    }

    fn observe(&mut self) {
        for team in Team::ALL {
            let memory = &mut self.memories[team.index()];
            for drone in self.drones.iter().filter(|d| d.team == team) {
                for x in 0..self.scenario.width {
                    for y in 0..self.scenario.height {
                        let p = Pos::new(x, y);
                        if drone.position.distance(p) <= SENSOR_RANGE {
                            memory.explored.insert(p);
                            if self.walls.contains(&p) {
                                memory.known_walls.insert(p);
                            }
                        }
                    }
                }
            }
            for crystal in &self.crystals {
                if memory.explored.contains(&crystal.position) {
                    memory
                        .known_crystals
                        .insert(crystal.position, crystal.amount);
                }
            }
        }
    }

    /// Produces the complete information boundary for one bot. Keeping this
    /// constructor in the engine makes accidental access to hidden state easy
    /// to audit and keeps all third-party bots on equal footing.
    pub fn observation_for(&self, index: usize) -> Observation {
        let drone = &self.drones[index];
        let team = drone.team;
        let memory = &self.memories[team.index()];
        let as_view = |drone: &Drone| DroneView {
            id: drone.id,
            team: drone.team,
            position: drone.position,
            cargo: drone.cargo,
            target: drone.target,
            blocked_turns: drone.blocked_turns,
            last_position: drone.last_position,
            yield_cooldown: drone.yield_cooldown,
        };
        Observation {
            turn: self.turn,
            width: self.scenario.width,
            height: self.scenario.height,
            me: as_view(drone),
            base: self.bases[team.index()],
            allies: self
                .drones
                .iter()
                .filter(|other| other.team == team)
                .map(as_view)
                .collect(),
            explored: memory.explored.clone(),
            known_walls: memory.known_walls.clone(),
            known_crystals: memory.known_crystals.clone(),
        }
    }

    #[allow(dead_code)]
    fn decide(&mut self, index: usize) -> Action {
        let drone = self.drones[index].clone();
        let team = drone.team;
        let base = self.bases[team.index()];

        if drone.position == base && drone.cargo > 0 {
            self.set_intent(index, Role::Courier, Some(base), "Unloading at home dock");
            return Action::Deposit;
        }
        if drone.cargo == CARGO_CAPACITY {
            self.set_intent(
                index,
                Role::Courier,
                Some(base),
                "Cargo full — returning home",
            );
            return self.move_toward_for_drone(index, base);
        }
        if self
            .crystals
            .iter()
            .any(|c| c.position == drone.position && c.amount > 0)
        {
            self.set_intent(
                index,
                Role::Harvester,
                Some(drone.position),
                "Extracting visible crystal",
            );
            return Action::Harvest;
        }

        let known: Vec<Pos> = self.memories[team.index()]
            .known_crystals
            .iter()
            .filter_map(|(p, amount)| (*amount > 0).then_some(*p))
            .collect();
        let mut known = known;
        known.sort_by_key(|p| (p.x, p.y));

        // Azure is a straightforward greedy baseline. Amber's first drone is a
        // hybrid scout: on a three-drone fleet, a permanently dedicated scout
        // spends too much of the team's carrying capacity. It scouts only until
        // the team has enough choices, then joins the logistics loop.
        let scouting_coverage_limit = (self.scenario.width * self.scenario.height) as usize
            * SCOUT_COVERAGE_LIMIT_PERCENT
            / 100;
        let needs_more_intel = known.len() < SCOUT_SOURCE_GOAL
            && self.memories[team.index()].explored.len() < scouting_coverage_limit;
        let strategy = self.scenario.strategies[team.index()];
        let should_scout = match strategy {
            Strategy::Autonomous => false,
            Strategy::DedicatedScout => {
                drone.id == 0
                    && self.memories[team.index()].explored.len()
                        < (self.scenario.width * self.scenario.height) as usize
            }
            Strategy::HybridScout => drone.id == 0 && needs_more_intel,
        };
        if should_scout {
            let target = drone
                .target
                .filter(|target| !self.memories[team.index()].explored.contains(target))
                .or_else(|| self.frontier_target(team, drone.position, drone.id));
            if let Some(target) = target {
                let reason = if strategy == Strategy::DedicatedScout {
                    "Dedicated scout charting the world"
                } else {
                    "Scouting until two sources are known"
                };
                self.set_intent(index, Role::Scout, Some(target), reason);
                return self.move_toward_for_drone(index, target);
            }
        }

        if !known.is_empty() {
            let target =
                if let Some(existing) = drone.target.filter(|target| known.contains(target)) {
                    existing
                } else {
                    // Greedy baseline, but with a small coordination rule: assign
                    // different known sources to different couriers. Without this,
                    // all three blue drones queue behind the same crystal and can
                    // spend the entire match blocking one another.
                    let mut sorted = known;
                    sorted.sort_by_key(|p| (drone.position.distance(*p), p.x, p.y));
                    sorted[drone.id % sorted.len()]
                };
            self.set_intent(
                index,
                Role::Courier,
                Some(target),
                if strategy == Strategy::Autonomous {
                    "Heading to nearest known resource"
                } else {
                    "Assigned to a team resource"
                },
            );
            let action = self.move_toward_for_drone(index, target);
            if !matches!(action, Action::Wait) || drone.position == target {
                return action;
            }
            // Drop an unreachable remembered target immediately and resume exploration.
            if let Some(fallback) = self.frontier_target(team, drone.position, drone.id) {
                self.set_intent(
                    index,
                    Role::Scout,
                    Some(fallback),
                    "Resource route blocked — rerouting",
                );
                return self.move_toward_for_drone(index, fallback);
            }
            return Action::Wait;
        }

        if let Some(target) = self.frontier_target(team, drone.position, drone.id) {
            self.set_intent(
                index,
                Role::Scout,
                Some(target),
                "Searching for energy signatures",
            );
            self.move_toward_for_drone(index, target)
        } else if drone.cargo > 0 {
            self.set_intent(
                index,
                Role::Courier,
                Some(base),
                "No resources remain — banking cargo",
            );
            self.move_toward_for_drone(index, base)
        } else {
            let parking = self.parking_spot(team, drone.id);
            if drone.position != parking {
                self.set_intent(
                    index,
                    Role::Courier,
                    Some(parking),
                    "No task — clearing the shipping lane",
                );
                self.move_toward_for_drone(index, parking)
            } else {
                self.set_intent(
                    index,
                    Role::Courier,
                    Some(parking),
                    "Parked clear of the shipping lane",
                );
                Action::Wait
            }
        }
    }

    #[allow(dead_code)]
    fn set_intent(&mut self, index: usize, role: Role, target: Option<Pos>, reason: &'static str) {
        self.drones[index].role = role;
        self.drones[index].target = target;
        self.drones[index].reason = reason.into();
    }

    #[allow(dead_code)]
    fn frontier_target(&self, team: Team, from: Pos, slot: usize) -> Option<Pos> {
        let explored = &self.memories[team.index()].explored;
        let mut candidates: Vec<Pos> = (0..self.scenario.width)
            .flat_map(|x| (0..self.scenario.height).map(move |y| Pos::new(x, y)))
            .filter(|p| !self.walls.contains(p) && !explored.contains(p))
            .collect();
        candidates.sort_by_key(|p| {
            (
                from.distance(*p) + ((p.x + p.y + slot as i32 * 3).rem_euclid(7)),
                p.x,
                p.y,
            )
        });
        candidates
            .into_iter()
            .find(|candidate| !matches!(self.move_toward(from, *candidate), Action::Wait))
    }

    #[allow(dead_code)]
    fn move_toward(&self, from: Pos, target: Pos) -> Action {
        if from == target {
            return Action::Wait;
        }
        let mut queue = VecDeque::from([from]);
        let mut previous = HashMap::new();
        previous.insert(from, from);
        while let Some(current) = queue.pop_front() {
            if current == target {
                break;
            }
            for next in current.neighbors() {
                if self.passable(next) && !previous.contains_key(&next) {
                    previous.insert(next, current);
                    queue.push_back(next);
                }
            }
        }
        if !previous.contains_key(&target) {
            return Action::Wait;
        }
        let mut step = target;
        while previous[&step] != from {
            step = previous[&step];
        }
        Action::Move(step)
    }

    /// After losing two reservations, a drone yields into a free side lane if
    /// one still has a route to its goal. This turns a face-to-face queue into
    /// a deliberate detour instead of an infinite left/right dance.
    #[allow(dead_code)]
    fn move_toward_for_drone(&self, index: usize, target: Pos) -> Action {
        let drone = &self.drones[index];
        let direct = self.move_toward(drone.position, target);
        if drone.blocked_turns < 2 && drone.yield_cooldown == 0 {
            return direct;
        }

        let occupied: HashSet<Pos> = self.drones.iter().map(|other| other.position).collect();
        let mut alternatives: Vec<(i32, Pos)> = drone
            .position
            .neighbors()
            .into_iter()
            .filter(|candidate| self.passable(*candidate) && !occupied.contains(candidate))
            .filter(|candidate| {
                drone.yield_cooldown == 0 || Some(*candidate) != drone.last_position
            })
            .filter_map(|candidate| {
                self.route_distance(candidate, target)
                    .map(|distance| (distance, candidate))
            })
            .collect();
        alternatives.sort_by_key(|(distance, candidate)| (*distance, candidate.x, candidate.y));
        alternatives
            .first()
            .map_or(direct, |(_, candidate)| Action::Move(*candidate))
    }

    #[allow(dead_code)]
    fn route_distance(&self, from: Pos, target: Pos) -> Option<i32> {
        let mut queue = VecDeque::from([(from, 0)]);
        let mut visited = HashSet::from([from]);
        while let Some((current, distance)) = queue.pop_front() {
            if current == target {
                return Some(distance);
            }
            for next in current.neighbors() {
                if self.passable(next) && visited.insert(next) {
                    queue.push_back((next, distance + 1));
                }
            }
        }
        None
    }

    #[allow(dead_code)]
    fn parking_spot(&self, team: Team, id: usize) -> Pos {
        let rows = (self.scenario.height - 2).max(1) as usize;
        let slot = Pos::new(2 + (id / rows) as i32, 1 + (id % rows) as i32);
        match team {
            Team::Azure => slot,
            Team::Amber => Pos::new(
                self.scenario.width - 1 - slot.x,
                self.scenario.height - 1 - slot.y,
            ),
        }
    }

    fn passable(&self, p: Pos) -> bool {
        p.x >= 0
            && p.x < self.scenario.width
            && p.y >= 0
            && p.y < self.scenario.height
            && !self.walls.contains(&p)
    }
}

struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn range(&mut self, min: i32, max: i32) -> i32 {
        min + (self.next() % (max - min) as u64) as i32
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_is_deterministic_and_symmetric() {
        let a = Simulation::new(42);
        let b = Simulation::new(42);
        assert_eq!(a.walls, b.walls);
        for wall in &a.walls {
            assert!(
                a.walls
                    .contains(&Pos::new(WIDTH - 1 - wall.x, HEIGHT - 1 - wall.y))
            );
        }
    }

    #[test]
    fn match_finishes_and_delivers_energy() {
        let mut sim = Simulation::new(7);
        while !sim.finished {
            sim.step();
        }
        assert!(sim.turn <= MAX_TURNS);
        assert!(sim.scores[0] + sim.scores[1] > 0);
    }

    #[test]
    fn same_seed_produces_same_result() {
        let mut a = Simulation::new(99);
        let mut b = Simulation::new(99);
        while !a.finished {
            a.step();
        }
        while !b.finished {
            b.step();
        }
        assert_eq!(a.scores, b.scores);
        assert_eq!(a.turn, b.turn);
    }

    #[test]
    fn explicit_decision_and_resolution_phases_match_step() {
        let mut convenience = Simulation::new(123);
        let mut explicit = Simulation::new(123);
        for _ in 0..12 {
            convenience.step();
            let decisions = explicit.collect_decisions();
            explicit.resolve_tick(&decisions);
        }
        assert_eq!(convenience.turn, explicit.turn);
        assert_eq!(convenience.scores, explicit.scores);
        assert_eq!(
            convenience
                .drones
                .iter()
                .map(|drone| (drone.position, drone.cargo))
                .collect::<Vec<_>>(),
            explicit
                .drones
                .iter()
                .map(|drone| (drone.position, drone.cargo))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            convenience
                .crystals
                .iter()
                .map(|crystal| (crystal.position, crystal.amount))
                .collect::<Vec<_>>(),
            explicit
                .crystals
                .iter()
                .map(|crystal| (crystal.position, crystal.amount))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn demo_seed_produces_visible_score() {
        let mut sim = Simulation::new(42);
        while !sim.finished {
            sim.step();
        }
        assert!(
            sim.scores[0] + sim.scores[1] > 0,
            "the default demo seed should show at least one delivery"
        );
        assert!(
            sim.scores[Team::Amber.index()] > 0,
            "the hybrid Amber strategy should contribute a delivery on the demo map"
        );
    }

    #[test]
    fn many_maps_finish_without_stagnating_or_losing_determinism() {
        for seed in 0..32 {
            let mut first = Simulation::new(seed);
            let mut second = Simulation::new(seed);
            let mut unchanged_turns = 0;
            while !first.finished {
                let before: Vec<Pos> = first.drones.iter().map(|drone| drone.position).collect();
                first.step();
                let after: Vec<Pos> = first.drones.iter().map(|drone| drone.position).collect();
                let unique_positions: HashSet<Pos> = after.iter().copied().collect();
                assert_eq!(
                    unique_positions.len(),
                    after.len(),
                    "seed {seed} allowed drones to overlap at turn {}",
                    first.turn
                );
                unchanged_turns = if before == after {
                    unchanged_turns + 1
                } else {
                    0
                };
                assert!(
                    unchanged_turns < 20,
                    "seed {seed} stagnated at turn {}",
                    first.turn
                );
            }
            while !second.finished {
                second.step();
            }
            assert_eq!(
                first.scores, second.scores,
                "seed {seed} was not deterministic"
            );
            assert!(first.turn <= MAX_TURNS, "seed {seed} exceeded match limit");
            assert!(
                !(first.turn == MAX_TURNS
                    && first.crystals.iter().all(|crystal| crystal.amount == 0)
                    && first.drones.iter().any(|drone| drone.cargo > 0)),
                "seed {seed} timed out while final cargo was still waiting to be delivered"
            );
        }
    }

    #[test]
    fn benchmark_scales_scenarios_and_returns_all_strategies() {
        let rows = benchmark_leadership(3, 1);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].drones_per_team, 2);
        assert_eq!(rows[1].drones_per_team, 3);
        assert!(rows.iter().all(|row| row.dedicated_win_rate >= 0.0
            && row.dedicated_win_rate <= 100.0
            && row.hybrid_win_rate >= 0.0
            && row.hybrid_win_rate <= 100.0));
    }

    struct IllegalMoveBot;
    impl Bot for IllegalMoveBot {
        fn decide(&mut self, _observation: &Observation) -> Decision {
            // A bot may ask, but cannot teleport: the engine is the referee.
            Decision::new(
                Intent::Move(Pos::new(99, 99)),
                Role::Scout,
                None,
                "Testing referee",
            )
        }
    }

    #[test]
    fn external_bot_only_submits_intents_and_engine_rejects_teleporting() {
        let mut sim =
            Simulation::with_bot_factory(3, Scenario::default(), |_, _| Box::new(IllegalMoveBot));
        let before: Vec<Pos> = sim.drones.iter().map(|drone| drone.position).collect();
        sim.step();
        let after: Vec<Pos> = sim.drones.iter().map(|drone| drone.position).collect();
        assert_eq!(
            before, after,
            "the referee must reject a non-adjacent bot move"
        );
        let view = sim.observation_for(0);
        assert!(!view.known_walls.is_empty() || !sim.walls.is_empty());
        assert!(view.known_crystals.len() <= sim.crystals.len());
    }
}
