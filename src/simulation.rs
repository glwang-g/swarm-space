use std::collections::{HashMap, HashSet, VecDeque};

pub const WIDTH: i32 = 24;
pub const HEIGHT: i32 = 16;
pub const MAX_TURNS: u32 = 300;
pub const SENSOR_RANGE: i32 = 5;
pub const CARGO_CAPACITY: u8 = 3;
const SCOUT_SOURCE_GOAL: usize = 2;
const SCOUT_COVERAGE_LIMIT_PERCENT: usize = 35;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self { Self { x, y } }
    pub fn distance(self, other: Self) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
    pub fn board_label(self) -> String {
        let column = char::from_u32(u32::from(b'A') + self.x as u32).unwrap_or('?');
        format!("{column}{}", self.y)
    }
    fn neighbors(self) -> [Self; 4] {
        [
            Self::new(self.x + 1, self.y),
            Self::new(self.x - 1, self.y),
            Self::new(self.x, self.y + 1),
            Self::new(self.x, self.y - 1),
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Team { Azure, Amber }

impl Team {
    pub const ALL: [Self; 2] = [Self::Azure, Self::Amber];
    pub const fn index(self) -> usize { match self { Self::Azure => 0, Self::Amber => 1 } }
    pub const fn label(self) -> &'static str { match self { Self::Azure => "AZURE", Self::Amber => "AMBER" } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role { Courier, Scout, Harvester }

impl Role {
    pub const fn label(self) -> &'static str {
        match self { Self::Courier => "Courier", Self::Scout => "Scout", Self::Harvester => "Harvester" }
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
    pub reason: &'static str,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action { Move(Pos), Harvest, Deposit, Wait }

#[derive(Clone, Debug, Default)]
pub struct TeamMemory {
    pub explored: HashSet<Pos>,
    pub known_crystals: HashMap<Pos, u8>,
}

#[derive(Clone, Debug)]
pub struct Simulation {
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

impl Simulation {
    pub fn new(seed: u64) -> Self {
        let bases = [Pos::new(1, HEIGHT / 2), Pos::new(WIDTH - 2, HEIGHT / 2)];
        let mut rng = Lcg::new(seed);
        let mut walls = HashSet::new();

        // Mirrored cloud gaps make each seed fair while keeping every match distinct.
        for x in 3..WIDTH / 2 {
            for y in 1..HEIGHT - 1 {
                if rng.chance(16) {
                    let a = Pos::new(x, y);
                    let b = Pos::new(WIDTH - 1 - x, HEIGHT - 1 - y);
                    if a.distance(bases[0]) > 3 && b.distance(bases[1]) > 3 {
                        walls.insert(a);
                        walls.insert(b);
                    }
                }
            }
        }

        // Keep a broad central shipping lane open.
        walls.retain(|p| p.y != HEIGHT / 2 && p.y != HEIGHT / 2 - 1);

        let mut crystals = Vec::new();
        let mut occupied = walls.clone();
        occupied.extend(bases);
        // Tutorial pair: each fleet can see a nearby source immediately, so
        // the first match demonstrates the harvest → return → deposit loop.
        let tutorial_left = Pos::new(4, HEIGHT / 2);
        let tutorial_right = Pos::new(WIDTH - 1 - tutorial_left.x, HEIGHT - 1 - tutorial_left.y);
        crystals.push(Crystal { position: tutorial_left, amount: 10 });
        crystals.push(Crystal { position: tutorial_right, amount: 10 });
        occupied.insert(tutorial_left);
        occupied.insert(tutorial_right);
        while crystals.len() < 8 {
            let p = Pos::new(rng.range(4, WIDTH / 2), rng.range(2, HEIGHT - 2));
            let mirror = Pos::new(WIDTH - 1 - p.x, HEIGHT - 1 - p.y);
            if !occupied.contains(&p) && !occupied.contains(&mirror) {
                let amount = 6 + rng.range(0, 5) as u8;
                crystals.push(Crystal { position: p, amount });
                crystals.push(Crystal { position: mirror, amount });
                occupied.insert(p);
                occupied.insert(mirror);
            }
        }

        // A rich neutral objective creates interaction in the middle.
        let center = Pos::new(WIDTH / 2 - 1, HEIGHT / 2);
        crystals.push(Crystal { position: center, amount: 16 });

        let starts = [-1, 0, 1];
        let mut drones = Vec::new();
        for team in Team::ALL {
            for (slot, offset) in starts.into_iter().enumerate() {
                let base = bases[team.index()];
                drones.push(Drone {
                    id: slot,
                    team,
                    position: Pos::new(base.x, base.y + offset),
                    cargo: 0,
                    role: if team == Team::Amber && slot == 0 { Role::Scout } else { Role::Courier },
                    target: None,
                    reason: "Booting flight plan",
                    move_request: None,
                    request_since: 0,
                    blocked_turns: 0,
                    last_position: None,
                    yield_cooldown: 0,
                });
            }
        }

        let mut sim = Self {
            turn: 0, scores: [0, 0], bases, walls, drones, crystals,
            memories: [TeamMemory::default(), TeamMemory::default()],
            finished: false,
            last_event: "Telemetry online — both fleets launched".into(),
            turn_explanation: "Both fleets are choosing their first assignments.".into(),
        };
        sim.observe();
        sim
    }

    pub fn step(&mut self) {
        if self.finished { return; }
        self.observe();
        let actions: Vec<Action> = (0..self.drones.len())
            .map(|index| self.decide(index))
            .collect();
        self.turn_explanation = self.drones.iter().map(|drone| {
            let target = drone.target.map_or("—".to_string(), |p| p.board_label());
            format!("{}{} {} → {}", if drone.team == Team::Azure { "A" } else { "B" }, drone.id + 1, drone.reason, target)
        }).collect::<Vec<_>>().join("\n");

        let planned_moves: Vec<Option<Pos>> = actions.iter().map(|action| {
            match action {
                Action::Move(next) if self.passable(*next) => Some(*next),
                _ => None,
            }
        }).collect();
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
                    if candidate_key < winner_key { *winner = index; }
                })
                .or_insert(index);
        }
        let selected: Vec<bool> = planned_moves.iter().enumerate()
            .map(|(index, next)| next.is_some_and(|p| winner_for_destination.get(&p) == Some(&index)))
            .collect();
        let occupants: HashMap<Pos, usize> = self.drones.iter().enumerate()
            .map(|(index, drone)| (drone.position, index)).collect();
        let mut resolution = vec![0_u8; self.drones.len()]; // 0 unknown, 1 visiting, 2 yes, 3 no
        let executable: Vec<bool> = (0..self.drones.len()).map(|index| {
            Self::can_execute_move(index, &planned_moves, &selected, &occupants, &mut resolution)
        }).collect();

        for (index, action) in actions.into_iter().enumerate() {
            match action {
                Action::Move(next) if executable[index] => {
                    let previous = self.drones[index].position;
                    self.drones[index].position = next;
                    self.drones[index].move_request = None;
                    self.drones[index].blocked_turns = 0;
                    self.drones[index].last_position = Some(previous);
                    self.drones[index].yield_cooldown = self.drones[index].yield_cooldown.saturating_sub(1);
                }
                Action::Move(_) => {
                    self.drones[index].blocked_turns += 1;
                    if self.drones[index].blocked_turns >= 2 {
                        self.drones[index].yield_cooldown = 3;
                    }
                }
                Action::Harvest => {
                    if self.drones[index].cargo < CARGO_CAPACITY {
                        if let Some(crystal) = self.crystals.iter_mut().find(|c| c.position == self.drones[index].position && c.amount > 0) {
                            crystal.amount -= 1;
                            self.drones[index].cargo += 1;
                            self.last_event = format!("{}-{} harvested a sky crystal", self.drones[index].team.label(), self.drones[index].id + 1);
                        }
                    }
                }
                Action::Deposit => {
                    let drone = &mut self.drones[index];
                    if drone.position == self.bases[drone.team.index()] && drone.cargo > 0 {
                        let delivered = drone.cargo as u32;
                        self.scores[drone.team.index()] += delivered;
                        drone.cargo = 0;
                        self.last_event = format!("{}-{} delivered {delivered} energy", drone.team.label(), drone.id + 1);
                    }
                }
                _ => {}
            }
        }

        self.turn += 1;
        self.observe();
        self.finished = self.turn >= MAX_TURNS || self.crystals.iter().all(|c| c.amount == 0) && self.drones.iter().all(|d| d.cargo == 0);
        if self.finished {
            self.last_event = match self.scores[0].cmp(&self.scores[1]) {
                std::cmp::Ordering::Greater => "Match complete — Azure wins".into(),
                std::cmp::Ordering::Less => "Match complete — Amber wins".into(),
                std::cmp::Ordering::Equal => "Match complete — draw".into(),
            };
        }
    }

    fn drone_order(&self, index: usize) -> usize {
        self.drones[index].team.index() * 3 + self.drones[index].id
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
                for x in 0..WIDTH { for y in 0..HEIGHT {
                    let p = Pos::new(x, y);
                    if drone.position.distance(p) <= SENSOR_RANGE { memory.explored.insert(p); }
                }}
            }
            for crystal in &self.crystals {
                if memory.explored.contains(&crystal.position) {
                    memory.known_crystals.insert(crystal.position, crystal.amount);
                }
            }
        }
    }

    fn decide(&mut self, index: usize) -> Action {
        let drone = self.drones[index].clone();
        let team = drone.team;
        let base = self.bases[team.index()];

        if drone.position == base && drone.cargo > 0 {
            self.set_intent(index, Role::Courier, Some(base), "Unloading at home dock");
            return Action::Deposit;
        }
        if drone.cargo == CARGO_CAPACITY {
            self.set_intent(index, Role::Courier, Some(base), "Cargo full — returning home");
            return self.move_toward_for_drone(index, base);
        }
        if self.crystals.iter().any(|c| c.position == drone.position && c.amount > 0) {
            self.set_intent(index, Role::Harvester, Some(drone.position), "Extracting visible crystal");
            return Action::Harvest;
        }

        let known: Vec<Pos> = self.memories[team.index()].known_crystals.iter()
            .filter_map(|(p, amount)| (*amount > 0).then_some(*p)).collect();
        let mut known = known;
        known.sort_by_key(|p| (p.x, p.y));

        // Azure is a straightforward greedy baseline. Amber's first drone is a
        // hybrid scout: on a three-drone fleet, a permanently dedicated scout
        // spends too much of the team's carrying capacity. It scouts only until
        // the team has enough choices, then joins the logistics loop.
        let scouting_coverage_limit = (WIDTH * HEIGHT) as usize * SCOUT_COVERAGE_LIMIT_PERCENT / 100;
        let needs_more_intel = known.len() < SCOUT_SOURCE_GOAL
            && self.memories[team.index()].explored.len() < scouting_coverage_limit;
        if team == Team::Amber && drone.id == 0 && needs_more_intel {
            let target = drone.target.filter(|target| !self.memories[team.index()].explored.contains(target))
                .or_else(|| self.frontier_target(team, drone.position, drone.id));
            if let Some(target) = target {
                self.set_intent(index, Role::Scout, Some(target), "Scouting until two sources are known");
                return self.move_toward_for_drone(index, target);
            }
        }

        if !known.is_empty() {
            let target = if let Some(existing) = drone.target.filter(|target| known.contains(target)) {
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
            self.set_intent(index, Role::Courier, Some(target), if team == Team::Amber { "Assigned to a team resource" } else { "Heading to nearest known resource" });
            let action = self.move_toward_for_drone(index, target);
            if !matches!(action, Action::Wait) || drone.position == target { return action; }
            // Drop an unreachable remembered target immediately and resume exploration.
            if let Some(fallback) = self.frontier_target(team, drone.position, drone.id) {
                self.set_intent(index, Role::Scout, Some(fallback), "Resource route blocked — rerouting");
                return self.move_toward_for_drone(index, fallback);
            }
            return Action::Wait;
        }

        if let Some(target) = self.frontier_target(team, drone.position, drone.id) {
            self.set_intent(index, Role::Scout, Some(target), "Searching for energy signatures");
            self.move_toward_for_drone(index, target)
        } else if drone.cargo > 0 {
            self.set_intent(index, Role::Courier, Some(base), "No resources remain — banking cargo");
            self.move_toward_for_drone(index, base)
        } else {
            let parking = self.parking_spot(team, drone.id);
            if drone.position != parking {
                self.set_intent(index, Role::Courier, Some(parking), "No task — clearing the shipping lane");
                self.move_toward_for_drone(index, parking)
            } else {
                self.set_intent(index, Role::Courier, Some(parking), "Parked clear of the shipping lane");
                Action::Wait
            }
        }
    }

    fn set_intent(&mut self, index: usize, role: Role, target: Option<Pos>, reason: &'static str) {
        self.drones[index].role = role;
        self.drones[index].target = target;
        self.drones[index].reason = reason;
    }

    fn frontier_target(&self, team: Team, from: Pos, slot: usize) -> Option<Pos> {
        let explored = &self.memories[team.index()].explored;
        let mut candidates: Vec<Pos> = (0..WIDTH).flat_map(|x| (0..HEIGHT).map(move |y| Pos::new(x, y)))
            .filter(|p| !self.walls.contains(p) && !explored.contains(p))
            .collect();
        candidates.sort_by_key(|p| (from.distance(*p) + ((p.x + p.y + slot as i32 * 3).rem_euclid(7)), p.x, p.y));
        candidates.into_iter().find(|candidate| !matches!(self.move_toward(from, *candidate), Action::Wait))
    }

    fn move_toward(&self, from: Pos, target: Pos) -> Action {
        if from == target { return Action::Wait; }
        let mut queue = VecDeque::from([from]);
        let mut previous = HashMap::new();
        previous.insert(from, from);
        while let Some(current) = queue.pop_front() {
            if current == target { break; }
            for next in current.neighbors() {
                if self.passable(next) && !previous.contains_key(&next) {
                    previous.insert(next, current);
                    queue.push_back(next);
                }
            }
        }
        if !previous.contains_key(&target) { return Action::Wait; }
        let mut step = target;
        while previous[&step] != from { step = previous[&step]; }
        Action::Move(step)
    }

    /// After losing two reservations, a drone yields into a free side lane if
    /// one still has a route to its goal. This turns a face-to-face queue into
    /// a deliberate detour instead of an infinite left/right dance.
    fn move_toward_for_drone(&self, index: usize, target: Pos) -> Action {
        let drone = &self.drones[index];
        let direct = self.move_toward(drone.position, target);
        if drone.blocked_turns < 2 && drone.yield_cooldown == 0 { return direct; }

        let occupied: HashSet<Pos> = self.drones.iter().map(|other| other.position).collect();
        let mut alternatives: Vec<(i32, Pos)> = drone.position.neighbors().into_iter()
            .filter(|candidate| self.passable(*candidate) && !occupied.contains(candidate))
            .filter(|candidate| drone.yield_cooldown == 0 || Some(*candidate) != drone.last_position)
            .filter_map(|candidate| self.route_distance(candidate, target).map(|distance| (distance, candidate)))
            .collect();
        alternatives.sort_by_key(|(distance, candidate)| (*distance, candidate.x, candidate.y));
        alternatives.first().map_or(direct, |(_, candidate)| Action::Move(*candidate))
    }

    fn route_distance(&self, from: Pos, target: Pos) -> Option<i32> {
        let mut queue = VecDeque::from([(from, 0)]);
        let mut visited = HashSet::from([from]);
        while let Some((current, distance)) = queue.pop_front() {
            if current == target { return Some(distance); }
            for next in current.neighbors() {
                if self.passable(next) && visited.insert(next) {
                    queue.push_back((next, distance + 1));
                }
            }
        }
        None
    }

    fn parking_spot(&self, team: Team, id: usize) -> Pos {
        let azure_slots = [Pos::new(2, 3), Pos::new(2, 12), Pos::new(2, 5)];
        let slot = azure_slots[id % azure_slots.len()];
        match team {
            Team::Azure => slot,
            Team::Amber => Pos::new(WIDTH - 1 - slot.x, HEIGHT - 1 - slot.y),
        }
    }

    fn passable(&self, p: Pos) -> bool {
        p.x >= 0 && p.x < WIDTH && p.y >= 0 && p.y < HEIGHT && !self.walls.contains(&p)
    }
}

struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self { Self(seed.max(1)) }
    fn next(&mut self) -> u64 { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); self.0 }
    fn range(&mut self, min: i32, max: i32) -> i32 { min + (self.next() % (max - min) as u64) as i32 }
    fn chance(&mut self, percent: u64) -> bool { self.next() % 100 < percent }
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
            assert!(a.walls.contains(&Pos::new(WIDTH - 1 - wall.x, HEIGHT - 1 - wall.y)));
        }
    }

    #[test]
    fn match_finishes_and_delivers_energy() {
        let mut sim = Simulation::new(7);
        while !sim.finished { sim.step(); }
        assert!(sim.turn <= MAX_TURNS);
        assert!(sim.scores[0] + sim.scores[1] > 0);
    }

    #[test]
    fn same_seed_produces_same_result() {
        let mut a = Simulation::new(99);
        let mut b = Simulation::new(99);
        while !a.finished { a.step(); }
        while !b.finished { b.step(); }
        assert_eq!(a.scores, b.scores);
        assert_eq!(a.turn, b.turn);
    }

    #[test]
    fn demo_seed_produces_visible_score() {
        let mut sim = Simulation::new(42);
        while !sim.finished { sim.step(); }
        assert!(sim.scores[0] + sim.scores[1] > 0, "the default demo seed should show at least one delivery");
        assert!(sim.scores[Team::Amber.index()] > 0, "the hybrid Amber strategy should contribute a delivery on the demo map");
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
                assert_eq!(unique_positions.len(), after.len(), "seed {seed} allowed drones to overlap at turn {}", first.turn);
                unchanged_turns = if before == after { unchanged_turns + 1 } else { 0 };
                assert!(unchanged_turns < 20, "seed {seed} stagnated at turn {}", first.turn);
            }
            while !second.finished { second.step(); }
            assert_eq!(first.scores, second.scores, "seed {seed} was not deterministic");
            assert!(first.turn <= MAX_TURNS, "seed {seed} exceeded match limit");
            assert!(
                !(first.turn == MAX_TURNS
                    && first.crystals.iter().all(|crystal| crystal.amount == 0)
                    && first.drones.iter().any(|drone| drone.cargo > 0)),
                "seed {seed} timed out while final cargo was still waiting to be delivered"
            );
        }
    }
}
