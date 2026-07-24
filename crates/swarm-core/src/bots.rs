//! Baseline bots shipped with the arena.
//!
//! They deliberately use only `Observation`, exactly as an external entrant
//! would. The simulation never gives a bot its hidden map, enemy state, or an
//! authority to move other drones.

use crate::{CARGO_CAPACITY, Decision, Intent, Observation, Pos, Role, Strategy, Team};
use std::collections::{HashMap, HashSet, VecDeque};

const SCOUT_SOURCE_GOAL: usize = 2;
const SCOUT_COVERAGE_LIMIT_PERCENT: usize = 35;

/// Persistent per-drone memory owned by the runner, not authoritative world state.
#[derive(Clone, Debug, Default)]
pub struct AgentMemory {
    pub assigned_target: Option<Pos>,
    pub route_failures: u32,
    pub ticks: u64,
}

#[derive(Clone, Debug, Default)]
pub struct MemoryPatch {
    pub assigned_target: Option<Option<Pos>>,
    pub route_failures: Option<u32>,
}

impl AgentMemory {
    pub fn apply(&mut self, patch: MemoryPatch) {
        if let Some(target) = patch.assigned_target {
            self.assigned_target = target;
        }
        if let Some(failures) = patch.route_failures {
            self.route_failures = failures;
        }
    }
}

/// Deterministic logical budget for a Bot turn. Callers can use it to bound
/// path searches without relying on platform-dependent wall-clock timing.
#[derive(Clone, Debug)]
pub struct TickBudget {
    remaining_path_nodes: usize,
}

impl TickBudget {
    pub fn new(path_nodes: usize) -> Self {
        Self {
            remaining_path_nodes: path_nodes,
        }
    }

    pub fn consume_path_nodes(&mut self, amount: usize) -> bool {
        if amount > self.remaining_path_nodes {
            self.remaining_path_nodes = 0;
            false
        } else {
            self.remaining_path_nodes -= amount;
            true
        }
    }

    pub fn exhausted(&self) -> bool {
        self.remaining_path_nodes == 0
    }
}

pub struct BotOutput {
    pub decision: Decision,
    pub memory: MemoryPatch,
}

impl BotOutput {
    pub fn new(decision: Decision) -> Self {
        Self {
            decision,
            memory: MemoryPatch::default(),
        }
    }
}

/// The extension point for a Swarm Space entrant.
///
/// A bot instance controls exactly one drone. It receives a copied, bounded
/// observation and submits one intent; the arena validates and resolves that
/// intent together with all other drones' intents.
pub trait Bot: Send + Sync {
    fn decide(&mut self, observation: &Observation) -> Decision;

    fn decide_with_context(
        &mut self,
        observation: &Observation,
        _memory: &AgentMemory,
        _budget: &mut TickBudget,
    ) -> BotOutput {
        BotOutput::new(self.decide(observation))
    }
}

/// The three built-in strategies are now ordinary bots, not special engine
/// branches. They are useful baselines for a future user supplied bot.
pub struct BaselineBot {
    strategy: Strategy,
}

impl BaselineBot {
    pub const fn new(strategy: Strategy) -> Self {
        Self { strategy }
    }
}

impl Bot for BaselineBot {
    fn decide(&mut self, view: &Observation) -> Decision {
        let drone = &view.me;
        if drone.position == view.base && drone.cargo > 0 {
            return Decision::new(
                Intent::Deposit,
                Role::Courier,
                Some(view.base),
                "Unloading at home dock",
            );
        }
        if drone.cargo == CARGO_CAPACITY {
            return Decision::new(
                move_toward(view, view.base),
                Role::Courier,
                Some(view.base),
                "Cargo full — returning home",
            );
        }
        if view
            .known_crystals
            .get(&drone.position)
            .copied()
            .unwrap_or(0)
            > 0
        {
            return Decision::new(
                Intent::Harvest,
                Role::Harvester,
                Some(drone.position),
                "Extracting visible crystal",
            );
        }

        let mut known: Vec<Pos> = view
            .known_crystals
            .iter()
            .filter_map(|(p, amount)| (*amount > 0).then_some(*p))
            .collect();
        known.sort_by_key(|p| (p.x, p.y));
        let coverage_limit =
            (view.width * view.height) as usize * SCOUT_COVERAGE_LIMIT_PERCENT / 100;
        let needs_more_intel =
            known.len() < SCOUT_SOURCE_GOAL && view.explored.len() < coverage_limit;
        let should_scout = match self.strategy {
            Strategy::Autonomous => false,
            Strategy::DedicatedScout => {
                drone.id == 0 && view.explored.len() < (view.width * view.height) as usize
            }
            Strategy::HybridScout => drone.id == 0 && needs_more_intel,
        };
        if should_scout {
            let target = drone
                .target
                .filter(|target| !view.explored.contains(target))
                .or_else(|| frontier_target(view, drone.position, drone.id));
            if let Some(target) = target {
                let reason = if self.strategy == Strategy::DedicatedScout {
                    "Dedicated scout charting the world"
                } else {
                    "Scouting until two sources are known"
                };
                return Decision::new(move_toward(view, target), Role::Scout, Some(target), reason);
            }
        }

        if !known.is_empty() {
            // A remembered resource can be temporarily unreachable because a
            // newly discovered wall or a traffic knot invalidated the old
            // route. Do not keep asking the referee for the same move forever.
            if drone.blocked_turns >= 4 {
                if let Some(fallback) = frontier_target(
                    view,
                    drone.position,
                    drone.id + drone.blocked_turns as usize,
                ) {
                    return Decision::new(
                        move_toward_for_drone(view, fallback),
                        Role::Scout,
                        Some(fallback),
                        "Route stalled — scouting a fresh approach",
                    );
                }
            }
            let target =
                if let Some(existing) = drone.target.filter(|target| known.contains(target)) {
                    existing
                } else {
                    known.sort_by_key(|p| (drone.position.distance(*p), p.x, p.y));
                    known[drone.id % known.len()]
                };
            let action = move_toward_for_drone(view, target);
            if !matches!(action, Intent::Wait) || drone.position == target {
                let reason = if self.strategy == Strategy::Autonomous {
                    "Heading to nearest known resource"
                } else {
                    "Assigned to a team resource"
                };
                return Decision::new(action, Role::Courier, Some(target), reason);
            }
            if let Some(fallback) = frontier_target(view, drone.position, drone.id) {
                return Decision::new(
                    move_toward_for_drone(view, fallback),
                    Role::Scout,
                    Some(fallback),
                    "Resource route blocked — rerouting",
                );
            }
            return Decision::new(
                Intent::Wait,
                Role::Courier,
                Some(target),
                "Waiting for route to resource",
            );
        }

        if let Some(target) = frontier_target(view, drone.position, drone.id) {
            Decision::new(
                move_toward_for_drone(view, target),
                Role::Scout,
                Some(target),
                "Searching for energy signatures",
            )
        } else if drone.cargo > 0 {
            Decision::new(
                move_toward(view, view.base),
                Role::Courier,
                Some(view.base),
                "No resources remain — banking cargo",
            )
        } else {
            let parking = parking_spot(view, drone.team, drone.id);
            if drone.position != parking {
                Decision::new(
                    move_toward_for_drone(view, parking),
                    Role::Courier,
                    Some(parking),
                    "No task — clearing the shipping lane",
                )
            } else {
                Decision::new(
                    Intent::Wait,
                    Role::Courier,
                    Some(parking),
                    "Parked clear of the shipping lane",
                )
            }
        }
    }
}

