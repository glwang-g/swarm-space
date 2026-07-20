use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::camera::{Projection, Viewport};
use bevy::camera::visibility::RenderLayers;
use bevy::ui::IsDefaultUiCamera;
use bevy::window::{MonitorSelection, WindowMode, WindowResolution, WindowResized};
use swarm_core::*;

const PANEL_WIDTH: f32 = 330.0;
const MIN_CELL: f32 = 24.0;
const MINIMAP_SIZE: f32 = 172.0;

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

#[derive(Resource, Clone, Copy)]
struct BoardLayout { cell: f32, origin: Vec2, size: Vec2 }

impl BoardLayout {
    fn for_scenario(scenario: Scenario, window_size: Vec2) -> Self {
        let playfield_width = (window_size.x - PANEL_WIDTH).max(240.0);
        // Cover the playable viewport instead of letterboxing it. Any excess
        // becomes navigable world space, which is why the camera can pan.
        let cell = (playfield_width / scenario.width as f32)
            .max(window_size.y / scenario.height as f32)
            .max(MIN_CELL);
        let width = scenario.width as f32 * cell;
        let height = scenario.height as f32 * cell;
        Self {
            cell,
            // World coordinates are independent of the window. When this is
            // larger than the viewport, the camera simply shows a portion.
            origin: Vec2::new(-width / 2.0, -height / 2.0),
            size: Vec2::new(width, height),
        }
    }
}

#[derive(Resource, Default)]
struct MiniMapDrag { active: bool }

#[derive(Resource)]
struct ArenaSetup {
    drones_per_team: usize,
    crystal_sites: usize,
    wall_chance_percent: u32,
}

impl Default for ArenaSetup {
    fn default() -> Self {
        let scenario = Scenario::default();
        Self {
            drones_per_team: scenario.drones_per_team,
            crystal_sites: scenario.crystal_sites,
            wall_chance_percent: scenario.wall_chance_percent,
        }
    }
}

impl ArenaSetup {
    fn scenario(&self) -> Scenario {
        let mut scenario = Scenario::scaled(self.drones_per_team, [Strategy::Autonomous, Strategy::HybridScout]);
        scenario.crystal_sites = self.crystal_sites;
        scenario.wall_chance_percent = self.wall_chance_percent;
        scenario
    }
}

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
#[derive(Component)] struct ArenaSetupText;
#[derive(Component)] struct SidebarScroll;
#[derive(Component)] struct MapCamera;
#[derive(Component)] struct MiniMapViewport { scale: f32 }

fn grid_translation(layout: BoardLayout, pos: Pos, z: f32) -> Vec3 {
    let x = layout.origin.x + (pos.x as f32 + 0.5) * layout.cell;
    let y = layout.origin.y + (pos.y as f32 + 0.5) * layout.cell;
    Vec3::new(x, y, z)
}

fn setup(mut commands: Commands, assets: Res<AssetServer>, windows: Query<&Window>) {
    commands.insert_resource(UiFont(assets.load("Songti.ttf")));
    let simulation = Simulation::new(42);
    let layout = BoardLayout::for_scenario(simulation.scenario, window_size(&windows));
    let window = windows.single().expect("one primary window");
    commands.spawn((
        MapCamera,
        Camera2d,
        Camera { clear_color: ClearColorConfig::Custom(BG), viewport: Some(map_viewport(window)), ..default() },
    ));
    // UI must be laid out against the full window, not the cropped world
    // viewport. Its render layer deliberately excludes the world sprites.
    commands.spawn((
        IsDefaultUiCamera,
        RenderLayers::layer(1),
        Camera2d,
        Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
    ));
    commands.insert_resource(layout);
    commands.insert_resource(MiniMapDrag::default());
    spawn_world(&mut commands, &simulation, layout);
    spawn_ui(&mut commands);
    commands.insert_resource(MatchState { simulation, paused: true, intro: true, guided: false, view_team: None, speed: 1, seed: 42, accumulator: 0.0 });
    commands.insert_resource(ArenaSetup::default());
}

