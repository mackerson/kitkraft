//! The viewport camera: pure math, no window and no GL, so every invariant is unit-tested.
//!
//! World space is the game's: units as stored in a `REFR`'s position, Z up, right-handed
//! (X east, Y north). The camera orbits a target point: `yaw` is the heading of the view
//! direction in the XY plane (radians from +X toward +Y), `pitch` how far it looks down
//! (radians, positive when the eye is above the target), `distance` from the eye to the
//! target. Orbit, pan, zoom and fly all change these four values; the view and projection
//! matrices are derived from them (OpenGL clip space, depth -1..1).
//!
//! Vector arithmetic is written as method calls (`a.add(b)`): the workspace denies
//! `clippy::arithmetic_side_effects` on operators of non-primitive types, and glam's f32
//! operations cannot overflow or panic.

use std::ops::{Add, Div, Mul, Sub};

use glam::{Mat4, Vec2, Vec3, Vec4};

/// Vertical field of view (radians), 60 degrees. Not observed in the CK.
pub const FOV_Y: f32 = std::f32::consts::FRAC_PI_3;
/// Pitch is kept inside +-89 degrees, so the view direction is never parallel to world up.
pub const PITCH_LIMIT: f32 = 1.553_343;
/// Closest and farthest orbit distance, in game units.
pub const MIN_DISTANCE: f32 = 10.0;
/// See [`MIN_DISTANCE`].
pub const MAX_DISTANCE: f32 = 500_000.0;
/// Radians of orbit per point of right-drag.
pub const ORBIT_PER_POINT: f32 = 0.008;
/// Wheel zoom: the distance is multiplied by `exp(-ZOOM_PER_POINT * scroll)`.
pub const ZOOM_PER_POINT: f32 = 0.002;
/// Fly speed in orbit distances per second (so it suits any scale); shift multiplies it.
pub const FLY_SPEED: f32 = 1.0;
/// See [`FLY_SPEED`].
pub const FLY_FAST: f32 = 4.0;

/// An orbit camera around `target` (module docs for the conventions).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Default for OrbitCamera {
    /// Looking north-east and 30 degrees down at the origin from 1,500 units: +X runs to
    /// the right, +Y away from the viewer, +Z up.
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            yaw: 60f32.to_radians(),
            pitch: 30f32.to_radians(),
            distance: 1_500.0,
        }
    }
}

/// `aspect` (width over height) if usable, else 1.
fn sane_aspect(aspect: f32) -> f32 {
    if aspect.is_finite() && aspect > 1e-3 {
        aspect
    } else {
        1.0
    }
}

