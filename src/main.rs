use bevy::prelude::*;
use bevy::window::{MonitorSelection, WindowMode, WindowResolution};
use swarm_core::*;

const CELL: f32 = 34.0;
const BOARD_WIDTH: f32 = WIDTH as f32 * CELL;
const BOARD_HEIGHT: f32 = HEIGHT as f32 * CELL;
const PANEL_WIDTH: f32 = 330.0;
const WINDOW_WIDTH: f32 = BOARD_WIDTH + PANEL_WIDTH;

const BG: Color = Color::srgb(0.025, 0.045, 0.09);
const SKY_TILE: Color = Color::srgb(0.055, 0.095, 0.16);
const WALL: Color = Color::srgb(0.16, 0.22, 0.27);
const AZURE: Color = Color::srgb(0.18, 0.72, 1.0);
const AMBER: Color = Color::srgb(1.0, 0.58, 0.16);
const CRYSTAL: Color = Color::srgb(0.62, 0.35, 1.0);
const MUTED: Color = Color::srgb(0.52, 0.62, 0.72);

#[derive(Resource)]
struct MatchState {
    simulation: Simulation,
    paused: bool,
    intro: bool,
    guided: bool,
    view_team: Option<Team>,
    speed: usize,
    seed: u64,
    accumulator: f32,
}

#[derive(Resource)]
struct UiFont(Handle<Font>);

#[derive(Component)] struct WorldVisual;
#[derive(Component)] struct DroneVisual(Team, usize);
#[derive(Component)] struct DroneLabel(Team, usize);
#[derive(Component)] struct CrystalVisual(Pos);
#[derive(Component)] struct TargetVisual(Team, usize);
#[derive(Component)] struct FogVisual(Pos);
#[derive(Component)] struct ScoreText;
#[derive(Component)] struct StatusText;
#[derive(Component)] struct FleetText;
#[derive(Component)] struct EventText;
#[derive(Component)] struct ProgressFill;
#[derive(Component)] struct EndOverlay;
#[derive(Component)] struct EndText;
#[derive(Component)] struct IntroOverlay;

fn grid_translation(pos: Pos, z: f32) -> Vec3 {
    let x = -WINDOW_WIDTH / 2.0 + (pos.x as f32 + 0.5) * CELL;
    let y = -BOARD_HEIGHT / 2.0 + (pos.y as f32 + 0.5) * CELL;
    Vec3::new(x, y, z)
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((Camera2d, Camera { clear_color: ClearColorConfig::Custom(BG), ..default() }));
    commands.insert_resource(UiFont(assets.load("Songti.ttf")));
    let simulation = Simulation::new(42);
    spawn_world(&mut commands, &simulation);
    spawn_ui(&mut commands);
    commands.insert_resource(MatchState { simulation, paused: true, intro: true, guided: false, view_team: None, speed: 1, seed: 42, accumulator: 0.0 });
}