fn window_size(windows: &Query<&Window>) -> Vec2 {
    windows.single().map(|window| Vec2::new(window.resolution.width(), window.resolution.height()))
        .unwrap_or(Vec2::new(1280.0, 720.0))
}

fn size_of(window: &Window) -> Vec2 {
    Vec2::new(window.resolution.width(), window.resolution.height())
}

fn map_viewport(window: &Window) -> Viewport {
    let panel = (PANEL_WIDTH * window.scale_factor()) as u32;
    Viewport {
        physical_position: UVec2::ZERO,
        physical_size: UVec2::new(window.resolution.physical_width().saturating_sub(panel), window.resolution.physical_height()),
        depth: 0.0..1.0,
    }
}

fn minimap_content_rect(window: &Window, layout: BoardLayout) -> (Vec2, Vec2, f32) {
    let scale = MINIMAP_SIZE / layout.size.x.max(layout.size.y);
    let map_size = layout.size * scale;
    let padding = 9.0;
    let outer = MINIMAP_SIZE + padding * 2.0;
    let frame_top_left = Vec2::new(
        window.resolution.width() - PANEL_WIDTH - 14.0 - outer,
        window.resolution.height() - 14.0 - outer,
    );
    (frame_top_left + Vec2::splat(padding) + (Vec2::splat(MINIMAP_SIZE) - map_size) * 0.5, map_size, scale)
}

fn clamp_camera_to_map(transform: &mut Transform, projection: &Projection, layout: BoardLayout, window: &Window) {
    let Projection::Orthographic(projection) = projection else { return };
    let playfield_width = (window.resolution.width() - PANEL_WIDTH).max(0.0);
    let half_view = Vec2::new(playfield_width, window.resolution.height()) * projection.scale * 0.5;
    let half_map = layout.size * 0.5;
    let max_x = (half_map.x - half_view.x).max(0.0);
    let max_y = (half_map.y - half_view.y).max(0.0);
    transform.translation.x = transform.translation.x.clamp(-max_x, max_x);
    transform.translation.y = transform.translation.y.clamp(-max_y, max_y);
}

