use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use rand::random_range;

const CELL_SIZE: f32 = 30.0;
const TICK_SECONDS: f32 = 0.2;
const GRID_WIDTH: i32 = 20;
const GRID_HEIGHT: i32 = 20;
const WINDOW_WIDTH: f32 = GRID_WIDTH as f32 * CELL_SIZE;
const WINDOW_HEIGHT: f32 = GRID_HEIGHT as f32 * CELL_SIZE;
const WINDOW_TITLE: &str = "Bevy Snake";
const GAME_OVER_TITLE: &str = "Bevy Snake - Game Over";

const HEAD_COLOR: Color = Color::srgb(0.95, 0.95, 0.95);
const BODY_COLOR: Color = Color::srgb(0.25, 0.80, 0.35);
const FOOD_COLOR: Color = Color::srgb(0.90, 0.25, 0.25);

// —— 方向枚举 ——
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    fn as_ivec(self) -> IVec2 {
        match self {
            Direction::Up => IVec2::new(0, 1),
            Direction::Down => IVec2::new(0, -1),
            Direction::Left => IVec2::new(-1, 0),
            Direction::Right => IVec2::new(1, 0),
        }
    }

    fn opposite(self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}

// —— 组件 ——
#[derive(Component)]
struct SnakePart;

#[derive(Component)]
struct FoodMarker;

// —— 资源 ——
#[derive(Resource)]
struct MoveTimer(Timer);

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
enum GameState {
    Playing,
    GameOver,
}

#[derive(Resource)]
struct Snake {
    body: Vec<IVec2>,
    segments: Vec<Entity>,
    direction: Direction,
    pending_direction: Option<Direction>,
}

#[derive(Resource)]
struct Food {
    position: IVec2,
    entity: Entity,
}

#[derive(Resource)]
struct Score(u32);

#[derive(Resource)]
struct Hud {
    score_text: Entity,
    game_over_overlay: Entity,
    game_over_score_text: Entity,
}

fn grid_to_translation(pos: IVec2) -> Vec3 {
    let x = (pos.x as f32 - GRID_WIDTH as f32 / 2.0 + 0.5) * CELL_SIZE;
    let y = (pos.y as f32 - GRID_HEIGHT as f32 / 2.0 + 0.5) * CELL_SIZE;
    Vec3::new(x, y, 0.0)
}

fn in_bounds(pos: IVec2) -> bool {
    pos.x >= 0 && pos.x < GRID_WIDTH && pos.y >= 0 && pos.y < GRID_HEIGHT
}

fn title_for_state(state: GameState, score: u32) -> String {
    match state {
        GameState::Playing => format!("{WINDOW_TITLE} - Score {score}"),
        GameState::GameOver => GAME_OVER_TITLE.to_string(),
    }
}

fn update_window_title(window: &mut Window, state: GameState, score: u32) {
    window.title = title_for_state(state, score);
}

fn random_food_position(occupied: &[IVec2]) -> IVec2 {
    loop {
        let pos = IVec2::new(
            random_range(0..GRID_WIDTH),
            random_range(0..GRID_HEIGHT),
        );
        if !occupied.contains(&pos) {
            return pos;
        }
    }
}

fn spawn_snake_part(commands: &mut Commands, pos: IVec2, color: Color) -> Entity {
    commands
        .spawn((
            SnakePart,
            Sprite::from_color(color, Vec2::splat(CELL_SIZE - 2.0)),
            Transform::from_translation(grid_to_translation(pos)),
        ))
        .id()
}

fn spawn_food(commands: &mut Commands, pos: IVec2) -> Entity {
    commands
        .spawn((
            FoodMarker,
            Sprite::from_color(FOOD_COLOR, Vec2::splat(CELL_SIZE - 6.0)),
            Transform::from_translation(grid_to_translation(pos)),
        ))
        .id()
}