fn passable(view: &Observation, p: Pos) -> bool {
    p.x >= 0 && p.x < view.width && p.y >= 0 && p.y < view.height && !view.known_walls.contains(&p)
}

fn move_toward(view: &Observation, target: Pos) -> Intent {
    if view.me.position == target {
        return Intent::Wait;
    }
    let mut queue = VecDeque::from([view.me.position]);
    let mut previous = HashMap::new();
    previous.insert(view.me.position, view.me.position);
    while let Some(current) = queue.pop_front() {
        if current == target {
            break;
        }
        for next in current.neighbors() {
            if passable(view, next) && !previous.contains_key(&next) {
                previous.insert(next, current);
                queue.push_back(next);
            }
        }
    }
    if !previous.contains_key(&target) {
        return Intent::Wait;
    }
    let mut step = target;
    while previous[&step] != view.me.position {
        step = previous[&step];
    }
    Intent::Move(step)
}

fn route_distance(view: &Observation, from: Pos, target: Pos) -> Option<i32> {
    let mut queue = VecDeque::from([(from, 0)]);
    let mut visited = HashSet::from([from]);
    while let Some((current, distance)) = queue.pop_front() {
        if current == target {
            return Some(distance);
        }
        for next in current.neighbors() {
            if passable(view, next) && visited.insert(next) {
                queue.push_back((next, distance + 1));
            }
        }
    }
    None
}

fn move_toward_for_drone(view: &Observation, target: Pos) -> Intent {
    let drone = &view.me;
    let direct = move_toward(view, target);
    if drone.blocked_turns < 2 && drone.yield_cooldown == 0 {
        return direct;
    }
    let occupied: HashSet<Pos> = view.allies.iter().map(|other| other.position).collect();
    let mut alternatives: Vec<(i32, Pos)> = drone
        .position
        .neighbors()
        .into_iter()
        .filter(|candidate| passable(view, *candidate) && !occupied.contains(candidate))
        .filter(|candidate| drone.yield_cooldown == 0 || Some(*candidate) != drone.last_position)
        .filter_map(|candidate| {
            route_distance(view, candidate, target).map(|distance| (distance, candidate))
        })
        .collect();
    alternatives.sort_by_key(|(distance, candidate)| (*distance, candidate.x, candidate.y));
    alternatives
        .first()
        .map_or(direct, |(_, candidate)| Intent::Move(*candidate))
}

fn frontier_target(view: &Observation, from: Pos, slot: usize) -> Option<Pos> {
    let mut candidates: Vec<Pos> = (0..view.width)
        .flat_map(|x| (0..view.height).map(move |y| Pos::new(x, y)))
        .filter(|p| passable(view, *p) && !view.explored.contains(p))
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
        .find(|candidate| !matches!(move_toward(view, *candidate), Intent::Wait))
}

fn parking_spot(view: &Observation, team: Team, id: usize) -> Pos {
    let rows = (view.height - 2).max(1) as usize;
    let slot = Pos::new(2 + (id / rows) as i32, 1 + (id % rows) as i32);
    match team {
        Team::Azure => slot,
        Team::Amber => Pos::new(view.width - 1 - slot.x, view.height - 1 - slot.y),
    }
}
