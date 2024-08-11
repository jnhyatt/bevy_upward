use bevy::{
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};
use bevy_upward::prelude::*;
use leafwing_input_manager::prelude::*;
use std::f32::consts::TAU;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            local_up_plugin,
            InputManagerPlugin::<Movement>::default(),
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, (movement, set_local_up))
        .run();
}

fn setup(
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut window = window.single_mut();
    window.cursor.visible = false;
    window.cursor.grab_mode = CursorGrabMode::Locked;
    let planet_mesh = meshes.add(Sphere::new(10.0).mesh().ico(20).unwrap());
    let material = materials.add(StandardMaterial::default());
    commands.spawn(PbrBundle {
        mesh: planet_mesh,
        material: material.clone(),
        ..default()
    });
    commands.spawn((
        AlignToGravity,
        AlignMode::Exponential { factor: 2.0 },
        InputManagerBundle::with_map(
            InputMap::default()
                .with_dual_axis(Movement::Planar, KeyboardVirtualDPad::WASD)
                .with_axis(
                    Movement::Vertical,
                    KeyboardVirtualAxis::new(KeyCode::KeyF, KeyCode::KeyR),
                )
                .with_axis(
                    Movement::Pitch,
                    MouseMoveAxis::Y.inverted().sensitivity(0.2),
                )
                .with_axis(Movement::Yaw, MouseMoveAxis::X.sensitivity(0.2))
                .with_axis(
                    Movement::Roll,
                    KeyboardVirtualAxis::new(KeyCode::KeyQ, KeyCode::KeyE),
                ),
        ),
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 0.0, 30.0),
            ..default()
        },
    ));
    commands.spawn(DirectionalLightBundle {
        transform: Transform::IDENTITY.looking_to(Vec3::new(1.0, -10.0, -5.0), Vec3::Y),
        ..default()
    });
}

#[derive(Component)]
struct AlignToGravity;

/// Aligns the player's local up with the closest planet's gravity.
fn set_local_up(
    players: Query<(Entity, &Transform), With<AlignToGravity>>,
    mut commands: Commands,
) {
    for (e, transform) in &players {
        let (up, distance) = Dir3::new_and_length(transform.translation).unwrap();
        if distance < 20.0 {
            commands.entity(e).insert(LocalUp(up));
        } else {
            commands.entity(e).remove::<LocalUp>();
        }
    }
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Movement {
    Planar,
    Vertical,
    Pitch,
    Yaw,
    Roll,
}

impl Actionlike for Movement {
    fn input_control_kind(&self) -> InputControlKind {
        match self {
            Movement::Planar => InputControlKind::DualAxis,
            Movement::Vertical => InputControlKind::Axis,
            Movement::Pitch => InputControlKind::Axis,
            Movement::Yaw => InputControlKind::Axis,
            Movement::Roll => InputControlKind::Axis,
        }
    }
}

fn movement(
    mut players: Query<(&ActionState<Movement>, &mut Transform, Option<&LocalUp>)>,
    time: Res<Time>,
) {
    for (actions, mut transform, local_up) in &mut players {
        let planar = actions.clamped_axis_pair(&Movement::Planar);
        let vertical = actions.clamped_value(&Movement::Vertical);
        let movement = planar.xy().extend(vertical).xzy() * Vec3::new(1.0, 1.0, -1.0);
        let move_speed = 5.0;
        let new_translation = transform.rotation * (movement * move_speed * time.delta_seconds());
        transform.translation += new_translation;
        let pitch = actions.value(&Movement::Pitch);
        let yaw = actions.value(&Movement::Yaw);
        let roll = actions.value(&Movement::Roll);
        let rotation_input = -Vec3::new(pitch, yaw, roll) * time.delta_seconds();
        let rotation = transform.rotation;
        transform.rotation = if let Some(up) = local_up {
            let pitch_angle = transform.local_z().dot(*up.0).acos();
            let clamped_pitch =
                (pitch_angle + rotation_input.x).clamp(TAU * 0.01, TAU * 0.49) - pitch_angle;
            Quat::from_axis_angle(*up.0, rotation_input.y)
                * Quat::from_axis_angle(*transform.local_x(), clamped_pitch)
                * rotation
        } else {
            rotation * Quat::from_scaled_axis(rotation_input)
        };
    }
}