fn spawn_world(commands: &mut Commands, sim: &Simulation, layout: BoardLayout) {
    for x in 0..sim.scenario.width { for y in 0..sim.scenario.height {
        let p = Pos::new(x, y);
        let color = if sim.walls.contains(&p) { WALL } else { SKY_TILE };
        let size = if sim.walls.contains(&p) { layout.cell - 3.0 } else { layout.cell - 1.0 };
        commands.spawn((WorldVisual, Sprite::from_color(color, Vec2::splat(size)), Transform::from_translation(grid_translation(layout, p, 0.0))));
    }}
    // Go-style coordinates on all four edges stay above the fog. The board is
    // 24 cells wide, so columns run A–X and rows run 0–15.
    for x in 0..sim.scenario.width {
        let label = char::from_u32(u32::from(b'A') + x as u32).unwrap_or('?').to_string();
        for y in [0, sim.scenario.height - 1] {
            let mut position = grid_translation(layout, Pos::new(x, y), 11.0);
            position.y += if y == 0 { -layout.cell * 0.33 } else { layout.cell * 0.33 };
            commands.spawn((
                WorldVisual,
                Text2d::new(label.clone()),
                TextFont::from_font_size(10.0),
                TextColor(Color::srgba(0.78, 0.88, 0.98, 0.78)),
                Transform::from_translation(position),
            ));
        }
    }
    for y in 0..sim.scenario.height {
        for x in [0, sim.scenario.width - 1] {
            let mut position = grid_translation(layout, Pos::new(x, y), 11.0);
            position.x += if x == 0 { -layout.cell * 0.31 } else { layout.cell * 0.31 };
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
        commands.spawn((WorldVisual, Sprite::from_color(color.with_alpha(0.32), Vec2::splat(layout.cell * 1.65)), Transform::from_translation(grid_translation(layout, sim.bases[team.index()], 1.0))));
        commands.spawn((WorldVisual, Sprite::from_color(color, Vec2::new(layout.cell * 0.72, layout.cell * 0.18)), Transform::from_translation(grid_translation(layout, sim.bases[team.index()], 2.0))));
    }
    for crystal in &sim.crystals {
        commands.spawn((WorldVisual, CrystalVisual(crystal.position), Sprite::from_color(CRYSTAL, Vec2::splat(layout.cell * 0.42)), Transform { translation: grid_translation(layout, crystal.position, 3.0), rotation: Quat::from_rotation_z(0.785), ..default() }));
    }
    for drone in &sim.drones {
        let color = if drone.team == Team::Azure { AZURE } else { AMBER };
        commands.spawn((WorldVisual, TargetVisual(drone.team, drone.id), Sprite::from_color(color.with_alpha(0.18), Vec2::splat(layout.cell * 0.72)), Transform::from_translation(grid_translation(layout, drone.position, 2.5)), Visibility::Hidden));
        commands.spawn((WorldVisual, DroneVisual(drone.team, drone.id), Sprite::from_color(color, Vec2::new(layout.cell * 0.66, layout.cell * 0.52)), Transform::from_translation(grid_translation(layout, drone.position, 5.0))));
        let label = format!("{}{}", if drone.team == Team::Azure { "A" } else { "B" }, drone.id + 1);
        commands.spawn((
            WorldVisual,
            DroneLabel(drone.team, drone.id),
            Text2d::new(label),
            TextFont::from_font_size(11.0),
            TextColor(Color::srgb(0.97, 0.99, 1.0)),
            Transform::from_translation(grid_translation(layout, drone.position, 6.0) + Vec3::new(0.0, -1.0, 0.0)),
        ));
    }
    for x in 0..sim.scenario.width { for y in 0..sim.scenario.height {
        let p = Pos::new(x, y);
        commands.spawn((WorldVisual, FogVisual(p), Sprite::from_color(Color::srgba(0.005, 0.012, 0.035, 0.94), Vec2::splat(layout.cell)), Transform::from_translation(grid_translation(layout, p, 10.0)), Visibility::Hidden));
    }}
    spawn_minimap(commands, sim, layout);
}

fn spawn_minimap(commands: &mut Commands, sim: &Simulation, layout: BoardLayout) {
    let scale = MINIMAP_SIZE / layout.size.x.max(layout.size.y);
    let map_size = layout.size * scale;
    let padding = 9.0;
    let outer = MINIMAP_SIZE + padding * 2.0;
    commands.spawn((
        WorldVisual,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(PANEL_WIDTH + 14.0), bottom: Val::Px(14.0),
            width: Val::Px(outer), height: Val::Px(outer),
            border: UiRect::all(Val::Px(1.0)), ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.94)),
        BorderColor::all(Color::srgba(0.35, 0.53, 0.70, 0.78)),
    )).with_children(|frame| {
        let offset = (MINIMAP_SIZE - map_size) * 0.5;
        frame.spawn((
            WorldVisual,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(padding + offset.x), top: Val::Px(padding + offset.y),
                width: Val::Px(map_size.x), height: Val::Px(map_size.y), ..default()
            },
            BackgroundColor(Color::srgb(0.035, 0.07, 0.12)),
        )).with_children(|map| {
            for x in 0..sim.scenario.width { for y in 0..sim.scenario.height {
                let pos = Pos::new(x, y);
                let color = if sim.walls.contains(&pos) { WALL }
                    else if sim.crystals.iter().any(|crystal| crystal.position == pos && crystal.amount > 0) { CRYSTAL }
                    else { SKY_TILE };
                map.spawn((
                    WorldVisual,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x as f32 * layout.cell * scale),
                        // UI coordinates run downward; board coordinates run upward.
                        top: Val::Px((sim.scenario.height - 1 - y) as f32 * layout.cell * scale),
                        width: Val::Px((layout.cell * scale).ceil()), height: Val::Px((layout.cell * scale).ceil()),
                        ..default()
                    },
                    BackgroundColor(color),
                ));
            }}
            for team in Team::ALL {
                let base = sim.bases[team.index()];
                let color = if team == Team::Azure { AZURE } else { AMBER };
                map.spawn((
                    WorldVisual,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(base.x as f32 * layout.cell * scale),
                        top: Val::Px((sim.scenario.height - 1 - base.y) as f32 * layout.cell * scale),
                        width: Val::Px((layout.cell * scale).max(3.0)), height: Val::Px((layout.cell * scale).max(3.0)),
                        ..default()
                    },
                    BackgroundColor(color),
                ));
            }
            map.spawn((
                WorldVisual,
                MiniMapViewport { scale },
                Node {
                    position_type: PositionType::Absolute,
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.45, 0.72, 1.0, 0.08)),
                BorderColor::all(Color::srgba(0.72, 0.88, 1.0, 0.94)),
            ));
        });
    });
}

