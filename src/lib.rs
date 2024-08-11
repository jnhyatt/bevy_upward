use bevy_app::{App, PostUpdate};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::AnyOf,
    schedule::{IntoSystemConfigs, IntoSystemSetConfigs, SystemSet},
    system::{Commands, Query, Res},
};
use bevy_math::{Dir3, Mat3, Quat};
use bevy_time::Time;
use bevy_transform::{components::Transform, TransformSystem::TransformPropagate};

pub mod prelude {
    pub use super::{local_up_plugin, AlignMode, LocalUp};
}

pub fn local_up_plugin(app: &mut App) {
    app.add_systems(PostUpdate, (align_up, sync_old_up).chain().in_set(AlignUp));
    app.configure_sets(PostUpdate, AlignUp.before(TransformPropagate));
}

/// Determines which direction this entity thinks is up. When this component is present, an entity
/// will try to rotate itself such that its local right vector is perpendicular to the given
/// direction (pointing at the horizon). This is accomplished by rolling the entity such that its
/// look direction is left unchanged.
#[derive(Component)]
pub struct LocalUp(pub Dir3);

/// One-frame delayed view of [`LocalUp`]. This is used to adjust a player to a moving local up
/// direction. For instance, a player walking around a planet will experience a changing up
/// direction. Storing the previous up direction allows us to rotate the player's view such that
/// their local attitude does not change with respect to the horizon. However, this can cause
/// problems with sharp local up changes. In the case of a player moving between areas with distinct
/// up directions, it can be preferable to avoid sudden attitude changes. In these cases, removing
/// this component in tandem with updating [`LocalUp`] will not change the player's look, but will
/// still adjust their roll to match the new up direction.
#[derive(Component)]
pub struct OldUp(pub Dir3);

/// System set for player local up alignment. Alignment happens in [`PostUpdate`] before transform
/// propagation. Player movement should happen before this (usually just in
/// [`Update`](bevy_app::Update)) to prevent invalid rotations from being visible to the user.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlignUp;

/// Interpolation strategy for local up alignment. If this component is not present on an [`Entity`]
/// with a [`LocalUp`] component, the entity will be snapped instantly to the target orientation.
#[derive(Component, Debug, Clone, Copy)]
pub enum AlignMode {
    /// Align to local up linearly with a maximum rotation rate. Much less jarring than immediate
    /// alignment (no `AlignMode` component).
    Linear {
        /// Rotate to target at this angular velocity in radians per second.
        rate: f32,
    },
    /// Align to local up at a rate proportional to the inverse distance to target. Smoother than
    /// [`AlignMode::Linear`], but *technically* never reaches the target (in practice, visible
    /// rotation doesn't persist longer than a few seconds with a reasonable `factor` value).
    Exponential {
        /// Rotate to target at a rate of `-offset * factor` in radians per second. For example,
        /// with a factor of 2, a player 1.5 radians offset from local up will rotate towards local
        /// up at a rate of 3 radians per second. This motion will start out fast and slow over time
        /// as the player orientation reaches its target.
        factor: f32,
    },
}

/// Sync an entity's [`OldUp`] with its [`LocalUp`]. If the entity's [`LocalUp`] has been removed,
/// the [`OldUp`] is also removed. This is used to detect changes to an entity's local up vector to
/// keep relative attitude consistent through frame adjustments.
pub fn sync_old_up(avatars: Query<(Entity, AnyOf<(&LocalUp, &OldUp)>)>, mut commands: Commands) {
    for (e, (local_up, old_up)) in &avatars {
        match (local_up, old_up) {
            (None, None) => unreachable!("AnyOf will yield `Some` for one or both values"),
            (None, Some(_)) => {
                commands.entity(e).remove::<OldUp>();
            }
            (Some(&LocalUp(up)), _) => {
                commands.entity(e).insert(OldUp(up));
            }
        }
    }
}

/// Align avatars with their local up direction. If the avatar's local up has changed since last
/// frame (tracked by means of [`OldUp`]), its transform will be adjusted to keep its view elevation
/// relative to the horizon unchanged. If the avatar did not have a [`LocalUp`] last frame, it will
/// be rolled to align with the new frame, keeping the look direction unchanged. The rate at which
/// this roll is performed is determined by the entity's [`AlignMode`].
pub fn align_up(
    mut avatars: Query<(
        Entity,
        &LocalUp,
        Option<&OldUp>,
        Option<&AlignMode>,
        &mut Transform,
    )>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, local_up, old_up, align_mode, mut transform) in &mut avatars {
        if let Some(old_up) = old_up {
            let rotation = Quat::from_rotation_arc(*old_up.0, *local_up.0);
            transform.rotation = rotation * transform.rotation;
        }
        commands.entity(e).insert(OldUp(local_up.0));
        let Some(new_right) = transform.forward().cross(*local_up.0).try_normalize() else {
            continue;
        };
        let new_up = new_right.cross(*transform.forward());
        let target_rotation =
            Quat::from_mat3(&Mat3::from_cols(new_right, new_up, *transform.local_z()));
        transform.rotation = match align_mode {
            None => target_rotation,
            Some(&AlignMode::Linear { rate }) => {
                // Step rotation towards target_rotation by at most rate * dt radians
                let angle_to_target = transform.rotation.angle_between(target_rotation);
                if angle_to_target == 0.0 {
                    target_rotation
                } else {
                    let delta_angle = angle_to_target.min(rate * time.delta_seconds());
                    let lerp_factor = delta_angle / angle_to_target;
                    transform.rotation.slerp(target_rotation, lerp_factor)
                }
            }
            Some(&AlignMode::Exponential { factor }) => transform
                .rotation
                .slerp(target_rotation, factor * time.delta_seconds()),
        };
    }
}
