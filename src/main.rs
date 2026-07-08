use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use rand::random_range;

// —— 常量 ——
const CELL_SIZE: f32 = 30.0;
const TICK_SECONDS: f32 = 0.20;
const GRID_WIDTH: i32 = 20;
const GRID_HEIGHT: i32 = 20;
const WINDOW_WIDTH: f32 = GRID_WIDTH as f32 * CELL_SIZE;
const WINDOW_HEIGHT: f32 = GRID_HEIGHT as f32 * CELL_SIZE;

// —— 配色 ——
const BG_LIGHT: Color = Color::srgb(0.67, 0.85, 0.32);
const BG_DARK: Color = Color::srgb(0.63, 0.82, 0.29);
const HEAD_COLOR: Color = Color::srgb(0.20, 0.28, 0.80);
const BODY_COLOR: Color = Color::srgb(0.30, 0.42, 0.90);
const FOOD_COLOR: Color = Color::srgb(0.90, 0.25, 0.25);

const SPRITE_INSET: f32 = 4.0;

// —— 方向 ——
#[derive(Clone, Copy, PartialEq)]
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
struct Food;

#[derive(Component, Clone, Copy)]
struct GridPos(IVec2);

#[derive(Component, Clone, Copy)]
struct PrevGridPos(IVec2);

/// 标记：这是分数文本，供 UI 更新系统定位
#[derive(Component)]
struct ScoreText;

// —— 资源 ——
#[derive(Resource)]
struct MoveTimer(Timer);

#[derive(Resource, Default)]
struct Score(u32);

#[derive(Resource, PartialEq, Eq, Clone, Copy)]
enum GameState {
    Playing,
    GameOver,
}

#[derive(Resource)]
struct Snake {
    body: Vec<Entity>,
    direction: Direction,
    pending: Option<Direction>,
    pending_grow: bool,
}

// —— 消息（Bevy 0.19 里 Event 更名为 Message，强调"数据流"而非回调）——
/// 蛇头刚吃到食物。多个订阅者可以并行处理：加长、加分、生成新食物、播音效等。
#[derive(Message)]
struct AteFoodEvent {
    food_entity: Entity,
    /// 被吃位置——目前未订阅，留给以后的粒子特效/音效订阅者用
    #[allow(dead_code)]
    at: IVec2,
}

/// 蛇死亡:撞墙或撞自己。订阅者处理状态切换、UI 更新、播音效等。
#[derive(Message, Default)]
struct GameOverEvent;

// —— 工具函数 ——
fn grid_to_pixel(pos: IVec2) -> Vec3 {
    let x = (pos.x as f32 - GRID_WIDTH as f32 / 2.0 + 0.5) * CELL_SIZE;
    let y = (pos.y as f32 - GRID_HEIGHT as f32 / 2.0 + 0.5) * CELL_SIZE;
    Vec3::new(x, y, 0.0)
}

fn in_bounds(pos: IVec2) -> bool {
    pos.x >= 0 && pos.x < GRID_WIDTH && pos.y >= 0 && pos.y < GRID_HEIGHT
}

fn random_empty_cell(occupied: &[IVec2]) -> IVec2 {
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
            GridPos(pos),
            PrevGridPos(pos),
            Sprite::from_color(color, Vec2::splat(CELL_SIZE - SPRITE_INSET)),
            Transform::from_translation(grid_to_pixel(pos)),
        ))
        .id()
}

fn spawn_food(commands: &mut Commands, pos: IVec2) {
    commands.spawn((
        Food,
        GridPos(pos),
        Sprite::from_color(FOOD_COLOR, Vec2::splat(CELL_SIZE - SPRITE_INSET - 2.0)),
        Transform::from_translation(grid_to_pixel(pos)),
    ));
}

fn spawn_initial_snake(commands: &mut Commands) -> (Vec<Entity>, Vec<IVec2>) {
    let head = IVec2::new(GRID_WIDTH / 2, GRID_HEIGHT / 2);
    let positions = vec![head, head - IVec2::new(1, 0), head - IVec2::new(2, 0)];
    let mut entities = Vec::with_capacity(positions.len());
    for (i, &pos) in positions.iter().enumerate() {
        let color = if i == 0 { HEAD_COLOR } else { BODY_COLOR };
        entities.push(spawn_snake_part(commands, pos, color));
    }
    (entities, positions)
}

fn set_window_title(window: &mut Window, state: GameState) {
    window.title = match state {
        GameState::Playing => "Bevy Snake".to_string(),
        GameState::GameOver => "Bevy Snake — Game Over (press R to restart)".to_string(),
    };
}