fn text_style(size: f32, color: Color) -> (TextFont, TextColor) { (TextFont::from_font_size(size), TextColor(color)) }

fn spawn_ui(commands: &mut Commands) {
    commands.spawn((SidebarScroll, ScrollPosition::default(), Node {
        position_type: PositionType::Absolute, right: Val::Px(0.0), top: Val::Px(0.0),
        width: Val::Px(PANEL_WIDTH), height: Val::Percent(100.0), padding: UiRect::all(Val::Px(22.0)),
        flex_direction: FlexDirection::Column, row_gap: Val::Px(11.0), overflow: Overflow::scroll_y(),
        scrollbar_width: 6.0, ..default()
    }, BackgroundColor(Color::srgb(0.035, 0.06, 0.105)))).with_children(|p| {
        p.spawn((Text::new("SWARM SPACE"), text_style(15.0, MUTED).0, text_style(15.0, MUTED).1));
        p.spawn((Text::new("Floating Isles Logistics Duel"), text_style(23.0, Color::WHITE).0, text_style(23.0, Color::WHITE).1));
        p.spawn((Text::new("HOW TO READ THE MAP"), text_style(13.0, MUTED).0, text_style(13.0, MUTED).1, Node { margin: UiRect::top(Val::Px(6.0)), ..default() }));
        p.spawn((Text::new("◆ crystal  ◼ wall  ● base\nFog: black unknown · blue/orange intel · violet shared"), text_style(12.0, Color::srgb(0.72, 0.82, 0.92)).0, text_style(12.0, Color::srgb(0.72, 0.82, 0.92)).1));
        p.spawn((ScoreText, Text::new(""), text_style(26.0, Color::WHITE).0, text_style(26.0, Color::WHITE).1));
        p.spawn((Node { width: Val::Percent(100.0), height: Val::Px(6.0), ..default() }, BackgroundColor(Color::srgb(0.10, 0.15, 0.22)))).with_children(|bar| {
            bar.spawn((ProgressFill, Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(AZURE)));
        });
        p.spawn((StatusText, Text::new(""), text_style(15.0, MUTED).0, text_style(15.0, MUTED).1));
        p.spawn((Text::new("SCENARIO SETUP"), text_style(13.0, MUTED).0, text_style(13.0, MUTED).1, Node { margin: UiRect::top(Val::Px(8.0)), ..default() }));
        p.spawn((ArenaSetupText, Text::new(""), text_style(13.0, Color::srgb(0.80, 0.88, 0.96)).0, text_style(13.0, Color::srgb(0.80, 0.88, 0.96)).1));
        p.spawn((Text::new("FLEET TELEMETRY"), text_style(13.0, MUTED).0, text_style(13.0, MUTED).1, Node { margin: UiRect::top(Val::Px(8.0)), ..default() }));
        p.spawn((FleetText, Text::new(""), text_style(14.0, Color::srgb(0.82, 0.88, 0.94)).0, text_style(14.0, Color::srgb(0.82, 0.88, 0.94)).1));
        p.spawn(Node { flex_grow: 1.0, ..default() });
        p.spawn((EventText, Text::new(""), text_style(14.0, Color::srgb(0.72, 0.78, 0.87)).0, text_style(14.0, Color::srgb(0.72, 0.78, 0.87)).1));
        p.spawn((Text::new("Map: wheel / Option+↑↓ zoom · middle-drag / Space-drag pan\nMini-map: click or drag the view frame · Panel: scroll wheel\nSPACE pause · N step · V view · 1/2/3 speed\nR replay · G new map · F11 fullscreen"), text_style(12.0, MUTED).0, text_style(12.0, MUTED).1));
    });

    commands.spawn((EndOverlay, Node {
        position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0),
        right: Val::Px(PANEL_WIDTH), bottom: Val::Px(0.0), justify_content: JustifyContent::Center,
        align_items: AlignItems::Center, ..default()
    }, BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.76)), Visibility::Hidden)).with_children(|p| {
        p.spawn((EndText, Text::new(""), text_style(38.0, Color::WHITE).0, text_style(38.0, Color::WHITE).1, TextLayout::justify(Justify::Center)));
    });

    commands.spawn((IntroOverlay, Node {
        position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0),
        right: Val::Px(PANEL_WIDTH), bottom: Val::Px(0.0), justify_content: JustifyContent::Center,
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
    mut setup: ResMut<ArenaSetup>,
    mut layout: ResMut<BoardLayout>,
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
    if keys.just_pressed(KeyCode::BracketLeft) { setup.drones_per_team = setup.drones_per_team.saturating_sub(1).max(2); }
    if keys.just_pressed(KeyCode::BracketRight) { setup.drones_per_team = (setup.drones_per_team + 1).min(16); }
    if keys.just_pressed(KeyCode::Comma) { setup.crystal_sites = setup.crystal_sites.saturating_sub(2).max(5); }
    if keys.just_pressed(KeyCode::Period) { setup.crystal_sites = (setup.crystal_sites + 2).min(81); }
    if keys.just_pressed(KeyCode::Semicolon) { setup.wall_chance_percent = setup.wall_chance_percent.saturating_sub(2); }
    if keys.just_pressed(KeyCode::Quote) { setup.wall_chance_percent = (setup.wall_chance_percent + 2).min(35); }
    if keys.just_pressed(KeyCode::Digit1) { state.speed = 1; }
    if keys.just_pressed(KeyCode::Digit2) { state.speed = 4; }
    if keys.just_pressed(KeyCode::Digit3) { state.speed = 16; }
    if keys.just_pressed(KeyCode::KeyN) && (state.paused || state.guided) { state.simulation.step(); state.paused = true; }
    let restart = keys.just_pressed(KeyCode::KeyR) || keys.just_pressed(KeyCode::Enter);
    let regenerate = keys.just_pressed(KeyCode::KeyG);
    if restart || regenerate {
        if regenerate { state.seed = state.seed.wrapping_mul(6364136223846793005).wrapping_add(1); }
        for entity in &visuals { commands.entity(entity).despawn(); }
        state.simulation = Simulation::with_scenario(state.seed, setup.scenario());
        let window_size = windows.single().map(|window| size_of(window)).unwrap_or(Vec2::new(1280.0, 720.0));
        *layout = BoardLayout::for_scenario(state.simulation.scenario, window_size);
        state.paused = false;
        state.intro = false;
        state.guided = false;
        state.view_team = None;
        state.accumulator = 0.0;
        spawn_world(&mut commands, &state.simulation, *layout);
    }
}

#[cfg(any())]
fn launch_benchmark(lab: &mut ExperimentLab) {
    if lab.receiver.is_some() {
        lab.benchmark_status = "Benchmark is already running — please wait for this sample set.".into();
        return;
    }
    let (sender, receiver) = mpsc::channel();
    let max_drones = lab.max_drones;
    let samples = lab.samples;
    lab.receiver = Some(Mutex::new(receiver));
    lab.rows.clear();
    lab.benchmark_progress = Some((0, (max_drones as u64 - 1) * samples * 4));
    lab.benchmark_status = format!("Running: 2–{max_drones} drones/team, {samples} seeded maps, both side assignments…");
    std::thread::spawn(move || {
        let rows = benchmark_leadership_with_progress(max_drones, samples, |completed, total| {
            let _ = sender.send(BenchmarkUpdate::Progress { completed, total });
        });
        let _ = sender.send(BenchmarkUpdate::Finished(rows));
    });
}

#[cfg(any())]
fn update_lab(mut lab: ResMut<ExperimentLab>, mut overlays: Query<&mut Visibility, With<LabOverlay>>, mut text: Query<&mut Text, With<LabText>>) {
    let mut updates = Vec::new();
    if let Some(receiver) = &lab.receiver {
        if let Ok(receiver) = receiver.lock() {
            while let Ok(update) = receiver.try_recv() {
                updates.push(update);
            }
        }
    }
    if !updates.is_empty() {
        let mut finished = None;
        for update in updates {
            match update {
                BenchmarkUpdate::Progress { completed, total } => lab.benchmark_progress = Some((completed, total)),
                BenchmarkUpdate::Finished(rows) => finished = Some(rows),
            }
        }
        if let Some(rows) = finished {
            lab.rows = rows;
            lab.receiver = None;
            lab.benchmark_progress = Some((1, 1));
            lab.benchmark_status = "Finished. Positive B score difference means the organised B plan outscored autonomous A.".into();
        }
    }
    if let Ok(mut visibility) = overlays.single_mut() {
        *visibility = if lab.visible { Visibility::Visible } else { Visibility::Hidden };
    }
    if !lab.is_changed() { return; }
    if let Ok(mut value) = text.single_mut() {
        let mut lines = vec![
            format!("Selected preview: {} drones/team · Enter to play it", lab.selected_drones),
            format!("Benchmark range: 2–{} drones/team · {} seeded maps each · B to run", lab.max_drones, lab.samples),
            "Map area, crystal sites, obstacles and turn budget scale with fleet size.".into(),
            lab.benchmark_status.clone(),
            "\n fleet | B dedicated vs A auto | B hybrid vs A auto | better B plan".into(),
        ];
        if let Some((completed, total)) = lab.benchmark_progress {
            let percent = if total == 0 { 0 } else { completed * 100 / total };
            let filled = (percent / 5) as usize;
            lines.push(format!("[{}{}] {percent:>3}%  {completed}/{total} matches", "#".repeat(filled), ".".repeat(20 - filled)));
        }
        for row in &lab.rows {
            let dedicated = format!("{:+.1} / {:>4.0}%", row.dedicated_delta, row.dedicated_win_rate);
            let hybrid = format!("{:+.1} / {:>4.0}%", row.hybrid_delta, row.hybrid_win_rate);
            let best = if row.dedicated_delta > 0.0 && row.dedicated_delta >= row.hybrid_delta { "SCOUT" }
                else if row.hybrid_delta > 0.0 { "HYBRID" } else { "AUTO" };
            lines.push(format!(" {:>5} | {:>21} | {:>18} | {best}", row.drones_per_team, dedicated, hybrid));
        }
        lines.push("\nEach cell is B score difference / B win rate. Every seed runs both left/right assignments.".into());
        **value = lines.join("\n");
    }
}

fn update_arena_setup(setup: Res<ArenaSetup>, mut text: Query<&mut Text, With<ArenaSetupText>>) {
    if !setup.is_changed() { return; }
    if let Ok(mut text) = text.single_mut() {
        **text = format!(
            "公平镜像场景（双方相同）\n[ / ] 每队无人机：{}\n, / . 晶体点：{}\n; / ' 障碍密度：{}%\nEnter 应用设置并重新开局",
            setup.drones_per_team, setup.crystal_sites, setup.wall_chance_percent,
        );
    }
}

// The map lives in world coordinates, so a window resize requires rebuilding
// the lightweight visual layer around the unchanged simulation state.
fn resize_board(
    mut resized: MessageReader<WindowResized>,
    windows: Query<&Window>,
    state: Res<MatchState>,
    mut layout: ResMut<BoardLayout>,
    mut commands: Commands,
    visuals: Query<Entity, With<WorldVisual>>,
    mut cameras: Query<&mut Camera, With<MapCamera>>,
) {
    if resized.read().next().is_none() { return; }
    *layout = BoardLayout::for_scenario(state.simulation.scenario, window_size(&windows));
    if let (Ok(window), Ok(mut camera)) = (windows.single(), cameras.single_mut()) {
        camera.viewport = Some(map_viewport(window));
    }
    for entity in &visuals { commands.entity(entity).despawn(); }
    spawn_world(&mut commands, &state.simulation, *layout);
}

fn map_camera_controls(
    mut wheel: MessageReader<MouseWheel>,
    mut motion: MessageReader<MouseMotion>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    layout: Res<BoardLayout>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<MapCamera>>,
) {
    let Ok(window) = windows.single() else { return };
    let playfield_width = (window.resolution.width() - PANEL_WIDTH).max(0.0);
    let (minimap_origin, minimap_size, _) = minimap_content_rect(window, *layout);
    let pointer_in_minimap = window.cursor_position().is_some_and(|cursor| {
        cursor.x >= minimap_origin.x && cursor.x <= minimap_origin.x + minimap_size.x
            && cursor.y >= minimap_origin.y && cursor.y <= minimap_origin.y + minimap_size.y
    });
    let pointer_in_map = window.cursor_position().is_some_and(|cursor| cursor.x < playfield_width) && !pointer_in_minimap;
    let Ok((mut transform, mut camera_projection)) = cameras.single_mut() else { return };
    let Projection::Orthographic(projection) = &mut *camera_projection else { return };

    if pointer_in_map {
        for event in wheel.read() {
            let amount = match event.unit { MouseScrollUnit::Line => event.y, MouseScrollUnit::Pixel => event.y / 38.0 };
            if amount > 0.0 { projection.scale = (projection.scale * 0.86).max(0.35); }
            if amount < 0.0 { projection.scale = (projection.scale * 1.16).min(1.0); }
        }
    } else {
        // This reader must still consume events outside the map so a later
        // pointer move cannot apply stale wheel input to the camera.
        wheel.clear();
    }
    if (keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight)) && keys.just_pressed(KeyCode::ArrowUp) {
        projection.scale = (projection.scale * 0.86).max(0.35);
    }
    if (keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight)) && keys.just_pressed(KeyCode::ArrowDown) {
        projection.scale = (projection.scale * 1.16).min(1.0);
    }

    if pointer_in_map && (mouse.pressed(MouseButton::Middle) || (keys.pressed(KeyCode::Space) && mouse.pressed(MouseButton::Left))) {
        for event in motion.read() {
            transform.translation.x -= event.delta.x * projection.scale;
            transform.translation.y += event.delta.y * projection.scale;
        }
    } else {
        motion.clear();
    }

    // Keep the viewport over the map. A small board remains centered; a large
    // board can be panned only until its edge reaches the viewport edge.
    clamp_camera_to_map(&mut transform, &camera_projection, *layout, window);
}

