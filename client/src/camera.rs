use avian3d::prelude::*;
use bevy::prelude::*;

/// Master game state
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum GameState {
    #[default]
    Playing,
    Paused,
}

/// Resource for global time scaling
#[derive(Resource)]
pub struct GameTimeScale(pub f32);

/// Transition duration in seconds
const TRANSITION_DURATION: f32 = 1.0;

/// Transition state tracking
#[derive(Resource)]
struct CameraTransition {
    active: bool,
    progress: f32,     // 0.0 to 1.0
    from_paused: bool, // true if transitioning from paused to playing
}

impl Default for CameraTransition {
    fn default() -> Self {
        Self {
            active: false,
            progress: 0.0,
            from_paused: false,
        }
    }
}

/// Marker for FreeCam (playing mode, perspective)
#[derive(Component)]
pub struct FreeCam {
    yaw: f32,
    pitch: f32,
    move_speed: f32,
}

/// Marker for BirdsEye cam (paused mode, orthographic)
#[derive(Component)]
struct BirdsEyeCam;

/// Marker for interpolation camera
#[derive(Component)]
struct InterpolationCam;

/// Marker component for pause UI
#[derive(Component)]
struct PauseUI;

/// Marker for the player camera that we control
#[derive(Component)]
pub struct LocalCamera;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .insert_resource(GameTimeScale(1.0))
            .insert_resource(CameraTransition::default())
            .add_systems(Startup, setup_cameras)
            .add_systems(
                Update,
                (
                    handle_pause_input,
                    freecam_controller,
                    update_camera_transition,
                    manage_camera_visibility,
                    manage_physics_pause,
                )
                    .chain(),
            )
            .add_systems(
                OnEnter(GameState::Paused),
                (spawn_pause_ui, start_transition_to_paused),
            )
            .add_systems(
                OnExit(GameState::Paused),
                (despawn_pause_ui, start_transition_to_playing),
            );
    }
}

/// Setup all three cameras
fn setup_cameras(mut commands: Commands) {
    // FreeCam - Perspective, active by default
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(50.0, 30.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
        Projection::Perspective(PerspectiveProjection::default()),
        LocalCamera,
        FreeCam {
            yaw: -std::f32::consts::FRAC_PI_4,
            pitch: -0.6,
            move_speed: 20.0,
        },
    ));

    // BirdsEye - Orthographic, disabled by default
    // Terrain is 100x100, camera at y=150 looking straight down
    let mut ortho_projection = OrthographicProjection::default_3d();
    ortho_projection.near = 0.0;
    ortho_projection.far = 300.0;
    ortho_projection.scale = 0.75; // Scale to show 150x150 world units (100x100 terrain + margins)

    commands.spawn((
        Camera3d::default(),
        Camera {
            is_active: false,
            order: 0, // Render before UI
            clear_color: ClearColorConfig::Default,
            ..default()
        },
        Transform::from_xyz(0.0, 150.0, 0.0).looking_at(Vec3::ZERO, Vec3::Z),
        Projection::Orthographic(ortho_projection),
        BirdsEyeCam,
    ));

    // Interpolation camera - Perspective, disabled by default
    commands.spawn((
        Camera3d::default(),
        Camera {
            is_active: false,
            ..default()
        },
        Transform::from_xyz(50.0, 30.0, 50.0),
        Projection::Perspective(PerspectiveProjection::default()),
        InterpolationCam,
    ));
}

/// Handle pause toggling (Escape key)
fn handle_pause_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut time_scale: ResMut<GameTimeScale>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        match current_state.get() {
            GameState::Paused => {
                next_state.set(GameState::Playing);
                time_scale.0 = 1.0;
            }
            GameState::Playing => {
                next_state.set(GameState::Paused);
                time_scale.0 = 0.0;
            }
        }
    }
}

/// FreeCam controller - WASD movement relative to camera orientation
fn freecam_controller(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    time: Res<Time>,
    mut camera_query: Query<(&mut Transform, &mut FreeCam)>,
    game_state: Res<State<GameState>>,
) {
    // Only control FreeCam when in Playing state
    if *game_state.get() != GameState::Playing {
        return;
    }

    let Ok((mut transform, mut freecam)) = camera_query.single_mut() else {
        return;
    };

    // Mouse rotation (right-click to pan)
    if mouse.pressed(MouseButton::Right) {
        let sensitivity = 0.003;
        for motion in mouse_motion.read() {
            freecam.yaw -= motion.delta.x * sensitivity;
            freecam.pitch -= motion.delta.y * sensitivity;
            freecam.pitch = freecam.pitch.clamp(
                -std::f32::consts::FRAC_PI_2 + 0.1,
                std::f32::consts::FRAC_PI_2 - 0.1,
            );
        }
    }

    // Update rotation from yaw/pitch
    transform.rotation = Quat::from_euler(EulerRot::YXZ, freecam.yaw, freecam.pitch, 0.0);

    // WASD movement relative to camera orientation
    let mut movement = Vec3::ZERO;
    let speed = freecam.move_speed;

    // Get forward/right from camera's current rotation
    let forward = transform.forward();
    let right = transform.right();

    if keyboard.pressed(KeyCode::KeyW) {
        movement += *forward * speed * time.delta_secs();
    }
    if keyboard.pressed(KeyCode::KeyS) {
        movement -= *forward * speed * time.delta_secs();
    }
    if keyboard.pressed(KeyCode::KeyA) {
        movement -= *right * speed * time.delta_secs();
    }
    if keyboard.pressed(KeyCode::KeyD) {
        movement += *right * speed * time.delta_secs();
    }
    if keyboard.pressed(KeyCode::ShiftLeft) {
        movement.y += speed * time.delta_secs();
    }
    if keyboard.pressed(KeyCode::ControlLeft) {
        movement.y -= speed * time.delta_secs();
    }

    transform.translation += movement;
}

