use bevy::prelude::*;

const CELL_SIZE: f32 = 30.0;
const TICK_SECONDS: f32 = 0.2;

// —— 方向枚举 ——
#[derive(Clone, Copy, PartialEq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    /// 该方向的"单位向量"（+1 格）
    fn as_vec(self) -> Vec2 {
        match self {
            Direction::Up => Vec2::new(0.0, 1.0),
            Direction::Down => Vec2::new(0.0, -1.0),
            Direction::Left => Vec2::new(-1.0, 0.0),
            Direction::Right => Vec2::new(1.0, 0.0),
        }
    }

    /// 相反方向（用于禁止 180° 掉头）
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
}

// —— 资源 ——
#[derive(Resource)]
struct MoveTimer(Timer);

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        SnakeHead {
            direction: Direction::Right,
        },
        Sprite::from_color(Color::WHITE, Vec2::splat(CELL_SIZE)),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

// —— 系统 1：读键盘，更新蛇头 direction ——
fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut SnakeHead>,
) {
    let Ok(mut head) = query.single_mut() else {
        return;
    };

    let new_dir = if keys.pressed(KeyCode::ArrowUp) {
        Some(Direction::Up)
    } else if keys.pressed(KeyCode::ArrowDown) {
        Some(Direction::Down)
    } else if keys.pressed(KeyCode::ArrowLeft) {
        Some(Direction::Left)
    } else if keys.pressed(KeyCode::ArrowRight) {
        Some(Direction::Right)
    } else {
        None
    };

    // 允许改方向，但不能 180° 掉头
    if let Some(dir) = new_dir
        && dir != head.direction.opposite()
    {
        head.direction = dir;
    }
}

// —— 系统 2：到点后按 direction 移动 ——
fn move_snake(
    time: Res<Time>,
    mut timer: ResMut<MoveTimer>,
    mut query: Query<(&SnakeHead, &mut Transform)>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        for (head, mut transform) in &mut query {
            let step = head.direction.as_vec() * CELL_SIZE;
            transform.translation.x += step.x;
            transform.translation.y += step.y;
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(MoveTimer(Timer::from_seconds(
            TICK_SECONDS,
            TimerMode::Repeating,
        )))
        .add_systems(Startup, setup)
        .add_systems(Update, (read_input, move_snake).chain())
        .run();
}