fn minimap_controls(
    mut motion: MessageReader<MouseMotion>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut drag: ResMut<MiniMapDrag>,
    windows: Query<&Window>,
    layout: Res<BoardLayout>,
    mut cameras: Query<(&mut Transform, &Projection), With<MapCamera>>,
) {
    let (Ok(window), Ok((mut camera, projection))) = (windows.single(), cameras.single_mut()) else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let (origin, size, scale) = minimap_content_rect(window, *layout);
    let inside = cursor.x >= origin.x && cursor.x <= origin.x + size.x && cursor.y >= origin.y && cursor.y <= origin.y + size.y;
    if mouse.just_pressed(MouseButton::Left) && inside {
        drag.active = true;
        let half_map = layout.size * 0.5;
        camera.translation.x = (cursor.x - origin.x) / scale - half_map.x;
        camera.translation.y = half_map.y - (cursor.y - origin.y) / scale;
    }
    if !mouse.pressed(MouseButton::Left) { drag.active = false; }
    if drag.active {
        for event in motion.read() {
            camera.translation.x += event.delta.x / scale;
            camera.translation.y -= event.delta.y / scale;
        }
    } else {
        motion.clear();
    }
    clamp_camera_to_map(&mut camera, projection, *layout, window);
}

fn sync_minimap_viewport(
    windows: Query<&Window>,
    layout: Res<BoardLayout>,
    cameras: Query<(&Transform, &Projection), With<MapCamera>>,
    mut viewport: Query<(&MiniMapViewport, &mut Node)>,
) {
    let (Ok(window), Ok((camera, projection)), Ok((mini, mut node))) = (windows.single(), cameras.single(), viewport.single_mut()) else { return };
    let Projection::Orthographic(projection) = projection else { return };
    let playfield_width = (window.resolution.width() - PANEL_WIDTH).max(0.0);
    let half_view = Vec2::new(playfield_width, window.resolution.height()) * projection.scale * 0.5;
    let half_map = layout.size * 0.5;
    let visible_min = (camera.translation.truncate() - half_view).max(-half_map);
    let visible_max = (camera.translation.truncate() + half_view).min(half_map);
    let world_size = (visible_max - visible_min).max(Vec2::ZERO);
    node.left = Val::Px((visible_min.x + half_map.x) * mini.scale);
    node.top = Val::Px((half_map.y - visible_max.y) * mini.scale);
    node.width = Val::Px(world_size.x * mini.scale);
    node.height = Val::Px(world_size.y * mini.scale);
}