fn spawn_world(commands: &mut Commands, sim: &Simulation) {
    for x in 0..WIDTH { for y in 0..HEIGHT {
        let p = Pos::new(x, y);
        let color = if sim.walls.contains(&p) { WALL } else { SKY_TILE };
        let size = if sim.walls.contains(&p) { CELL - 3.0 } else { CELL - 1.0 };
        commands.spawn((WorldVisual, Sprite::from_color(color, Vec2::splat(size)), Transform::from_translation(grid_translation(p, 0.0))));
    }}
    // Go-style coordinates on all four edges stay above the fog. The board is
    // 24 cells wide, so columns run A–X and rows run 0–15.
    for x in 0..WIDTH {
        let label = char::from_u32(u32::from(b'A') + x as u32).unwrap_or('?').to_string();
        for y in [0, HEIGHT - 1] {
            let mut position = grid_translation(Pos::new(x, y), 11.0);
            position.y += if y == 0 { -CELL * 0.33 } else { CELL * 0.33 };
            commands.spawn((
                WorldVisual,
                Text2d::new(label.clone()),
                TextFont::from_font_size(10.0),
                TextColor(Color::srgba(0.78, 0.88, 0.98, 0.78)),
                Transform::from_translation(position),
            ));
        }
    }
    for y in 0..HEIGHT {
        for x in [0, WIDTH - 1] {
            let mut position = grid_translation(Pos::new(x, y), 11.0);
            position.x += if x == 0 { -CELL * 0.31 } else { CELL * 0.31 };
            commands.spawn((
                WorldVisual,
                Text2d::new(y.to_string()),
                TextFont::from_font_size(9.0),
                TextColor(Color::srgba(0.78, 0.88, 0.98, 0.78)),
                Transform::from_translation(position),
            ));
        }
    }
    for team in Team::ALL {
        let color = if team == Team::Azure { AZURE } else { AMBER };
        commands.spawn((WorldVisual, Sprite::from_color(color.with_alpha(0.32), Vec2::splat(CELL * 1.65)), Transform::from_translation(grid_translation(sim.bases[team.index()], 1.0))));
        commands.spawn((WorldVisual, Sprite::from_color(color, Vec2::new(CELL * 0.72, CELL * 0.18)), Transform::from_translation(grid_translation(sim.bases[team.index()], 2.0))));
    }
    for crystal in &sim.crystals {
        commands.spawn((WorldVisual, CrystalVisual(crystal.position), Sprite::from_color(CRYSTAL, Vec2::splat(CELL * 0.42)), Transform { translation: grid_translation(crystal.position, 3.0), rotation: Quat::from_rotation_z(0.785), ..default() }));
    }
    for drone in &sim.drones {
        let color = if drone.team == Team::Azure { AZURE } else { AMBER };
        commands.spawn((WorldVisual, TargetVisual(drone.team, drone.id), Sprite::from_color(color.with_alpha(0.18), Vec2::splat(CELL * 0.72)), Transform::from_translation(grid_translation(drone.position, 2.5)), Visibility::Hidden));
        commands.spawn((WorldVisual, DroneVisual(drone.team, drone.id), Sprite::from_color(color, Vec2::new(CELL * 0.66, CELL * 0.52)), Transform::from_translation(grid_translation(drone.position, 5.0))));
        let label = format!("{}{}", if drone.team == Team::Azure { "A" } else { "B" }, drone.id + 1);
        commands.spawn((
            WorldVisual,
            DroneLabel(drone.team, drone.id),
            Text2d::new(label),
            TextFont::from_font_size(11.0),
            TextColor(Color::srgb(0.97, 0.99, 1.0)),
            Transform::from_translation(grid_translation(drone.position, 6.0) + Vec3::new(0.0, -1.0, 0.0)),
        ));
    }
    for x in 0..WIDTH { for y in 0..HEIGHT {
        let p = Pos::new(x, y);
        commands.spawn((WorldVisual, FogVisual(p), Sprite::from_color(Color::srgba(0.005, 0.012, 0.035, 0.94), Vec2::splat(CELL)), Transform::from_translation(grid_translation(p, 10.0)), Visibility::Hidden));
    }}
}

fn text_style(size: f32, color: Color) -> (TextFont, TextColor) { (TextFont::from_font_size(size), TextColor(color)) }