fn spawn_hud(commands: &mut Commands) -> Hud {
    let score_text = commands
        .spawn((
            Text::new("Score: 0"),
            TextFont::from_font_size(24.0),
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                top: Val::Px(12.0),
                ..default()
            },
        ))
        .id();

    let game_over_overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            Visibility::Hidden,
        ))
        .id();

    let mut game_over_score_text = Entity::PLACEHOLDER;
    commands.entity(game_over_overlay).with_children(|parent| {
        parent
            .spawn((
                Node {
                    padding: UiRect::all(Val::Px(28.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(12.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.08, 0.1, 0.95)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("Game Over"),
                    TextFont::from_font_size(44.0),
                    TextColor(Color::WHITE),
                    TextLayout::justify(Justify::Center),
                ));

                game_over_score_text = panel
                    .spawn((
                        Text::new("Final score: 0"),
                        TextFont::from_font_size(24.0),
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                        TextLayout::justify(Justify::Center),
                    ))
                    .id();

                panel.spawn((
                    Text::new("Press R to restart"),
                    TextFont::from_font_size(20.0),
                    TextColor(Color::srgb(0.85, 0.85, 0.85)),
                    TextLayout::justify(Justify::Center),
                ));
            });
    });

    Hud {
        score_text,
        game_over_overlay,
        game_over_score_text,
    }
}

fn update_score_text(commands: &mut Commands, hud: &Hud, score: u32) {
    commands
        .entity(hud.score_text)
        .insert(Text::new(format!("Score: {score}")));
}

fn update_game_over_text(commands: &mut Commands, hud: &Hud, score: u32) {
    commands.entity(hud.game_over_score_text).insert(Text::new(format!(
        "Final score: {score}"
    )));
}

fn show_game_over_overlay(commands: &mut Commands, hud: &Hud) {
    commands
        .entity(hud.game_over_overlay)
        .insert(Visibility::Visible);
}

fn hide_game_over_overlay(commands: &mut Commands, hud: &Hud) {
    commands
        .entity(hud.game_over_overlay)
        .insert(Visibility::Hidden);
}

fn spawn_initial_snake(commands: &mut Commands) -> Snake {
    let head = IVec2::new(GRID_WIDTH / 2, GRID_HEIGHT / 2);
    let body = vec![
        head,
        head - IVec2::new(1, 0),
        head - IVec2::new(2, 0),
    ];

    let mut segments = Vec::with_capacity(body.len());
    for (index, &pos) in body.iter().enumerate() {
        let color = if index == 0 { HEAD_COLOR } else { BODY_COLOR };
        segments.push(spawn_snake_part(commands, pos, color));
    }

    Snake {
        body,
        segments,
        direction: Direction::Right,
        pending_direction: None,
    }
}

fn sync_snake_visuals(
    snake: &Snake,
    transforms: &mut Query<&mut Transform, With<SnakePart>>,
) {
    for (&entity, &pos) in snake.segments.iter().zip(&snake.body) {
        if let Ok(mut transform) = transforms.get_mut(entity) {
            transform.translation = grid_to_translation(pos);
        }
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    let snake = spawn_initial_snake(&mut commands);
    let food_position = random_food_position(&snake.body);
    let food_entity = spawn_food(&mut commands, food_position);
    let hud = spawn_hud(&mut commands);

    commands.insert_resource(snake);
    commands.insert_resource(Food {
        position: food_position,
        entity: food_entity,
    });
    commands.insert_resource(Score(0));
    commands.insert_resource(hud);
}

// —— 系统 1：读键盘，记录下一次转向 ——
fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    game_state: Res<GameState>,
    mut snake: ResMut<Snake>,
) {
    if *game_state == GameState::GameOver {
        return;
    }

    let new_dir = if keys.just_pressed(KeyCode::ArrowUp) {
        Some(Direction::Up)
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        Some(Direction::Down)
    } else if keys.just_pressed(KeyCode::ArrowLeft) {
        Some(Direction::Left)
    } else if keys.just_pressed(KeyCode::ArrowRight) {
        Some(Direction::Right)
    } else {
        None
    };

    if let Some(dir) = new_dir {
        snake.pending_direction = Some(dir);
    }
}

fn apply_pending_direction(snake: &mut Snake) {
    let Some(next_dir) = snake.pending_direction.take() else {
        return;
    };

    if snake.body.len() == 1 || next_dir != snake.direction.opposite() {
        snake.direction = next_dir;
    }
}