fn scroll_sidebar(
    mut wheel: MessageReader<MouseWheel>,
    mut sidebars: Query<(&mut ScrollPosition, &ComputedNode), With<SidebarScroll>>,
    windows: Query<&Window>,
) {
    let over_panel = windows.single().ok().and_then(|window| window.cursor_position())
        .is_none_or(|cursor| cursor.x >= window_size(&windows).x - PANEL_WIDTH);
    if !over_panel { wheel.clear(); return; }
    for event in wheel.read() {
        let delta = match event.unit {
            MouseScrollUnit::Line => event.y * 34.0,
            MouseScrollUnit::Pixel => event.y,
        };
        for (mut position, node) in &mut sidebars {
            let max_scroll = (node.content_size().y - node.size().y).max(0.0) * node.inverse_scale_factor;
            position.y = (position.y - delta).clamp(0.0, max_scroll);
        }
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
    layout: Res<BoardLayout>,
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
            transform.translation = grid_translation(*layout, drone.position, 5.0);
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
            transform.translation = grid_translation(*layout, drone.position, 6.0) + Vec3::new(0.0, -1.0, 0.0);
            *visibility = match viewed_team {
                Some(team) if drone.team != team && !is_currently_visible(team, drone.position) => Visibility::Hidden,
                _ => Visibility::Visible,
            };
        }
    }
    for (marker, mut transform, mut visibility) in &mut visuals.p2() {
        if let Some(target) = state.simulation.drones.iter().find(|d| d.team == marker.0 && d.id == marker.1).and_then(|d| d.target) {
            transform.translation = grid_translation(*layout, target, 2.5);
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
        .add_systems(Update, (controls, resize_board, map_camera_controls, minimap_controls, sync_minimap_viewport, run_match, sync_visuals, update_ui, update_arena_setup, scroll_sidebar, apply_ui_font, disable_word_segmentation).chain())
        .run();
}
