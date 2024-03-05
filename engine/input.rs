use crate::utils::pair_to_i32;
use crate::window::Key;

pub struct InputHandler {
    mouse_prev_x: i32,
    mouse_prev_y: i32,

    mouse_diff_x: i32,
    mouse_diff_y: i32,

    forward: i8,
    right: i8,

    key_forward: bool,
    key_right: bool,
    key_back: bool,
    key_left: bool,
    key_up: bool,
}

impl InputHandler {
    pub const fn new(mouse_prev: (f64, f64)) -> Self {
        let (mouse_prev_x, mouse_prev_y) = pair_to_i32(mouse_prev);

        Self {
            mouse_prev_x,
            mouse_prev_y,
            mouse_diff_x: 0,
            mouse_diff_y: 0,
            forward: 0,
            right: 0,
            key_forward: false,
            key_right: false,
            key_back: false,
            key_left: false,
            key_up: false,
        }
    }

    pub fn handle_mouse(&mut self, mouse_pos: (f64, f64)) {
        let (x, y) = pair_to_i32(mouse_pos);

        self.mouse_diff_x = x - self.mouse_prev_x;
        self.mouse_diff_y = y - self.mouse_prev_y;

        self.mouse_prev_x = x;
        self.mouse_prev_y = y;
    }

    pub fn handle_key_press(&mut self, key: Key) {
        match key {
            Key::W => {
                self.key_forward = true;
                self.forward = 1;
            }
            Key::S => {
                self.key_back = true;
                self.forward = -1;
            }
            Key::D => {
                self.key_right = true;
                self.right = 1;
            }
            Key::A => {
                self.key_left = true;
                self.right = -1;
            }
            Key::Space => self.key_up = true,
            _ => (),
        }
    }

    pub fn handle_key_release(&mut self, key: Key) {
        match key {
            Key::W => {
                self.key_forward = false;
                self.forward = -i8::from(self.key_back);
            }
            Key::S => {
                self.key_back = false;
                self.forward = i8::from(self.key_forward);
            }
            Key::D => {
                self.key_right = false;
                self.right = -i8::from(self.key_left);
            }
            Key::A => {
                self.key_left = false;
                self.right = i8::from(self.key_right);
            }
            Key::Space => self.key_up = false,
            _ => (),
        }
    }

    pub fn forward(&self) -> i8 {
        self.forward
    }

    pub fn right(&self) -> i8 {
        self.right
    }

    pub fn mouse_diff_x(&self) -> i32 {
        self.mouse_diff_x
    }

    pub fn mouse_diff_y(&self) -> i32 {
        self.mouse_diff_y
    }
}