fn spawn_ui(commands: &mut Commands) {
    commands.spawn((Node {
        position_type: PositionType::Absolute, right: Val::Px(0.0), top: Val::Px(0.0),
        width: Val::Px(PANEL_WIDTH), height: Val::Percent(100.0), padding: UiRect::all(Val::Px(22.0)),
        flex_direction: FlexDirection::Column, row_gap: Val::Px(13.0), ..default()
    }, BackgroundColor(Color::srgb(0.035, 0.06, 0.105)))).with_children(|p| {
        p.spawn((Text::new("SWARM SPACE"), text_style(15.0, MUTED).0, text_style(15.0, MUTED).1));
        p.spawn((Text::new("Floating Isles\nLogistics Duel"), text_style(29.0, Color::WHITE).0, text_style(29.0, Color::WHITE).1));
        p.spawn((Text::new("HOW TO READ THE MAP"), text_style(13.0, MUTED).0, text_style(13.0, MUTED).1, Node { margin: UiRect::top(Val::Px(6.0)), ..default() }));
        p.spawn((Text::new("◆ crystal   ◼ wall   ● base\nBLUE = Greedy   ORANGE = Explorer\nGlobal fog: black unknown · blue/orange team intel · violet shared"), text_style(13.0, Color::srgb(0.72, 0.82, 0.92)).0, text_style(13.0, Color::srgb(0.72, 0.82, 0.92)).1));
        p.spawn((ScoreText, Text::new(""), text_style(26.0, Color::WHITE).0, text_style(26.0, Color::WHITE).1));
        p.spawn((Node { width: Val::Percent(100.0), height: Val::Px(6.0), ..default() }, BackgroundColor(Color::srgb(0.10, 0.15, 0.22)))).with_children(|bar| {
            bar.spawn((ProgressFill, Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(AZURE)));
        });
        p.spawn((StatusText, Text::new(""), text_style(15.0, MUTED).0, text_style(15.0, MUTED).1));
        p.spawn((Text::new("FLEET TELEMETRY"), text_style(13.0, MUTED).0, text_style(13.0, MUTED).1, Node { margin: UiRect::top(Val::Px(8.0)), ..default() }));
        p.spawn((FleetText, Text::new(""), text_style(14.0, Color::srgb(0.82, 0.88, 0.94)).0, text_style(14.0, Color::srgb(0.82, 0.88, 0.94)).1));
        p.spawn(Node { flex_grow: 1.0, ..default() });
        p.spawn((EventText, Text::new(""), text_style(14.0, Color::srgb(0.72, 0.78, 0.87)).0, text_style(14.0, Color::srgb(0.72, 0.78, 0.87)).1));
        p.spawn((Text::new("SPACE pause  N step  T teaching mode\nV view  1/2/3 speed  R replay  G new map  F11 fullscreen"), text_style(13.0, MUTED).0, text_style(13.0, MUTED).1));
    });

    commands.spawn((EndOverlay, Node {
        position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0),
        width: Val::Px(BOARD_WIDTH), height: Val::Percent(100.0), justify_content: JustifyContent::Center,
        align_items: AlignItems::Center, ..default()
    }, BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.76)), Visibility::Hidden)).with_children(|p| {
        p.spawn((EndText, Text::new(""), text_style(38.0, Color::WHITE).0, text_style(38.0, Color::WHITE).1, TextLayout::justify(Justify::Center)));
    });

    commands.spawn((IntroOverlay, Node {
        position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0),
        width: Val::Px(BOARD_WIDTH), height: Val::Percent(100.0), justify_content: JustifyContent::Center,
        align_items: AlignItems::Center, ..default()
    }, BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.92)))).with_children(|p| {
        p.spawn((Node { width: Val::Px(470.0), padding: UiRect::all(Val::Px(30.0)), flex_direction: FlexDirection::Column, row_gap: Val::Px(14.0), ..default() }, BackgroundColor(Color::srgb(0.055, 0.09, 0.15)))).with_children(|card| {
            card.spawn((Text::new("漂浮群岛物流战"), text_style(32.0, Color::WHITE).0, text_style(32.0, Color::WHITE).1));
            card.spawn((Text::new("两支无人机舰队争夺天空晶体。\n300 回合内，把更多能量运回基地的一方获胜。"), text_style(17.0, Color::srgb(0.82, 0.89, 0.96)).0, text_style(17.0, Color::srgb(0.82, 0.89, 0.96)).1));
            card.spawn((Text::new("蓝队：Greedy Bot，优先最近的已知晶体\n橙队：Explorer Bot，一架侦察、两架分工运输\n每架无人机只能看到附近 5 格，发现的信息会共享。"), text_style(15.0, MUTED).0, text_style(15.0, MUTED).1));
            card.spawn((Text::new("观察顺序：找到晶体 → 采集 → 满载返航 → 交付"), text_style(15.0, Color::srgb(0.65, 0.95, 0.8)).0, text_style(15.0, Color::srgb(0.65, 0.95, 0.8)).1));
            card.spawn((Text::new("按 Enter 或 Space 开始比赛"), text_style(19.0, Color::WHITE).0, text_style(19.0, Color::WHITE).1));
        });
    });
}

fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<MatchState>,
    mut commands: Commands,
    visuals: Query<Entity, With<WorldVisual>>,
    mut windows: Query<&mut Window>,
) {
    if keys.just_pressed(KeyCode::F11) {
        if let Ok(mut window) = windows.single_mut() {
            window.mode = match window.mode {
                WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                _ => WindowMode::Windowed,
            };
        }
    }
    if state.intro {
        if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
            state.intro = false;
            state.paused = false;
        }
        return;
    }
    if keys.just_pressed(KeyCode::Space) { state.paused = !state.paused; }
    if keys.just_pressed(KeyCode::KeyT) {
        state.guided = !state.guided;
        state.paused = state.guided;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        state.view_team = match state.view_team {
            None => Some(Team::Azure),
            Some(Team::Azure) => Some(Team::Amber),
            Some(Team::Amber) => None,
        };
    }
    if keys.just_pressed(KeyCode::Digit1) { state.speed = 1; }
    if keys.just_pressed(KeyCode::Digit2) { state.speed = 4; }
    if keys.just_pressed(KeyCode::Digit3) { state.speed = 16; }
    if keys.just_pressed(KeyCode::KeyN) && (state.paused || state.guided) { state.simulation.step(); state.paused = true; }
    let restart = keys.just_pressed(KeyCode::KeyR);
    let regenerate = keys.just_pressed(KeyCode::KeyG);
    if restart || regenerate {
        if regenerate { state.seed = state.seed.wrapping_mul(6364136223846793005).wrapping_add(1); }
        for entity in &visuals { commands.entity(entity).despawn(); }
        state.simulation = Simulation::new(state.seed);
        state.paused = false;
        state.intro = false;
        state.guided = false;
        state.view_team = None;
        state.accumulator = 0.0;
        spawn_world(&mut commands, &state.simulation);
    }
}