// —— Startup ——
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // 棋盘背景
    for x in 0..GRID_WIDTH {
        for y in 0..GRID_HEIGHT {
            let pos = IVec2::new(x, y);
            let color = if (x + y) % 2 == 0 { BG_LIGHT } else { BG_DARK };
            let px = grid_to_pixel(pos);
            commands.spawn((
                Sprite::from_color(color, Vec2::splat(CELL_SIZE)),
                Transform::from_xyz(px.x, px.y, -1.0),
            ));
        }
    }

    // 初始蛇
    let (entities, positions) = spawn_initial_snake(&mut commands);
    commands.insert_resource(Snake {
        body: entities,
        direction: Direction::Right,
        pending: None,
        pending_grow: false,
    });

    // 食物
    spawn_food(&mut commands, random_empty_cell(&positions));

    // 分数 UI:右上角文本
    commands.spawn((
        ScoreText,
        Text::new("Score: 0"),
        TextFont {
            font_size: FontSize::Px(22.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(14.0),
            ..default()
        },
    ));
}

// —— 系统:读键盘 ——
fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut snake: ResMut<Snake>,
    mut timer: ResMut<MoveTimer>,
) {
    let new_dir = if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) {
        Some(Direction::Up)
    } else if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) {
        Some(Direction::Down)
    } else if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA) {
        Some(Direction::Left)
    } else if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD) {
        Some(Direction::Right)
    } else {
        None
    };

    if let Some(dir) = new_dir
        && dir != snake.direction
        && dir != snake.direction.opposite()
    {
        snake.pending = Some(dir);
        // "eager input"：让 tick 立刻发生，消除按键到响应的延迟
        // 只在还有一段时间才到 tick 时才提前，避免刚好在 tick 边缘重复触发
        let remaining = timer.0.remaining_secs();
        if remaining > TICK_SECONDS * 0.5 {
            let duration = timer.0.duration();
            timer.0.set_elapsed(duration);
        }
    }
}

// —— 系统:到 tick 时前推整条蛇；碰撞则发出 GameOverEvent ——
fn tick_snake(
    time: Res<Time>,
    mut timer: ResMut<MoveTimer>,
    mut commands: Commands,
    mut snake: ResMut<Snake>,
    mut game_over: MessageWriter<GameOverEvent>,
    mut query: Query<(&mut GridPos, &mut PrevGridPos), With<SnakePart>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    if let Some(dir) = snake.pending.take() {
        snake.direction = dir;
    }

    let positions: Vec<IVec2> = snake
        .body
        .iter()
        .filter_map(|&e| query.get(e).ok().map(|(p, _)| p.0))
        .collect();
    if positions.is_empty() {
        return;
    }

    let new_head = positions[0] + snake.direction.as_ivec();
    let old_tail = *positions.last().unwrap();

    // —— 碰撞检测 ——
    // 撞墙
    let out_of_bounds = !in_bounds(new_head);
    // 撞自己：新头位置和"移动后依然存在的身体段"重合
    //   如果不生长：尾巴会让位，所以只查 positions[..len-1]
    //   如果生长：尾巴不让位，要查所有
    let will_hit_self = if snake.pending_grow {
        positions.contains(&new_head)
    } else {
        positions[..positions.len() - 1].contains(&new_head)
    };

    if out_of_bounds || will_hit_self {
        game_over.write(GameOverEvent);
        return;
    }

    // —— 前推所有节 ——
    for (i, &entity) in snake.body.iter().enumerate() {
        if let Ok((mut pos, mut prev)) = query.get_mut(entity) {
            prev.0 = positions[i];
            pos.0 = if i == 0 { new_head } else { positions[i - 1] };
        }
    }

    // —— 生长 ——
    if snake.pending_grow {
        snake.pending_grow = false;
        let entity = spawn_snake_part(&mut commands, old_tail, BODY_COLOR);
        snake.body.push(entity);
    }
}

// —— 系统:检测吃食物,只发事件 ——
fn detect_food_eaten(
    snake: Res<Snake>,
    pos_query: Query<&GridPos, With<SnakePart>>,
    food_query: Query<(Entity, &GridPos), With<Food>>,
    mut events: MessageWriter<AteFoodEvent>,
) {
    let Some(&head_entity) = snake.body.first() else { return };
    let Ok(head_pos) = pos_query.get(head_entity) else { return };
    for (food_entity, food_pos) in &food_query {
        if head_pos.0 == food_pos.0 {
            events.write(AteFoodEvent {
                food_entity,
                at: food_pos.0,
            });
        }
    }
}

