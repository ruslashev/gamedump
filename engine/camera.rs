use std::f32::consts::{FRAC_1_SQRT_2, PI};

use glam::{vec3, Mat4, Vec3, Vec4Swizzles};

use crate::input::InputHandler;
use crate::main_loop::DT;
use crate::utils::*;

const PITCH_MIN: f32 = -PI / 2.0 + 0.001;
const PITCH_MAX: f32 = PI / 2.0 - 0.001;

pub struct Camera {
    position: Vec3,
    angles: Vec3,

    fov_y: f32,
    near: f32,
    far: f32,
    aspect_ratio: f32,

    proj: Mat4,
    view: Mat4,
    inverse: Mat4,
    plane_len: f32,

    proj_needs_recalc: bool,
    view_needs_recalc: bool,
}

impl Camera {
    pub fn new(position: Vec3, fov_y_deg: f32, angles: Vec3, aspect_ratio: f32) -> Self {
        let fov_y = fov_y_deg.to_radians();

        Self {
            position,
            angles,
            fov_y,
            near: 0.05,
            far: 100.0,
            aspect_ratio,
            proj: Mat4::IDENTITY,
            view: Mat4::IDENTITY,
            inverse: Mat4::IDENTITY,
            plane_len: calc_plane_len(fov_y, aspect_ratio),
            proj_needs_recalc: true,
            view_needs_recalc: true,
        }
    }

    pub fn proj(&mut self) -> &Mat4 {
        if self.proj_needs_recalc {
            self.recalc_proj_matrix();
        }

        &self.proj
    }

    pub fn view(&mut self) -> &Mat4 {
        if self.view_needs_recalc {
            self.recalc_view_matrix();
        }

        &self.view
    }

    pub fn inverse(&mut self) -> &Mat4 {
        if self.proj_needs_recalc || self.view_needs_recalc {
            if self.proj_needs_recalc {
                self.recalc_proj_matrix();
            }

            if self.view_needs_recalc {
                self.recalc_view_matrix();
            }

            let mult = self.proj * self.view;

            self.inverse = mult.inverse();
        }

        &self.inverse
    }

    pub const fn position(&self) -> Vec3 {
        self.position
    }

    pub fn ang_y(&self) -> f32 {
        self.angles.y
    }

    pub fn plane_len(&self) -> f32 {
        self.plane_len
    }

    pub fn handle_resize(&mut self, w: u32, h: u32) {
        self.aspect_ratio = to_f32(w) / to_f32(h);
        self.plane_len = calc_plane_len(self.fov_y, self.aspect_ratio);
        self.proj_needs_recalc = true;
    }

    pub fn update(&mut self, input: &InputHandler) {
        self.update_angles(input);
        self.update_position(input);
    }

    fn update_angles(&mut self, input: &InputHandler) {
        let mouse_dx = input.mouse_diff_x();
        let mouse_dy = input.mouse_diff_y();

        if mouse_dx == 0 && mouse_dy == 0 {
            return;
        }

        let sensitivity = 4.0;
        let m_yaw = 0.022;
        let m_pitch = 0.022;
        let to_rads = PI / 180.0;

        let mouse_dx = i32_to_f32(mouse_dx);
        let mouse_dy = i32_to_f32(mouse_dy);

        self.angles.y += mouse_dx * m_yaw * sensitivity * to_rads;

        self.angles.x += mouse_dy * m_pitch * sensitivity * to_rads;
        self.angles.x = self.angles.x.clamp(PITCH_MIN, PITCH_MAX);

        self.view_needs_recalc = true;
    }

    fn update_position(&mut self, input: &InputHandler) {
        let forward = input.forward();
        let right = input.right();

        if forward == 0 && right == 0 {
            return;
        }

        if self.view_needs_recalc {
            self.recalc_view_matrix();
        }

        let dir_forward = -self.view.row(2).xyz();
        let dir_right = self.view.row(0).xyz();
        let move_speed = 8.0;

        let mut dir = vec3(0.0, 0.0, 0.0);

        if forward == 1 {
            dir += dir_forward;
        } else if forward == -1 {
            dir -= dir_forward;
        }

        if right == 1 {
            dir += dir_right;
        } else if right == -1 {
            dir -= dir_right;
        }

        if forward != 0 && right != 0 {
            dir.x *= FRAC_1_SQRT_2;
            dir.y *= FRAC_1_SQRT_2;
            dir.z *= FRAC_1_SQRT_2;
        }

        self.position += dir * move_speed * DT;

        self.view_needs_recalc = true;
    }

    fn recalc_view_matrix(&mut self) {
        self.view = Mat4::IDENTITY
            * Mat4::from_euler(glam::EulerRot::XYZ, self.angles.x, self.angles.y, self.angles.z)
            * Mat4::from_translation(-self.position);

        self.view_needs_recalc = false;
    }

    fn recalc_proj_matrix(&mut self) {
        self.proj = Mat4::perspective_rh(self.fov_y, self.aspect_ratio, self.near, self.far);
        self.proj.y_axis.y *= -1.0;

        self.proj_needs_recalc = false;
    }
}

fn calc_plane_len(fov_y: f32, aspect_ratio: f32) -> f32 {
    /* 1. fov_x = 2 * atan(tan(fov_y / 2) * aspect_ratio)
     *
     * 2.   plane
     *    <------^
     *     ^     |
     *      \    |
     *       \   | dir (length 1)
     *        \ θ|
     *         \_|
     *          \|
     *
     * plane = dir (rotated 90°) * plane_len (p)
     *
     * plane_len = tan θ
     * plane_len = tan (fov_x / 2)
     *
     * 3. plane_len = tan ((2 * atan(tan(fov_y / 2) * aspect_ratio)) / 2)
     *    plane_len = tan (atan(tan(fov_y / 2) * aspect_ratio))
     *    plane_len = tan(fov_y / 2) * aspect_ratio
     */

    (fov_y / 2.0).tan() * aspect_ratio
}