fn run_match(time: Res<Time>, mut state: ResMut<MatchState>) {
    if state.intro || state.paused || state.guided || state.simulation.finished { return; }
    state.accumulator += time.delta_secs();
    let interval = 0.24 / state.speed as f32;
    while state.accumulator >= interval {
        state.accumulator -= interval;
        state.simulation.step();
        if state.simulation.finished { break; }
    }
}

fn sync_visuals(
    state: Res<MatchState>,
    mut visuals: ParamSet<(
        Query<(&DroneVisual, &mut Transform, &mut Sprite, &mut Visibility)>,
        Query<(&DroneLabel, &mut Transform, &mut Visibility)>,
        Query<(&TargetVisual, &mut Transform, &mut Visibility)>,
        Query<(&CrystalVisual, &mut Visibility)>,
        Query<(&FogVisual, &mut Sprite, &mut Visibility)>,
    )>,
) {
    let viewed_team = state.view_team;
    let is_currently_visible = |team: Team, pos: Pos| state.simulation.drones.iter()
        .filter(|drone| drone.team == team)
        .any(|drone| drone.position.distance(pos) <= SENSOR_RANGE);

    for (marker, mut transform, mut sprite, mut visibility) in &mut visuals.p0() {
        if let Some(drone) = state.simulation.drones.iter().find(|d| d.team == marker.0 && d.id == marker.1) {
            transform.translation = grid_translation(drone.position, 5.0);
            let fullness = drone.cargo as f32 / CARGO_CAPACITY as f32;
            sprite.color = if drone.team == Team::Azure { AZURE } else { AMBER }.mix(&Color::WHITE, fullness * 0.35);
            *visibility = match viewed_team {
                Some(team) if drone.team != team && !is_currently_visible(team, drone.position) => Visibility::Hidden,
                _ => Visibility::Visible,
            };
        }
    }
    for (marker, mut transform, mut visibility) in &mut visuals.p1() {
        if let Some(drone) = state.simulation.drones.iter().find(|d| d.team == marker.0 && d.id == marker.1) {
            transform.translation = grid_translation(drone.position, 6.0) + Vec3::new(0.0, -1.0, 0.0);
            *visibility = match viewed_team {
                Some(team) if drone.team != team && !is_currently_visible(team, drone.position) => Visibility::Hidden,
                _ => Visibility::Visible,
            };
        }
    }
    for (marker, mut transform, mut visibility) in &mut visuals.p2() {
        if let Some(target) = state.simulation.drones.iter().find(|d| d.team == marker.0 && d.id == marker.1).and_then(|d| d.target) {
            transform.translation = grid_translation(target, 2.5);
            *visibility = if viewed_team.map_or(true, |team| team == marker.0) { Visibility::Visible } else { Visibility::Hidden };
        } else { *visibility = Visibility::Hidden; }
    }
    for (marker, mut visibility) in &mut visuals.p3() {
        let amount = match viewed_team {
            Some(team) => state.simulation.memories[team.index()].known_crystals.get(&marker.0).copied().unwrap_or(0),
            None => state.simulation.crystals.iter().find(|c| c.position == marker.0).map_or(0, |c| c.amount),
        };
        *visibility = if amount > 0 { Visibility::Visible } else { Visibility::Hidden };
    }
    for (marker, mut sprite, mut visibility) in &mut visuals.p4() {
        let azure_knows = state.simulation.memories[Team::Azure.index()].explored.contains(&marker.0);
        let amber_knows = state.simulation.memories[Team::Amber.index()].explored.contains(&marker.0);
        match viewed_team {
            Some(team) => {
                *visibility = if state.simulation.memories[team.index()].explored.contains(&marker.0) {
                    Visibility::Hidden
                } else {
                    sprite.color = Color::srgba(0.005, 0.012, 0.035, 0.94);
                    Visibility::Visible
                };
            }
            None => {
                // Omniscient mode reveals the terrain but visualises who has
                // discovered it: dark is unknown to both, violet is shared.
                let color = match (azure_knows, amber_knows) {
                    (false, false) => Color::srgba(0.005, 0.012, 0.035, 0.88),
                    (true, false) => AZURE.with_alpha(0.30),
                    (false, true) => AMBER.with_alpha(0.30),
                    (true, true) => Color::srgba(0.62, 0.38, 0.98, 0.12),
                };
                sprite.color = color;
                *visibility = Visibility::Visible;
            }
        }
    }
}