/// Start transition from playing to paused
fn start_transition_to_paused(mut transition: ResMut<CameraTransition>) {
    transition.active = true;
    transition.progress = 0.0;
    transition.from_paused = false;
}

/// Start transition from paused to playing
fn start_transition_to_playing(mut transition: ResMut<CameraTransition>) {
    transition.active = true;
    transition.progress = 0.0;
    transition.from_paused = true;
}

/// Update camera transition and interpolation
fn update_camera_transition(
    time: Res<Time>,
    mut transition: ResMut<CameraTransition>,
    freecam_query: Query<
        (&Transform, &Projection),
        (
            With<FreeCam>,
            Without<InterpolationCam>,
            Without<BirdsEyeCam>,
        ),
    >,
    birdseye_query: Query<
        (&Transform, &Projection),
        (
            With<BirdsEyeCam>,
            Without<InterpolationCam>,
            Without<FreeCam>,
        ),
    >,
    mut interp_query: Query<
        (&mut Transform, &mut Projection),
        (
            With<InterpolationCam>,
            Without<FreeCam>,
            Without<BirdsEyeCam>,
        ),
    >,
) {
    if !transition.active {
        return;
    }

    // Update progress
    transition.progress += time.delta_secs() / TRANSITION_DURATION;

    if transition.progress >= 1.0 {
        transition.progress = 1.0;
        transition.active = false;
    }

    // Get source and target transforms and projections
    let Ok((freecam_transform, freecam_proj)) = freecam_query.single() else {
        return;
    };
    let Ok((birdseye_transform, birdseye_proj)) = birdseye_query.single() else {
        return;
    };
    let Ok((mut interp_transform, mut interp_proj)) = interp_query.single_mut() else {
        return;
    };

    // Determine interpolation direction
    let (from_transform, to_transform, from_proj, to_proj) = if transition.from_paused {
        (
            birdseye_transform,
            freecam_transform,
            birdseye_proj,
            freecam_proj,
        )
    } else {
        (
            freecam_transform,
            birdseye_transform,
            freecam_proj,
            birdseye_proj,
        )
    };

    // Interpolate position and rotation
    let t = ease_in_out_cubic(transition.progress);
    interp_transform.translation = from_transform.translation.lerp(to_transform.translation, t);
    interp_transform.rotation = from_transform.rotation.slerp(to_transform.rotation, t);

    // Interpolate projection
    // For simplicity, we'll keep it perspective throughout and just adjust the FOV
    // At t=1.0 when going to orthographic, we'll snap to ortho
    // At t=0.0 when starting from orthographic, we'll snap to perspective
    if t < 0.5 {
        *interp_proj = from_proj.clone();
    } else {
        *interp_proj = to_proj.clone();
    }
}

/// Manage which camera is active based on state and transition
fn manage_camera_visibility(
    transition: Res<CameraTransition>,
    game_state: Res<State<GameState>>,
    mut freecam_query: Query<
        &mut Camera,
        (
            With<FreeCam>,
            Without<InterpolationCam>,
            Without<BirdsEyeCam>,
        ),
    >,
    mut birdseye_query: Query<
        &mut Camera,
        (
            With<BirdsEyeCam>,
            Without<InterpolationCam>,
            Without<FreeCam>,
        ),
    >,
    mut interp_query: Query<
        &mut Camera,
        (
            With<InterpolationCam>,
            Without<FreeCam>,
            Without<BirdsEyeCam>,
        ),
    >,
) {
    let Ok(mut freecam) = freecam_query.single_mut() else {
        return;
    };
    let Ok(mut birdseye) = birdseye_query.single_mut() else {
        return;
    };
    let Ok(mut interp_cam) = interp_query.single_mut() else {
        return;
    };

    if transition.active {
        // During transition, only interpolation camera is active
        freecam.is_active = false;
        birdseye.is_active = false;
        interp_cam.is_active = true;
    } else {
        // After transition, activate the appropriate camera
        interp_cam.is_active = false;
        match game_state.get() {
            GameState::Playing => {
                freecam.is_active = true;
                birdseye.is_active = false;
            }
            GameState::Paused => {
                freecam.is_active = false;
                birdseye.is_active = true;
            }
        }
    }
}

/// Easing function for smooth transitions
fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Spawn pause UI overlay
fn spawn_pause_ui(mut commands: Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
            PauseUI,
            ZIndex(1000),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("PAUSED"),
                TextFont {
                    font_size: 100.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

/// Despawn pause UI when unpausing
fn despawn_pause_ui(mut commands: Commands, ui_query: Query<Entity, With<PauseUI>>) {
    for entity in ui_query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Manage physics pause state based on game state
fn manage_physics_pause(
    game_state: Res<State<GameState>>,
    mut physics_time: ResMut<Time<Physics>>,
) {
    match game_state.get() {
        GameState::Paused => {
            if !physics_time.is_paused() {
                physics_time.pause();
            }
        }
        GameState::Playing => {
            if physics_time.is_paused() {
                physics_time.unpause();
            }
        }
    }
}