impl OrbitCamera {
    /// The unit view direction, from the eye toward the target.
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(cp * cy, cp * sy, -sp)
    }

    /// The unit screen-right direction (horizontal: the camera never rolls).
    pub fn right(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(sy, -cy, 0.0)
    }

    /// The unit screen-up direction.
    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward())
    }

    /// The eye position.
    pub fn eye(&self) -> Vec3 {
        self.target.sub(self.forward().mul(self.distance))
    }

    /// Near and far clip distances, scaled with the orbit distance so the depth buffer
    /// keeps its precision near the target at any zoom.
    pub fn clip(&self) -> (f32, f32) {
        let near = (self.distance * 0.002).clamp(0.5, 100.0);
        let far = (self.distance * 100.0).max(50_000.0);
        (near, far)
    }

    /// World to view space (right-handed, Y up, looking down -Z).
    pub fn view(&self) -> Mat4 {
        glam::camera::rh::view::look_to_mat4(self.eye(), self.forward(), Vec3::Z)
    }

    /// View to OpenGL clip space for a viewport of `aspect` (width over height).
    pub fn projection(&self, aspect: f32) -> Mat4 {
        let (near, far) = self.clip();
        glam::camera::rh::proj::opengl::perspective(FOV_Y, sane_aspect(aspect), near, far)
    }

    /// World to clip space: what the shaders get.
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.projection(aspect).mul_mat4(&self.view())
    }

    /// A world point in normalized device coordinates (x right, y up, z depth, each
    /// -1..1 when visible); `None` when the point is behind the eye.
    pub fn to_ndc(&self, aspect: f32, world: Vec3) -> Option<Vec3> {
        let clip = self.view_proj(aspect).mul_vec4(world.extend(1.0));
        if clip.w <= f32::EPSILON {
            return None;
        }
        Some(clip.truncate().div(clip.w))
    }

    /// The world point at normalized device coordinates `ndc` (the inverse of
    /// [`Self::to_ndc`]).
    pub fn from_ndc(&self, aspect: f32, ndc: Vec3) -> Vec3 {
        // Inverting the two factors separately keeps more precision than inverting
        // their product when the clip range is wide.
        let view_space = self.projection(aspect).inverse().mul_vec4(ndc.extend(1.0));
        let world: Vec4 = self.view().inverse().mul_vec4(view_space);
        if world.w.abs() <= f32::EPSILON {
            return self.target;
        }
        world.truncate().div(world.w)
    }

    /// The picking ray through a point of the viewport given in normalized device
    /// coordinates: its origin on the near plane and its unit direction.
    pub fn ray(&self, aspect: f32, ndc: Vec2) -> (Vec3, Vec3) {
        let near = self.from_ndc(aspect, ndc.extend(-1.0));
        let far = self.from_ndc(aspect, ndc.extend(1.0));
        let dir = far.sub(near).normalize_or(self.forward());
        (near, dir)
    }

    /// Right-drag: turn around the target. Dragging right turns the scene to the right,
    /// dragging down raises the eye. The distance and the target do not change.
    pub fn orbit(&mut self, drag: Vec2) {
        if !drag.is_finite() {
            return;
        }
        self.yaw = (self.yaw - drag.x * ORBIT_PER_POINT).rem_euclid(std::f32::consts::TAU);
        self.pitch = (self.pitch + drag.y * ORBIT_PER_POINT).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Middle-drag: move the target (and the eye with it) in the screen plane, so the
    /// point under the cursor at the target's depth follows the cursor.
    /// `viewport_height` is the viewport's height in the same units as `drag`.
    pub fn pan(&mut self, drag: Vec2, viewport_height: f32) {
        if !drag.is_finite() || !viewport_height.is_finite() || viewport_height < 1.0 {
            return;
        }
        let per_point = 2.0 * self.distance * (FOV_Y * 0.5).tan() / viewport_height;
        let shift = self.up().mul(drag.y).sub(self.right().mul(drag.x));
        self.target = self.target.add(shift.mul(per_point));
    }

    /// Wheel: move toward the target (positive `scroll`, wheel up) or away from it.
    pub fn zoom(&mut self, scroll: f32) {
        if !scroll.is_finite() {
            return;
        }
        self.distance =
            (self.distance * (-scroll * ZOOM_PER_POINT).exp()).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// WASD: move the eye and the target together along the view direction (`ahead`,
    /// +1 for W, -1 for S) and the screen-right direction (`side`, +1 for D, -1 for A)
    /// for `dt` seconds.
    pub fn fly(&mut self, ahead: f32, side: f32, dt: f32, fast: bool) {
        if !(ahead.is_finite() && side.is_finite() && dt.is_finite()) || dt <= 0.0 {
            return;
        }
        let speed = self.distance * FLY_SPEED * if fast { FLY_FAST } else { 1.0 };
        let step = self
            .forward()
            .mul(ahead)
            .add(self.right().mul(side))
            .normalize_or_zero();
        self.target = self.target.add(step.mul(speed * dt.min(0.25)));
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    const ASPECT: f32 = 16.0 / 9.0;

    fn close(a: Vec3, b: Vec3, tol: f32) -> bool {
        (a - b).length() <= tol * (1.0 + a.length().max(b.length()))
    }

    fn cameras() -> Vec<OrbitCamera> {
        let mut out = vec![OrbitCamera::default()];
        for (yaw, pitch, distance) in [(0.0, 0.0, 100.0), (3.0, -1.2, 25.0), (5.5, 1.5, 40_000.0)] {
            out.push(OrbitCamera {
                target: Vec3::new(1_000.0, -2_048.0, 300.0),
                yaw,
                pitch,
                distance,
            });
        }
        out
    }

    #[test]
    fn view_and_projection_round_trip() {
        for cam in cameras() {
            for p in [
                cam.target,
                cam.target + Vec3::new(0.3, -0.2, 0.1) * cam.distance,
                cam.target + (cam.right() * 0.4 + cam.up() * 0.2) * cam.distance,
            ] {
                let ndc = cam.to_ndc(ASPECT, p).unwrap();
                assert!(ndc.abs().max_element() <= 1.0, "{ndc:?} not visible");
                let back = cam.from_ndc(ASPECT, ndc);
                // f32 depth near the far end of the range: within 1e-4 of the orbit
                // distance (0.15 units at the default 1,500).
                let err = (back - p).length();
                assert!(err <= cam.distance * 1e-4, "{p:?} -> {ndc:?} -> {back:?}");
            }
        }
    }

    #[test]
    fn the_target_is_at_the_center_and_the_eye_at_its_distance() {
        for cam in cameras() {
            let ndc = cam.to_ndc(ASPECT, cam.target).unwrap();
            assert!(ndc.x.abs() < 1e-4 && ndc.y.abs() < 1e-4, "{ndc:?}");
            assert!(((cam.eye() - cam.target).length() - cam.distance).abs() < 1e-2);
            assert!(cam
                .to_ndc(ASPECT, cam.eye() - cam.forward() * 10.0)
                .is_none());
        }
    }

    /// Inverted axes would show here: +X to the right and +Z up on screen, +Y away from
    /// the viewer, for the default camera.
    #[test]
    fn world_axes_point_the_right_way_on_screen() {
        let cam = OrbitCamera::default();
        let o = cam.to_ndc(ASPECT, Vec3::ZERO).unwrap();
        let x = cam.to_ndc(ASPECT, Vec3::X * 100.0).unwrap();
        let z = cam.to_ndc(ASPECT, Vec3::Z * 100.0).unwrap();
        let y = cam.to_ndc(ASPECT, Vec3::Y * 100.0).unwrap();
        assert!(x.x > o.x, "+X should go right: {x:?}");
        assert!(z.y > o.y, "+Z should go up: {z:?}");
        assert!(y.z > o.z, "+Y should go away from the viewer: {y:?}");
        // Looking straight north from the south: +X exactly right, +Y straight ahead.
        let north = OrbitCamera {
            yaw: std::f32::consts::FRAC_PI_2,
            pitch: 0.0,
            ..OrbitCamera::default()
        };
        assert!(close(north.right(), Vec3::X, 1e-6));
        assert!(close(north.forward(), Vec3::Y, 1e-6));
        assert!(close(north.up(), Vec3::Z, 1e-6));
    }

    #[test]
    fn the_aspect_ratio_stretches_x_only() {
        let cam = OrbitCamera {
            pitch: 0.0,
            ..OrbitCamera::default()
        };
        let p = cam.target + cam.right() * 100.0 + cam.up() * 100.0;
        let square = cam.to_ndc(1.0, p).unwrap();
        let wide = cam.to_ndc(2.0, p).unwrap();
        assert!((square.y - wide.y).abs() < 1e-5);
        assert!((square.x - 2.0 * wide.x).abs() < 1e-5);
        // A bad aspect falls back to square instead of producing NaN.
        assert_eq!(cam.to_ndc(0.0, p), cam.to_ndc(1.0, p));
        assert_eq!(cam.to_ndc(f32::NAN, p), cam.to_ndc(1.0, p));
    }

    #[test]
    fn orbit_keeps_target_and_distance_and_clamps_pitch() {
        for mut cam in cameras() {
            let (target, distance) = (cam.target, cam.distance);
            for drag in [
                Vec2::new(40.0, 0.0),
                Vec2::new(-300.0, 25.0),
                Vec2::new(0.0, 10_000.0),
                Vec2::new(0.0, -10_000.0),
            ] {
                cam.orbit(drag);
                assert_eq!(cam.target, target);
                assert_eq!(cam.distance, distance);
                assert!(((cam.eye() - target).length() - distance).abs() < distance * 1e-4);
                assert!(cam.pitch.abs() <= PITCH_LIMIT);
                assert!((0.0..std::f32::consts::TAU).contains(&cam.yaw));
                // Never looking straight up or down: the view matrix stays finite.
                assert!(cam.forward().cross(Vec3::Z).length() > 1e-3);
                assert!(cam.view().is_finite());
            }
            let before = cam;
            cam.orbit(Vec2::new(f32::NAN, 3.0));
            assert_eq!(cam, before);
        }
    }

    /// Turntable feel: the near side of the scene (between the eye and the target)
    /// follows the mouse.
    #[test]
    fn orbit_turns_the_scene_with_the_mouse() {
        let mut cam = OrbitCamera::default();
        let p = cam.target - cam.forward() * 500.0;
        let before = cam.to_ndc(ASPECT, p).unwrap();
        cam.orbit(Vec2::new(10.0, 0.0));
        let after = cam.to_ndc(ASPECT, p).unwrap();
        assert!(after.x > before.x, "{before:?} -> {after:?}");
        let pitch = cam.pitch;
        cam.orbit(Vec2::new(0.0, 10.0));
        assert!(cam.pitch > pitch && cam.eye().z > 0.0);
    }

    #[test]
    fn pan_moves_target_and_eye_together_in_the_screen_plane() {
        for mut cam in cameras() {
            let (eye, target) = (cam.eye(), cam.target);
            let fwd = cam.forward();
            cam.pan(Vec2::new(30.0, -12.0), 600.0);
            let moved = cam.target - target;
            assert!(moved.length() > 0.0);
            assert!(moved.dot(fwd).abs() < 1e-3 * moved.length());
            assert!(close(cam.eye() - eye, moved, 1e-4));
            assert_eq!(cam.forward(), fwd);
        }
        // The point under the cursor follows it: drag right by a quarter of the height
        // and the old target moves right by half an NDC unit (NDC spans 2 per height).
        let mut cam = OrbitCamera::default();
        let old = cam.target;
        cam.pan(Vec2::new(150.0, 0.0), 600.0);
        let ndc = cam.to_ndc(1.0, old).unwrap();
        assert!((ndc.x - 0.5).abs() < 1e-3, "{ndc:?}");
        assert!(ndc.y.abs() < 1e-3, "{ndc:?}");
        let before = cam;
        cam.pan(Vec2::new(5.0, 5.0), 0.0);
        assert_eq!(cam, before);
    }

    #[test]
    fn zoom_keeps_target_and_direction_within_limits() {
        let mut cam = OrbitCamera::default();
        let (target, fwd) = (cam.target, cam.forward());
        cam.zoom(120.0);
        assert!(cam.distance < 1_500.0);
        cam.zoom(-240.0);
        assert!(cam.distance > 1_500.0);
        cam.zoom(1e9);
        assert_eq!(cam.distance, MIN_DISTANCE);
        cam.zoom(-1e9);
        assert_eq!(cam.distance, MAX_DISTANCE);
        cam.zoom(f32::INFINITY);
        assert_eq!(cam.distance, MAX_DISTANCE);
        assert_eq!((cam.target, cam.forward()), (target, fwd));
        let (near, far) = cam.clip();
        assert!(near > 0.0 && far > near * 100.0);
    }

    #[test]
    fn fly_moves_eye_and_target_together() {
        let mut cam = OrbitCamera::default();
        let offset = cam.eye() - cam.target;
        let start = cam.target;
        cam.fly(1.0, 0.0, 0.1, false);
        let step = cam.target - start;
        assert!(close(step.normalize(), cam.forward(), 1e-5));
        assert!((step.length() - cam.distance * FLY_SPEED * 0.1).abs() < 1e-2);
        assert!(close(cam.eye() - cam.target, offset, 1e-5));
        let at = cam.target;
        cam.fly(0.0, -1.0, 0.1, true);
        assert!(close((cam.target - at).normalize(), -cam.right(), 1e-5));
        let at = cam.target;
        cam.fly(1.0, 1.0, -1.0, false);
        cam.fly(f32::NAN, 0.0, 0.1, false);
        assert_eq!(cam.target, at);
    }

    #[test]
    fn the_center_ray_passes_through_the_target() {
        for cam in cameras() {
            let (origin, dir) = cam.ray(ASPECT, Vec2::ZERO);
            assert!((dir.length() - 1.0).abs() < 1e-5);
            let to_target = cam.target - origin;
            let off_ray = to_target - dir * to_target.dot(dir);
            assert!(off_ray.length() < 1e-3 * cam.distance, "{off_ray:?}");
        }
    }
}