fn update_ui(
    state: Res<MatchState>,
    mut texts: ParamSet<(
        Query<&mut Text, With<ScoreText>>,
        Query<&mut Text, With<StatusText>>,
        Query<&mut Text, With<FleetText>>,
        Query<&mut Text, With<EventText>>,
        Query<&mut Text, With<EndText>>,
    )>,
    mut fill: Query<&mut Node, With<ProgressFill>>,
    mut overlays: ParamSet<(
        Query<&mut Visibility, With<EndOverlay>>,
        Query<&mut Visibility, With<IntroOverlay>>,
    )>,
) {
    let sim = &state.simulation;
    if let Ok(mut text) = texts.p0().single_mut() { **text = format!("{}  :  {}", sim.scores[0], sim.scores[1]); }
    if let Ok(mut text) = texts.p1().single_mut() {
        let remaining: u32 = sim.crystals.iter().map(|crystal| crystal.amount as u32).sum();
        let view = match state.view_team { None => "OMNISCIENT", Some(Team::Azure) => "AZURE MEMORY", Some(Team::Amber) => "AMBER MEMORY" };
        **text = format!("AZURE  Greedy Bot       AMBER  Explorer Bot\nTurn {:03} / {}   {}   Speed {}×\nView: {}   Crystals remaining: {}",
            sim.turn, MAX_TURNS, if state.intro { "READY" } else if sim.finished { "MATCH OVER" } else if state.guided { "TEACHING" } else if state.paused { "PAUSED" } else { "RUNNING" }, state.speed, view, remaining);
    }
    let mut lines = Vec::new();
    for drone in &sim.drones {
        let glyph = if drone.team == Team::Azure { "A" } else { "B" };
        let target = drone.target.map_or("—".into(), |p| p.board_label());
        lines.push(format!("{}{}  {:9}  {}/{}  → {:5}  {}", glyph, drone.id + 1, drone.role.label(), drone.cargo, CARGO_CAPACITY, target, drone.reason));
    }
    if let Ok(mut text) = texts.p2().single_mut() { **text = lines.join("\n"); }
    if let Ok(mut text) = texts.p3().single_mut() {
        **text = format!("TURN {} DECISIONS\n{}\n\nLATEST EVENT\n{}", sim.turn, sim.turn_explanation, sim.last_event);
    }
    if let Ok(mut node) = fill.single_mut() { node.width = Val::Percent(sim.turn as f32 / MAX_TURNS as f32 * 100.0); }
    let visible = sim.finished;
    if let Ok(mut value) = overlays.p0().single_mut() { *value = if visible { Visibility::Visible } else { Visibility::Hidden }; }
    if let Ok(mut value) = overlays.p1().single_mut() { *value = if state.intro { Visibility::Visible } else { Visibility::Hidden }; }
    if visible { if let Ok(mut text) = texts.p4().single_mut() {
        let winner = if sim.scores[0] > sim.scores[1] { "AZURE WINS" } else if sim.scores[1] > sim.scores[0] { "AMBER WINS" } else { "DRAW" };
        **text = format!("{}\n{} : {}\n\nBLUE used nearest-resource greed.\nORANGE used scouting and role assignment.\n\nPress R to replay this map\nPress G for a new map", winner, sim.scores[0], sim.scores[1]);
    }}
}

fn apply_ui_font(font: Res<UiFont>, mut texts: Query<&mut TextFont>) {
    for mut text_font in &mut texts {
        text_font.font = font.0.clone().into();
    }
}

fn disable_word_segmentation(mut layouts: Query<&mut TextLayout>) {
    for mut layout in &mut layouts {
        layout.linebreak = LineBreak::NoWrap;
    }
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin { primary_window: Some(Window {
                    title: "Swarm Space — Floating Isles Logistics Duel".into(),
                    resolution: WindowResolution::new(1280, 720),
                    resizable: true,
                    ..default()
                }), ..default() }),
        )
        .add_systems(Startup, setup)
        .add_systems(Update, (controls, run_match, sync_visuals, update_ui, apply_ui_font, disable_word_segmentation).chain())
        .run();
}