// —— 订阅 AteFoodEvent:蛇准备加长 ——
fn grow_snake_on_eat(mut events: MessageReader<AteFoodEvent>, mut snake: ResMut<Snake>) {
    for _ in events.read() {
        snake.pending_grow = true;
    }
}

// —— 订阅 AteFoodEvent:加分 ——
fn award_score_on_eat(mut events: MessageReader<AteFoodEvent>, mut score: ResMut<Score>) {
    for _ in events.read() {
        score.0 += 1;
    }
}

// —— 订阅 AteFoodEvent:清除旧食物、生成新食物 ——
fn respawn_food_on_eat(
    mut events: MessageReader<AteFoodEvent>,
    mut commands: Commands,
    snake: Res<Snake>,
    pos_query: Query<&GridPos, With<SnakePart>>,
) {
    for ev in events.read() {
        commands.entity(ev.food_entity).despawn();
        let occupied: Vec<IVec2> = snake
            .body
            .iter()
            .filter_map(|&e| pos_query.get(e).ok().map(|p| p.0))
            .collect();
        spawn_food(&mut commands, random_empty_cell(&occupied));
    }
}

// —— 订阅 GameOverEvent:切换状态 ——
fn transition_to_game_over(
    mut events: MessageReader<GameOverEvent>,
    mut state: ResMut<GameState>,
) {
    for _ in events.read() {
        *state = GameState::GameOver;
    }
}

// —— 订阅 GameOverEvent:更新窗口标题 ——
fn update_window_on_game_over(
    mut events: MessageReader<GameOverEvent>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    for _ in events.read() {
        if let Ok(mut window) = windows.single_mut() {
            set_window_title(&mut window, GameState::GameOver);
        }
    }
}

// —— 系统:每帧插值 ——
fn interpolate_visual(
    timer: Res<MoveTimer>,
    mut query: Query<(&GridPos, &PrevGridPos, &mut Transform)>,
) {
    let t = (timer.0.elapsed_secs() / timer.0.duration().as_secs_f32()).clamp(0.0, 1.0);
    for (pos, prev, mut transform) in &mut query {
        let from = grid_to_pixel(prev.0);
        let to = grid_to_pixel(pos.0);
        let lerped = from.lerp(to, t);
        transform.translation.x = lerped.x;
        transform.translation.y = lerped.y;
    }
}

// —— 系统:分数变化时刷新 UI 文本 ——
fn update_score_text(score: Res<Score>, mut query: Query<&mut Text, With<ScoreText>>) {
    if !score.is_changed() {
        return;
    }
    for mut text in &mut query {
        text.0 = format!("Score: {}", score.0);
    }
}

// —— 系统:游戏结束时按 R 重开 ——
fn handle_restart(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut snake: ResMut<Snake>,
    mut state: ResMut<GameState>,
    mut timer: ResMut<MoveTimer>,
    mut score: ResMut<Score>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    parts: Query<Entity, With<SnakePart>>,
    foods: Query<Entity, With<Food>>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }

    // 清场
    for e in &parts {
        commands.entity(e).despawn();
    }
    for e in &foods {
        commands.entity(e).despawn();
    }

    // 重建蛇和食物
    let (entities, positions) = spawn_initial_snake(&mut commands);
    snake.body = entities;
    snake.direction = Direction::Right;
    snake.pending = None;
    snake.pending_grow = false;
    spawn_food(&mut commands, random_empty_cell(&positions));

    timer.0.reset();
    score.0 = 0;
    *state = GameState::Playing;
    if let Ok(mut window) = windows.single_mut() {
        set_window_title(&mut window, GameState::Playing);
    }
}

// —— run_if 条件 ——
fn is_playing(state: Res<GameState>) -> bool {
    *state == GameState::Playing
}
fn is_game_over(state: Res<GameState>) -> bool {
    *state == GameState::GameOver
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bevy Snake".to_string(),
                resolution: WindowResolution::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(BG_DARK))
        .insert_resource(GameState::Playing)
        .insert_resource(Score::default())
        .insert_resource(MoveTimer(Timer::from_seconds(
            TICK_SECONDS,
            TimerMode::Repeating,
        )))
        .add_message::<AteFoodEvent>()
        .add_message::<GameOverEvent>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                read_input,
                tick_snake,
                detect_food_eaten,
                grow_snake_on_eat,
                award_score_on_eat,
                respawn_food_on_eat,
                transition_to_game_over,
                update_window_on_game_over,
            )
                .chain()
                .run_if(is_playing),
        )
        .add_systems(Update, interpolate_visual)
        .add_systems(Update, update_score_text)
        .add_systems(Update, handle_restart.run_if(is_game_over))
        .run();
}
