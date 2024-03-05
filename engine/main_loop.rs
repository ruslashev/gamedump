use anyhow::Result;
use glam::vec3;

use crate::camera::Camera;
use crate::input::InputHandler;
use crate::renderer::Renderer;
use crate::utils::to_f32;
use crate::window::{Event, Key, Resolution, Window};
use crate::world::World;

pub const UPDATES_PER_SECOND: i16 = 60;
pub const DT: f32 = 1.0 / (UPDATES_PER_SECOND as f32);
const DT64: f64 = DT as f64;

pub struct MainLoop {
    pub window: Window,
    pub renderer: Renderer,
    pub camera: Camera,
    pub input: InputHandler,
    pub running: bool,
    world: World,
}

impl MainLoop {
    pub fn new(res: Resolution, app_name: &'static str) -> Result<Self> {
        let mut window = Window::new(res, app_name)?;
        let renderer = Renderer::new(app_name, &window)?;
        let world = World::new(256, 128, 256);

        let position = vec3(0.0, 0.0, 0.0);
        let fov_y = 70.0;
        let ang_y = std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_4;
        let angles = vec3(0.0, ang_y, 0.0);
        let aspect_ratio = to_f32(window.width()) / to_f32(window.height());
        let camera = Camera::new(position, fov_y, angles, aspect_ratio);

        let input = InputHandler::new(window.initial_mouse_pos());

        let mut inst = Self {
            window,
            renderer,
            camera,
            input,
            running: true,
            world,
        };

        inst.renderer.add_compute_target(&inst.world);

        Ok(inst)
    }

    pub fn run(&mut self) {
        let mut next_title_update_time = 0.0;
        let mut current_time = self.window.current_time();

        while self.running {
            Window::poll_events(self, Self::handle_event);

            let real_time = self.window.current_time();

            while current_time < real_time {
                current_time += DT64;

                self.update(current_time);
            }

            if self.window.should_close() {
                break;
            }

            self.draw();

            self.update_title(&mut next_title_update_time, real_time);
        }
    }

    pub fn benchmark(&mut self, frames: usize) {
        let mut current_time = self.window.current_time();
        let mut frame = 0;

        while self.running {
            let real_time = self.window.current_time();

            while current_time < real_time {
                current_time += DT64;

                self.update(current_time);
            }

            self.draw();

            frame += 1;

            if frame >= frames {
                break;
            }
        }
    }

    fn handle_event(
        event: Event,
        running: &mut bool,
        win_handle: &mut glfw::Window,
        renderer: &mut Renderer,
        camera: &mut Camera,
        input: &mut InputHandler,
    ) {
        match event {
            Event::KeyPress(Key::Escape) => *running = false,
            Event::KeyPress(Key::T) => Window::center_mouse(win_handle),
            Event::KeyPress(Key::E) => Window::uncenter_mouse(win_handle),
            Event::KeyPress(key) => input.handle_key_press(key),
            Event::KeyRelease(key) => input.handle_key_release(key),
            Event::MouseMove(_mx, _my) => (),
            Event::WindowResize(w, h) => {
                camera.handle_resize(w, h);
                renderer.handle_resize(w, h);
            }
            Event::None => (),
        }
    }

    fn update(&mut self, _current_time: f64) {
        self.input.handle_mouse(self.window.mouse_pos());
        self.camera.update(&self.input);
    }

    fn draw(&mut self) {
        self.renderer.update_data(&mut self.camera, &mut self.world);
        self.renderer.draw();
    }

    fn limit_fps(&mut self, target_fps: f64, real_time: f64) {
        let frame_end = self.window.current_time();
        let frame_time = frame_end - real_time;
        let to_sleep = 1000.0 / target_fps - frame_time;

        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let to_sleep = {
            if to_sleep.is_sign_negative() {
                return;
            }

            std::time::Duration::from_millis(to_sleep as u64)
        };

        std::thread::sleep(to_sleep);
    }

    fn update_title(&mut self, next_title_update_time: &mut f64, frame_start: f64) {
        let title_update_delay = 0.1;
        let frame_end = self.window.current_time();

        if frame_end > *next_title_update_time {
            *next_title_update_time = frame_end + title_update_delay;

            let frame_time = frame_end - frame_start;
            let fps = 1.0 / frame_time;
            let title = format!("game | FPS = {:05.0}, ms = {:05.5}", fps, frame_time * 1000.0);

            self.window.set_title(&title);
        }
    }
}
