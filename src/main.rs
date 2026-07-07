use bevy::prelude::*;
use bevy::window::WindowResolution;
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

// —— 资源 ——
#[derive(Resource)]
struct MoveTimer(Timer);

#[derive(Resource)]
struct Snake {
    /// 从头到尾的实体列表
    body: Vec<Entity>,
    direction: Direction,
    pending: Option<Direction>,
    /// 下一次 tick 时是否要长一节
    pending_grow: bool,
}

fn grid_to_pixel(pos: IVec2) -> Vec3 {
    let x = (pos.x as f32 - GRID_WIDTH as f32 / 2.0 + 0.5) * CELL_SIZE;
    let y = (pos.y as f32 - GRID_HEIGHT as f32 / 2.0 + 0.5) * CELL_SIZE;
    Vec3::new(x, y, 0.0)
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

    // 初始蛇：3 节水平摆放，头在正中
    let head_pos = IVec2::new(GRID_WIDTH / 2, GRID_HEIGHT / 2);
    let body_positions = vec![
        head_pos,
        head_pos - IVec2::new(1, 0),
        head_pos - IVec2::new(2, 0),
    ];

    let mut body_entities = Vec::with_capacity(body_positions.len());
    for (i, &pos) in body_positions.iter().enumerate() {
        let color = if i == 0 { HEAD_COLOR } else { BODY_COLOR };
        body_entities.push(spawn_snake_part(&mut commands, pos, color));
    }

    commands.insert_resource(Snake {
        body: body_entities,
        direction: Direction::Right,
        pending: None,
        pending_grow: false,
    });

    // 食物
    let food_pos = random_empty_cell(&body_positions);
    commands.spawn((
        Food,
        GridPos(food_pos),
        Sprite::from_color(FOOD_COLOR, Vec2::splat(CELL_SIZE - SPRITE_INSET - 2.0)),
        Transform::from_translation(grid_to_pixel(food_pos)),
    ));
}

// —— 系统：读键盘 ——
fn read_input(keys: Res<ButtonInput<KeyCode>>, mut snake: ResMut<Snake>) {
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
    }
}

// —— 系统：到 tick 时整条蛇前推 ——
fn tick_snake(
    time: Res<Time>,
    mut timer: ResMut<MoveTimer>,
    mut commands: Commands,
    mut snake: ResMut<Snake>,
    mut query: Query<(&mut GridPos, &mut PrevGridPos), With<SnakePart>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    // 应用缓冲的转向
    if let Some(dir) = snake.pending.take() {
        snake.direction = dir;
    }

    // 收集当前所有节的位置（只读借用，用完就还）
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

    // 依次把每一节推到"前一节的旧位置"（头 = 新位置）
    for (i, &entity) in snake.body.iter().enumerate() {
        if let Ok((mut pos, mut prev)) = query.get_mut(entity) {
            prev.0 = positions[i];
            pos.0 = if i == 0 { new_head } else { positions[i - 1] };
        }
    }

    // 如果有 pending_grow，在原尾巴位置新增一节
    if snake.pending_grow {
        snake.pending_grow = false;
        let entity = spawn_snake_part(&mut commands, old_tail, BODY_COLOR);
        snake.body.push(entity);
    }
}

// —— 系统:吃食物 ——
fn eat_food(
    mut commands: Commands,
    mut snake: ResMut<Snake>,
    snake_pos_query: Query<&GridPos, With<SnakePart>>,
    food_query: Query<(Entity, &GridPos), With<Food>>,
) {
    let Some(&head_entity) = snake.body.first() else { return };
    let Ok(head_pos) = snake_pos_query.get(head_entity) else { return };
    let head_grid = head_pos.0;

    // 生成新食物要避开的位置 = 整条蛇
    let occupied: Vec<IVec2> = snake
        .body
        .iter()
        .filter_map(|&e| snake_pos_query.get(e).ok().map(|p| p.0))
        .collect();

    for (food_entity, food_pos) in &food_query {
        if head_grid == food_pos.0 {
            commands.entity(food_entity).despawn();
            snake.pending_grow = true;
            let new_pos = random_empty_cell(&occupied);
            commands.spawn((
                Food,
                GridPos(new_pos),
                Sprite::from_color(FOOD_COLOR, Vec2::splat(CELL_SIZE - SPRITE_INSET - 2.0)),
                Transform::from_translation(grid_to_pixel(new_pos)),
            ));
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
        .insert_resource(MoveTimer(Timer::from_seconds(
            TICK_SECONDS,
            TimerMode::Repeating,
        )))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (read_input, tick_snake, eat_food, interpolate_visual).chain(),
        )
        .run();
}