fn end_game(
    commands: &mut Commands,
    game_state: &mut ResMut<GameState>,
    windows: &mut Query<&mut Window, With<PrimaryWindow>>,
    hud: &Hud,
    score: u32,
) {
    **game_state = GameState::GameOver;
    update_game_over_text(commands, hud, score);
    show_game_over_overlay(commands, hud);

    if let Ok(mut window) = windows.single_mut() {
        window.title = GAME_OVER_TITLE.to_string();
    }
}

fn restart_game(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut game_state: ResMut<GameState>,
    mut timer: ResMut<MoveTimer>,
    mut snake: ResMut<Snake>,
    mut food: ResMut<Food>,
    mut score: ResMut<Score>,
    hud: Res<Hud>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if *game_state != GameState::GameOver || !keys.just_pressed(KeyCode::KeyR) {
        return;
    }

    for entity in snake.segments.drain(..) {
        commands.entity(entity).despawn();
    }
    commands.entity(food.entity).despawn();

    let new_snake = spawn_initial_snake(&mut commands);
    let new_food_position = random_food_position(&new_snake.body);
    let new_food_entity = spawn_food(&mut commands, new_food_position);

    *snake = new_snake;
    *food = Food {
        position: new_food_position,
        entity: new_food_entity,
    };
    score.0 = 0;
    *game_state = GameState::Playing;
    timer.0.reset();
    hide_game_over_overlay(&mut commands, &hud);
    update_score_text(&mut commands, &hud, score.0);

    if let Ok(mut window) = windows.single_mut() {
        update_window_title(&mut window, GameState::Playing, score.0);
    }
}

// —— 系统 2：到点后按 direction 移动 ——
fn move_snake(
    time: Res<Time>,
    mut timer: ResMut<MoveTimer>,
    mut game_state: ResMut<GameState>,
    mut commands: Commands,
    mut snake: ResMut<Snake>,
    mut food: ResMut<Food>,
    mut score: ResMut<Score>,
    hud: Res<Hud>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut transforms: ParamSet<(
        Query<&mut Transform, With<SnakePart>>,
        Query<&mut Transform, With<FoodMarker>>,
    )>,
) {
    if *game_state == GameState::GameOver {
        return;
    }

    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    apply_pending_direction(&mut snake);

    let head = snake.body[0];
    let next_head = head + snake.direction.as_ivec();
    let will_grow = next_head == food.position;
    let collision_limit = if will_grow {
        snake.body.len()
    } else {
        snake.body.len().saturating_sub(1)
    };

    if !in_bounds(next_head) || snake.body[..collision_limit].contains(&next_head) {
        end_game(&mut commands, &mut game_state, &mut windows, &hud, score.0);
        return;
    }

    let tail_pos = *snake.body.last().unwrap();
    snake.body.insert(0, next_head);

    if will_grow {
        score.0 += 1;
        let new_tail_entity = spawn_snake_part(&mut commands, tail_pos, BODY_COLOR);
        snake.segments.push(new_tail_entity);
        update_score_text(&mut commands, &hud, score.0);

        food.position = random_food_position(&snake.body);
        if let Ok(mut food_transform) = transforms.p1().get_mut(food.entity) {
            food_transform.translation = grid_to_translation(food.position);
        }

        if let Ok(mut window) = windows.single_mut() {
            update_window_title(&mut window, GameState::Playing, score.0);
        }
    } else {
        snake.body.pop();
    }

    sync_snake_visuals(&snake, &mut transforms.p0());
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: title_for_state(GameState::Playing, 0),
                resolution: WindowResolution::new(
                    WINDOW_WIDTH as u32,
                    WINDOW_HEIGHT as u32,
                ),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(MoveTimer(Timer::from_seconds(
            TICK_SECONDS,
            TimerMode::Repeating,
        )))
        .insert_resource(GameState::Playing)
        .add_systems(Startup, setup)
        .add_systems(Update, (read_input, move_snake, restart_game).chain())
        .run();
}
