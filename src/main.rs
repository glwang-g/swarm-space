use bevy::prelude::*;
use bevy::window::WindowResolution;

// —— 常量 ——
const CELL_SIZE: f32 = 30.0;
const TICK_SECONDS: f32 = 0.12;      // 缩短：手感更跟手
const GRID_WIDTH: i32 = 20;
const GRID_HEIGHT: i32 = 20;
const WINDOW_WIDTH: f32 = GRID_WIDTH as f32 * CELL_SIZE;
const WINDOW_HEIGHT: f32 = GRID_HEIGHT as f32 * CELL_SIZE;

// —— 配色 ——
const BG_COLOR: Color = Color::srgb(0.09, 0.10, 0.13);      // 深灰蓝背景
const HEAD_COLOR: Color = Color::srgb(0.95, 0.95, 0.95);    // 蛇头亮白

// —— 方向枚举 ——
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
struct SnakeHead {
    direction: Direction,
    /// 缓冲下一次要转向的方向（防止两次 tick 之间连按丢失）
    pending: Option<Direction>,
}

/// 网格坐标（逻辑位置）
#[derive(Component, Clone, Copy)]
struct GridPos(IVec2);

/// 上一格坐标，用于插值起点
#[derive(Component, Clone, Copy)]
struct PrevGridPos(IVec2);

// —— 资源 ——
#[derive(Resource)]
struct MoveTimer(Timer);

/// 把网格坐标转成屏幕像素中心
fn grid_to_pixel(pos: IVec2) -> Vec3 {
    let x = (pos.x as f32 - GRID_WIDTH as f32 / 2.0 + 0.5) * CELL_SIZE;
    let y = (pos.y as f32 - GRID_HEIGHT as f32 / 2.0 + 0.5) * CELL_SIZE;
    Vec3::new(x, y, 0.0)
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    let start = IVec2::new(GRID_WIDTH / 2, GRID_HEIGHT / 2);
    commands.spawn((
        SnakeHead {
            direction: Direction::Right,
            pending: None,
        },
        GridPos(start),
        PrevGridPos(start),
        // Sprite 稍小于格子，留出"网格缝"
        Sprite::from_color(HEAD_COLOR, Vec2::splat(CELL_SIZE - 2.0)),
        Transform::from_translation(grid_to_pixel(start)),
    ));
}

// —— 系统 1：读键盘，写入 pending ——
fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut SnakeHead>,
) {
    let Ok(mut head) = query.single_mut() else { return; };

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
        && dir != head.direction
        && dir != head.direction.opposite()
    {
        head.pending = Some(dir);
    }
}

// —— 系统 2：到 tick 时更新逻辑网格坐标 ——
fn tick_snake(
    time: Res<Time>,
    mut timer: ResMut<MoveTimer>,
    mut query: Query<(&mut SnakeHead, &mut GridPos, &mut PrevGridPos)>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    for (mut head, mut pos, mut prev) in &mut query {
        // 消费 pending
        if let Some(dir) = head.pending.take() {
            head.direction = dir;
        }
        prev.0 = pos.0;
        pos.0 = pos.0 + head.direction.as_ivec();
    }
}

// —— 系统 3：每帧插值，把 Transform 从上一格平滑挪到当前格 ——
fn interpolate_visual(
    timer: Res<MoveTimer>,
    mut query: Query<(&GridPos, &PrevGridPos, &mut Transform)>,
) {
    // 0.0 = 刚 tick 完（在上一格），1.0 = 到达当前格
    let t = timer.0.elapsed_secs() / timer.0.duration().as_secs_f32();
    let t = t.clamp(0.0, 1.0);

    for (pos, prev, mut transform) in &mut query {
        let from = grid_to_pixel(prev.0);
        let to = grid_to_pixel(pos.0);
        transform.translation = from.lerp(to, t);
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
        .insert_resource(ClearColor(BG_COLOR))
        .insert_resource(MoveTimer(Timer::from_seconds(
            TICK_SECONDS,
            TimerMode::Repeating,
        )))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (read_input, tick_snake, interpolate_visual).chain(),
        )
        .run();
}
