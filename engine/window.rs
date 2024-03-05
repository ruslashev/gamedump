use std::mem::MaybeUninit;
use std::ptr;
use std::sync::mpsc::Receiver;

use anyhow::{bail, Context, Result};
use ash::vk;
pub use glfw::Key;
use log::warn;

use crate::camera::Camera;
use crate::input::InputHandler;
use crate::main_loop::MainLoop;
use crate::renderer::Renderer;
use crate::utils::to_i32;

pub struct Window {
    pub(super) glfw: glfw::Glfw,
    pub(super) handle: glfw::Window,
    pub(super) events: EventRx,
    pub(super) width: u32,
    pub(super) height: u32,
}

#[derive(Clone, Copy)]
pub enum Resolution {
    Windowed(u32, u32),
    BorderlessFullscreen,
    Fullscreen,
    FullscreenWithRes(u32, u32),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Event {
    KeyPress(Key),
    KeyRelease(Key),
    MouseMove(f64, f64),
    WindowResize(u32, u32),
    None,
}

type EventRx = Receiver<(f64, glfw::WindowEvent)>;
type EventCb =
    fn(Event, &mut bool, &mut glfw::Window, &mut Renderer, &mut Camera, &mut InputHandler);

impl Window {
    pub fn new(res: Resolution, title: &str) -> Result<Self> {
        let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).context("Failed to initialize GLFW")?;

        if !glfw.vulkan_supported() {
            bail!("Vulkan not supported");
        }

        glfw.window_hint(glfw::WindowHint::Visible(true));
        glfw.window_hint(glfw::WindowHint::ClientApi(glfw::ClientApiHint::NoApi));
        glfw.window_hint(glfw::WindowHint::CenterCursor(true));

        let (width, height, mut handle, events) =
            create_window(&mut glfw, res, title).context("Failed to create window")?;

        if center_window(&mut handle, res).is_err() {
            warn!("Failed to center window");
        }

        handle.set_key_polling(true);
        handle.set_cursor_pos_polling(true);
        handle.set_size_polling(true);

        handle.set_cursor_mode(glfw::CursorMode::Disabled);

        if glfw.supports_raw_motion() {
            handle.set_raw_mouse_motion(true);
        }

        let inst = Self {
            glfw,
            handle,
            events,
            width,
            height,
        };

        Ok(inst)
    }

    pub fn get_required_extensions(&self) -> Vec<String> {
        self.glfw.get_required_instance_extensions().expect("Vulkan API unavaliable")
    }

    pub fn create_surface(&self, instance: &ash::Instance) -> Result<vk::SurfaceKHR> {
        let mut surface = MaybeUninit::uninit();

        self.handle
            .create_window_surface(instance.handle(), ptr::null(), surface.as_mut_ptr())
            .result()
            .context("Failed to create window surface")?;

        let surface = unsafe { surface.assume_init() };

        Ok(surface)
    }

    pub fn current_time(&self) -> f64 {
        self.glfw.get_time()
    }

    pub fn block_until_event(&mut self) {
        self.glfw.wait_events();
    }

    pub fn set_title(&mut self, title: &str) {
        self.handle.set_title(title);
    }

    pub fn center_mouse(handle: &mut glfw::Window) {
        handle.set_cursor_mode(glfw::CursorMode::Disabled);
    }

    pub fn uncenter_mouse(handle: &mut glfw::Window) {
        handle.set_cursor_mode(glfw::CursorMode::Normal);
    }

    pub fn mouse_pos(&self) -> (f64, f64) {
        self.handle.get_cursor_pos()
    }

    pub fn initial_mouse_pos(&mut self) -> (f64, f64) {
        let (mut x, mut y) = self.handle.get_cursor_pos();

        self.glfw.poll_events();

        for (_, event) in glfw::flush_messages(&self.events) {
            if let glfw::WindowEvent::CursorPos(nx, ny) = event {
                x = nx;
                y = ny;
            }
        }

        (x, y)
    }

    pub fn should_close(&self) -> bool {
        self.handle.should_close()
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn poll_events(main_loop: &mut MainLoop, handle_event: EventCb) {
        let running = &mut main_loop.running;
        let handle = &mut main_loop.window.handle;
        let renderer = &mut main_loop.renderer;
        let camera = &mut main_loop.camera;
        let input = &mut main_loop.input;

        main_loop.window.glfw.poll_events();

        for (_, glfw_event) in glfw::flush_messages(&main_loop.window.events) {
            let event = match glfw_event {
                glfw::WindowEvent::Key(key, _scancode, action, _modifiers) => match action {
                    glfw::Action::Release => Event::KeyRelease(key),
                    glfw::Action::Repeat => Event::None,
                    glfw::Action::Press => Event::KeyPress(key),
                },
                glfw::WindowEvent::CursorPos(x, y) => Event::MouseMove(x, y),
                glfw::WindowEvent::Size(w, h) => {
                    let w = w.unsigned_abs();
                    let h = h.unsigned_abs();

                    main_loop.window.width = w;
                    main_loop.window.height = h;

                    Event::WindowResize(w, h)
                }
                _ => Event::None,
            };

            if event != Event::None {
                handle_event(event, running, handle, renderer, camera, input);
            }
        }
    }
}

fn create_window(
    glfw: &mut glfw::Glfw,
    res: Resolution,
    title: &str,
) -> Result<(u32, u32, glfw::Window, EventRx)> {
    let (width, height, win) = match res {
        Resolution::Windowed(width, height) => {
            let win = glfw.create_window(width, height, title, glfw::WindowMode::Windowed);

            (width, height, win)
        }
        Resolution::BorderlessFullscreen => {
            let monitor = glfw::Monitor::from_primary();
            let vid_mode = monitor.get_video_mode().context("Failed to get video mode")?;
            let width = vid_mode.width;
            let height = vid_mode.height;
            let mode = glfw::WindowMode::Windowed;

            glfw.window_hint(glfw::WindowHint::Decorated(false));
            glfw.window_hint(glfw::WindowHint::Resizable(false));
            glfw.window_hint(glfw::WindowHint::Maximized(true));

            let win = glfw.create_window(width, height, title, mode);

            (width, height, win)
        }
        Resolution::Fullscreen => {
            let monitor = glfw::Monitor::from_primary();
            let vid_mode = monitor.get_video_mode().context("Failed to get video mode")?;
            let width = vid_mode.width;
            let height = vid_mode.height;
            let mode = glfw::WindowMode::FullScreen(&monitor);
            let win = glfw.create_window(width, height, title, mode);

            (width, height, win)
        }
        Resolution::FullscreenWithRes(width, height) => {
            let monitor = glfw::Monitor::from_primary();
            let mode = glfw::WindowMode::FullScreen(&monitor);
            let win = glfw.create_window(width, height, title, mode);

            (width, height, win)
        }
    };

    match win {
        Some((handle, events)) => Ok((width, height, handle, events)),
        None => bail!("Failed to create window"),
    }
}

fn center_window(handle: &mut glfw::Window, res: Resolution) -> Result<()> {
    if let Resolution::Windowed(win_sx, win_sy) = res {
        let monitor = glfw::Monitor::from_primary();
        let vid_mode = monitor.get_video_mode().context("Failed to get video mode")?;

        let scr_hx = to_i32(vid_mode.width) / 2;
        let scr_hy = to_i32(vid_mode.height) / 2;
        let win_hx = to_i32(win_sx) / 2;
        let win_hy = to_i32(win_sy) / 2;

        let win_x = scr_hx - win_hx;
        let win_y = scr_hy - win_hy;

        handle.set_pos(win_x, win_y);
    }

    Ok(())
}
