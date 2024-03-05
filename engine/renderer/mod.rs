mod mesh;
mod pipeline;
mod render_pass;
mod vulkan;

use std::default::Default;
use std::mem::{size_of, ManuallyDrop};

use anyhow::Result;
use ash::extensions::ext::DebugUtils;
use ash::vk;
use glam::{vec2, vec3, Mat4, Vec2, Vec3};

use self::mesh::*;
use self::render_pass::*;
use self::vulkan::*;
use crate::camera::Camera;
use crate::image::Image;
use crate::utils::*;
use crate::window::Window;
use crate::world::{self, World};

macro_rules! include_shader {
    ($name:literal) => {
        include_bytes!(concat!("../../target/shaders/", $name, ".spv"))
    };
}

const FRAMES_IN_FLIGHT: u16 = 2;

pub const SIZE_F32: u32 = to_u32(size_of::<f32>());

pub const DRAW_TIMEOUT_NS: u64 = 5 * 1000 * 1000 * 1000;

pub struct Renderer {
    instance: ash::Instance,
    debug_data: Option<DebugData>,
    surface: ManuallyDrop<Surface>,
    phys_device_info: PhysDeviceInfo,
    device: ash::Device,
    device_mem_properties: vk::PhysicalDeviceMemoryProperties,
    queues: Queues,
    swapchain: Swapchain,
    viewport: vk::Viewport,
    scissor: vk::Rect2D,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    depth_format: vk::Format,
    depth_textures: Vec<FramebufferAttachment>,
    attachments: Vec<FramebufferAttachment>,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
    image_available: Vec<vk::Semaphore>,
    render_finished: Vec<vk::Semaphore>,
    is_rendering: Vec<vk::Fence>,
    texture: ManuallyDrop<Texture>,
    compute_target: Option<ComputeTarget>,
    compute_target_mesh: Option<MeshData>,
    meshes: Vec<MeshData>,
    current_frame: usize,
    per_frame_copies: usize,
    win_width: u32,
    win_height: u32,
    win_resized: bool,
}

pub struct DebugData {
    debug_utils_loader: DebugUtils,
    debug_messenger: vk::DebugUtilsMessengerEXT,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModelViewProjUBO {
    model: Mat4,
    view: Mat4,
    proj: Mat4,
}

#[repr(C, packed)]
#[derive(Default)]
pub struct RayCastPushConstants {
    pos: Vec3,
    pad1: f32,
    dir: Vec2,
    plane: Vec2,
    world_size_x: u32,
    world_size_y: u32,
    world_size_z: u32,
    pad2: f32,
}

#[repr(C, packed)]
pub struct SkyboxPushConstants {
    inv: Mat4,
    pos: Vec3,
    _pad1: f32,
    res: Vec2,
    _pad2: [f32; 2],
}

#[repr(C, packed)]
pub struct CrosshairPushConstants {
    color: Vec3,
    _pad: f32,
    res: Vec2,
}

#[repr(C, packed)]
pub struct RayTracePushConstants {
    color: Vec3,
    _pad: f32,
}

impl Renderer {
    #[allow(clippy::too_many_lines)]
    pub fn new(app_name: &'static str, window: &Window) -> Result<Self> {
        let entry = ash::Entry::linked();
        let instance = create_instance(app_name, &entry, window);
        let debug_data = create_debug_data(&entry, &instance);
        let surface = ManuallyDrop::new(Surface::new(&entry, &instance, window)?);
        let phys_device_info = pick_phys_device(&instance, &surface);
        let phys_device = phys_device_info.phys_device;
        let device_mem_properties =
            unsafe { instance.get_physical_device_memory_properties(phys_device) };
        let device = create_logical_device(&instance, &phys_device_info);
        let queues = get_queues(&device, &phys_device_info.queue_family_indices);
        let win_width = window.width();
        let win_height = window.height();
        let swapchain_format = choose_swapchain_format(phys_device, &surface);
        let depth_format = find_depth_format(&instance, phys_device);

        let render_pass =
            create_render_pass_no_attachments(&device, swapchain_format.format, depth_format);

        let (swapchain, depth_textures, attachments, framebuffers, viewport, scissor) =
            Self::create_swapchain_and_accessories(
                &phys_device_info,
                &surface,
                swapchain_format,
                win_width,
                win_height,
                &instance,
                &device,
                &device_mem_properties,
                depth_format,
                render_pass,
            );

        let graphics_queue_idx = phys_device_info.queue_family_indices.graphics;
        let per_frame_copies = FRAMES_IN_FLIGHT as usize;
        let command_pool = create_command_pool(&device, graphics_queue_idx, true);
        let command_buffers = alloc_command_buffers(&device, command_pool, per_frame_copies);

        let image_available = create_semaphores(&device, per_frame_copies);
        let render_finished = create_semaphores(&device, per_frame_copies);
        let is_rendering = create_fences(&device, true, per_frame_copies);

        let texture = ManuallyDrop::new(Texture::new(
            &device,
            &device_mem_properties,
            command_pool,
            queues.graphics,
            "assets/cat.jxl",
        ));

        let meshes = create_meshes(
            window,
            &device,
            &device_mem_properties,
            command_pool,
            queues.graphics,
            render_pass,
            &texture,
            &attachments,
            &depth_textures,
            per_frame_copies,
        );

        let inst = Self {
            instance,
            debug_data,
            surface,
            phys_device_info,
            device,
            device_mem_properties,
            queues,
            swapchain,
            viewport,
            scissor,
            command_pool,
            command_buffers,
            render_pass,
            depth_format,
            depth_textures,
            attachments,
            framebuffers,
            image_available,
            render_finished,
            is_rendering,
            texture,
            compute_target: None,
            compute_target_mesh: None,
            meshes,
            current_frame: 0,
            per_frame_copies,
            win_width,
            win_height,
            win_resized: false,
        };

        Ok(inst)
    }

    fn create_swapchain_and_accessories(
        phys_device_info: &PhysDeviceInfo,
        surface: &Surface,
        swapchain_format: vk::SurfaceFormatKHR,
        win_width: u32,
        win_height: u32,
        instance: &ash::Instance,
        device: &ash::Device,
        device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
        depth_format: vk::Format,
        render_pass: vk::RenderPass,
    ) -> (
        Swapchain,
        Vec<FramebufferAttachment>,
        Vec<FramebufferAttachment>,
        Vec<vk::Framebuffer>,
        vk::Viewport,
        vk::Rect2D,
    ) {
        let swapchain = Swapchain::new(
            phys_device_info.phys_device,
            surface,
            swapchain_format,
            win_width,
            win_height,
            instance,
            device,
            &phys_device_info.queue_family_indices,
        );

        let num_swapchain_images = swapchain.image_views.len();

        let mut depth_textures = Vec::with_capacity(num_swapchain_images);

        for _ in 0..num_swapchain_images {
            let depth_texture = FramebufferAttachment::new(
                device,
                device_mem_properties,
                swapchain.extent,
                depth_format,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                    | vk::ImageUsageFlags::INPUT_ATTACHMENT,
                vk::ImageAspectFlags::DEPTH,
            );

            depth_textures.push(depth_texture);
        }

        let attachments = vec![];

        let framebuffers = create_framebuffers(
            device,
            &swapchain.image_views,
            &depth_textures,
            &attachments,
            swapchain.extent,
            render_pass,
        );

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: to_f32(win_width),
            height: to_f32(win_height),
            min_depth: 0.0,
            max_depth: 1.0,
        };

        let scissor = vk::Rect2D {
            offset: vk::Offset2D::default(),
            extent: swapchain.extent,
        };

        (swapchain, depth_textures, attachments, framebuffers, viewport, scissor)
    }

    pub fn draw(&mut self) {
        if let Some(ct) = &self.compute_target {
            ct.wait(self.current_frame);
            ct.record_compute_commands(self.current_frame);
            ct.submit(self.current_frame);
        }

        self.wait();

        let Some(image_index) = self.acquire_image() else {
            return;
        };

        self.record_commands(image_index);
        self.submit();
        self.present_frame(image_index);

        self.current_frame += 1;
        self.current_frame %= FRAMES_IN_FLIGHT as usize;
    }

    fn wait(&self) {
        let is_rendering = self.is_rendering[self.current_frame];

        unsafe {
            self.device
                .wait_for_fences(&[is_rendering], true, DRAW_TIMEOUT_NS)
                .check_err("wait for fences");
        }
    }

    fn acquire_image(&mut self) -> Option<u32> {
        let image_available = self.image_available[self.current_frame];
        let is_rendering = self.is_rendering[self.current_frame];

        unsafe {
            let res = self.swapchain.loader.acquire_next_image(
                self.swapchain.handle,
                DRAW_TIMEOUT_NS,
                image_available,
                vk::Fence::null(),
            );

            match res {
                // If the swapchain is suboptimal, wait until `present_frame()` to recreate it,
                // in case the number of images will change on resize.
                Ok((image_index, _suboptimal)) => {
                    self.device.reset_fences(&[is_rendering]).check_err("reset fences");
                    Some(image_index)
                }
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.recreate_swapchain();
                    None
                }
                Err(e) => panic!("failed to acquire next image: err = {}", e),
            }
        }
    }

    fn record_commands(&self, image_index: u32) {
        let framebuffer = self.framebuffers[image_index as usize];
        let cmd_buffer = self.command_buffers[self.current_frame];
        let begin_info = ONE_TIME_SUBMIT;

        let clear_values = [CLEAR_COLOR, CLEAR_DEPTH];

        let render_pass_info = vk::RenderPassBeginInfo {
            render_pass: self.render_pass,
            framebuffer,
            render_area: self.scissor,
            clear_value_count: to_u32(clear_values.len()),
            p_clear_values: clear_values.as_ptr(),
            ..Default::default()
        };

        unsafe {
            self.device
                .reset_command_buffer(cmd_buffer, vk::CommandBufferResetFlags::empty())
                .check_err("reset cmd buffer");

            self.device
                .begin_command_buffer(cmd_buffer, &begin_info)
                .check_err("begin recording to command buffer");

            if let Some(ct) = &self.compute_target {
                ct.acquire_barrier_for_graphics_queue(cmd_buffer, self.current_frame);
            }

            self.device.cmd_set_viewport(cmd_buffer, 0, &[self.viewport]);

            self.device.cmd_set_scissor(cmd_buffer, 0, &[self.scissor]);

            self.device.cmd_begin_render_pass(
                cmd_buffer,
                &render_pass_info,
                vk::SubpassContents::INLINE,
            );

            if let Some(cm) = &self.compute_target_mesh {
                cm.record_draw_commands(cmd_buffer, self.current_frame);
            }

            for mesh in &self.meshes {
                mesh.record_draw_commands(cmd_buffer, self.current_frame);
            }

            self.device.cmd_end_render_pass(cmd_buffer);

            self.device.end_command_buffer(cmd_buffer).check_err("end command buffer recording");
        }
    }

    fn submit(&mut self) {
        let cmd_buffer = &self.command_buffers[self.current_frame];
        let image_available = self.image_available[self.current_frame];
        let render_finished = self.render_finished[self.current_frame];
        let is_rendering = self.is_rendering[self.current_frame];

        let wait_semaphores;
        let wait_dst_stages;

        if let Some(ct) = &self.compute_target {
            let compute_finished = ct.compute_finished(self.current_frame);

            wait_semaphores = vec![compute_finished, image_available];
            wait_dst_stages = vec![
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ];
        } else {
            wait_semaphores = vec![image_available];
            wait_dst_stages = vec![vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        };

        let submit_info = vk::SubmitInfo {
            wait_semaphore_count: to_u32(wait_semaphores.len()),
            p_wait_semaphores: wait_semaphores.as_ptr(),
            p_wait_dst_stage_mask: wait_dst_stages.as_ptr(),
            command_buffer_count: 1,
            p_command_buffers: cmd_buffer,
            signal_semaphore_count: 1,
            p_signal_semaphores: &render_finished,
            ..Default::default()
        };

        unsafe {
            self.device
                .queue_submit(self.queues.graphics, &[submit_info], is_rendering)
                .check_err("submit to draw queue");
        }
    }

    fn present_frame(&mut self, image_index: u32) {
        let render_finished = self.render_finished[self.current_frame];
        let res = self.swapchain.present(render_finished, image_index, self.queues.present);
        let suboptimal = Ok(true);

        if res == suboptimal || res == Err(vk::Result::ERROR_OUT_OF_DATE_KHR) || self.win_resized {
            self.recreate_swapchain();
            self.win_resized = false;
        } else if let Err(e) = res {
            panic!("failed to queue image for presentation: err = {}", e);
        }
    }

    pub fn update_data(&mut self, camera: &mut Camera, world: &mut World) {
        let win_size = (to_f32(self.win_width), to_f32(self.win_height));

        if let Some(ct) = &mut self.compute_target {
            ct.update_data(camera, world, self.current_frame);
        }

        for mesh in &mut self.meshes {
            mesh.update_data(camera, win_size, self.current_frame);
        }
    }

    pub fn handle_resize(&mut self, w: u32, h: u32) {
        self.win_width = w;
        self.win_height = h;
        self.win_resized = true;
    }

    pub fn add_compute_target(&mut self, world: &World) {
        let compute_push_consts = RayCastPushConstants {
            world_size_x: world.size_x(),
            world_size_y: world.size_y(),
            world_size_z: world.size_z(),
            ..Default::default()
        };

        let compute_update_data_cb =
            |ct: &mut ComputeTarget, camera: &mut Camera, world: &mut World, _current_frame| {
                if let Some(PushConstType::RayCast(c)) = ct.push_const_mut() {
                    let ang = camera.ang_y();
                    let ang_rot = ang + std::f32::consts::FRAC_PI_2;
                    let dir = Vec2::from_angle(ang);
                    let rot = Vec2::from_angle(ang_rot);
                    let dir = vec2(dir.y, -dir.x); // IDFK
                    let rot = vec2(rot.y, -rot.x);
                    let plane = rot * camera.plane_len();

                    c.pos = camera.position();
                    c.dir = dir;
                    c.plane = plane;
                }

                if world.needs_upload() {
                    ct.copy_to_buffer(0, world.sizes());
                    ct.copy_to_buffer(1, world.spans());
                    world.uploaded();
                }
            };

        let compute_target = ComputeTarget::new(
            &self.instance,
            &self.phys_device_info,
            &self.device,
            &self.device_mem_properties,
            self.command_pool,
            &self.queues,
            self.win_width,
            self.win_height,
            include_shader!("raycasting.comp"),
            32,
            1,
            Some(PushConstType::RayCast(compute_push_consts)),
            compute_update_data_cb,
            self.per_frame_copies,
        );

        let compute_textures = compute_target.textures();

        let mesh = Mesh::textured_screen_quad()
            .to_builder(
                &self.device,
                &self.device_mem_properties,
                self.command_pool,
                self.queues.graphics,
                self.render_pass,
                self.per_frame_copies,
                include_shader!("textured_screen_quad.vert"),
                include_shader!("textured_screen_quad.frag"),
            )
            .with_textures(compute_textures)
            .build();

        self.compute_target = Some(compute_target);
        self.compute_target_mesh = Some(mesh);
    }

    fn recreate_swapchain(&mut self) {
        unsafe {
            self.device.device_wait_idle().check_err("wait for device");
            self.cleanup_swapchain();
        }

        let (swapchain, depth_textures, attachments, framebuffers, viewport, scissor) =
            Self::create_swapchain_and_accessories(
                &self.phys_device_info,
                &self.surface,
                self.swapchain.format,
                self.win_width,
                self.win_height,
                &self.instance,
                &self.device,
                &self.device_mem_properties,
                self.depth_format,
                self.render_pass,
            );

        self.swapchain = swapchain;
        self.depth_textures = depth_textures;
        self.attachments = attachments;
        self.framebuffers = framebuffers;
        self.viewport = viewport;
        self.scissor = scissor;
    }

    unsafe fn cleanup_swapchain(&mut self) {
        for fb in &self.framebuffers {
            self.device.destroy_framebuffer(*fb, None);
        }

        self.depth_textures.drain(..);
        self.attachments.drain(..);

        self.swapchain.destroy();
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().check_err("wait for device");

            for sem in &self.image_available {
                self.device.destroy_semaphore(*sem, None);
            }

            for sem in &self.render_finished {
                self.device.destroy_semaphore(*sem, None);
            }

            for fence in &self.is_rendering {
                self.device.destroy_fence(*fence, None);
            }

            self.cleanup_swapchain();

            ManuallyDrop::drop(&mut self.texture);

            self.compute_target.take();
            self.compute_target_mesh.take();

            self.device.destroy_render_pass(self.render_pass, None);

            self.meshes.drain(..);

            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_device(None);
            ManuallyDrop::drop(&mut self.surface);

            self.debug_data.take();

            self.instance.destroy_instance(None);
        }
    }
}

impl Drop for DebugData {
    fn drop(&mut self) {
        unsafe {
            self.debug_utils_loader.destroy_debug_utils_messenger(self.debug_messenger, None);
        }
    }
}

fn create_attachments(
    device: &ash::Device,
    device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
    swapchain: &Swapchain,
) -> Vec<FramebufferAttachment> {
    let mut attachments = Vec::with_capacity(swapchain.image_views.len());

    let format = swapchain.format.format;
    let extent = swapchain.extent;
    let usage = vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::INPUT_ATTACHMENT;
    let aspect_mask = vk::ImageAspectFlags::COLOR;

    for _ in &swapchain.image_views {
        let attachment = FramebufferAttachment::new(
            device,
            device_mem_properties,
            extent,
            format,
            usage,
            aspect_mask,
        );

        attachments.push(attachment);
    }

    attachments
}

#[allow(unused_variables, clippy::too_many_lines)]
fn create_meshes(
    window: &Window,
    device: &ash::Device,
    device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
    command_pool: vk::CommandPool,
    graphics_queue: vk::Queue,
    render_pass: vk::RenderPass,
    texture: &Texture,
    attachments: &[FramebufferAttachment],
    depth_textures: &[FramebufferAttachment],
    per_frame_copies: usize,
) -> Vec<MeshData> {
    let win_sx = to_f32(window.width());
    let win_sy = to_f32(window.height());
    let win_res = vec2(win_sx, win_sy);

    let skybox = {
        let skybox_push_consts = SkyboxPushConstants {
            inv: Mat4::IDENTITY,
            pos: Vec3::default(),
            _pad1: 0.0,
            res: win_res,
            _pad2: [0.0; 2],
        };

        let mut skybox = Mesh::screen_rect()
            .to_builder(
                device,
                device_mem_properties,
                command_pool,
                graphics_queue,
                render_pass,
                per_frame_copies,
                include_shader!("skybox.vert"),
                include_shader!("skybox.frag"),
            )
            .with_push_consts(
                PushConstType::Skybox(skybox_push_consts),
                vk::ShaderStageFlags::FRAGMENT,
            )
            .build();

        skybox.set_update_data_cb(|mesh, camera, win_size, _current_frame| {
            if let Some(PushConstType::Skybox(s)) = mesh.push_const_mut() {
                s.inv = *camera.inverse();
                s.pos = camera.position();
                s.res.x = win_size.0;
                s.res.y = win_size.1;
            }
        });

        skybox
    };

    let crosshair = {
        let crosshair_push_consts = CrosshairPushConstants {
            res: win_res,
            _pad: 0.0,
            color: vec3(0.0, 1.0, 0.0),
        };

        let mut crosshair = Mesh::crosshair(6.0, 2.0)
            .to_builder(
                device,
                device_mem_properties,
                command_pool,
                graphics_queue,
                render_pass,
                per_frame_copies,
                include_shader!("crosshair.vert"),
                include_shader!("crosshair.frag"),
            )
            .with_push_consts(
                PushConstType::Crosshair(crosshair_push_consts),
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            )
            .build();

        crosshair.set_update_data_cb(|mesh, _camera, win_size, _current_frame| {
            if let Some(PushConstType::Crosshair(s)) = mesh.push_const_mut() {
                s.res.x = win_size.0;
                s.res.y = win_size.1;
            }
        });

        crosshair
    };

    let grid = {
        let mvp = ModelViewProjUBO {
            model: Mat4::IDENTITY,
            view: Mat4::IDENTITY,
            proj: Mat4::IDENTITY,
        };

        let mut grid = Mesh::grid(1.0, 32)
            .to_builder(
                device,
                device_mem_properties,
                command_pool,
                graphics_queue,
                render_pass,
                per_frame_copies,
                include_shader!("grid.vert"),
                include_shader!("grid.frag"),
            )
            .with_uniform_buffer(UniformBufferType::ModelViewProj(mvp))
            .build();

        grid.set_update_data_cb(|mesh, camera, _win_size, current_frame| {
            if let Some(UniformBufferType::ModelViewProj(m)) = mesh.uniform_buffer_mut() {
                m.view = *camera.view();
                m.proj = *camera.proj();
            }

            mesh.copy_to_uniform_mapping(current_frame);
        });

        grid
    };

    let cube_lines = {
        let mvp = ModelViewProjUBO {
            model: Mat4::from_translation(vec3(0.0, 0.5, 0.0)),
            view: Mat4::IDENTITY,
            proj: Mat4::IDENTITY,
        };

        let mut cube_lines = Mesh::cube_lines(1.0)
            .to_builder(
                device,
                device_mem_properties,
                command_pool,
                graphics_queue,
                render_pass,
                per_frame_copies,
                include_shader!("cube.vert"),
                include_shader!("cube.frag"),
            )
            .with_uniform_buffer(UniformBufferType::ModelViewProj(mvp))
            .build();

        cube_lines.set_update_data_cb(|mesh, camera, _win_size, current_frame| {
            if let Some(UniformBufferType::ModelViewProj(m)) = mesh.uniform_buffer_mut() {
                m.view = *camera.view();
                m.proj = *camera.proj();
            }

            mesh.copy_to_uniform_mapping(current_frame);
        });

        cube_lines
    };

    let axes = {
        let mvp = ModelViewProjUBO {
            model: Mat4::IDENTITY,
            view: Mat4::IDENTITY,
            proj: Mat4::IDENTITY,
        };

        let mut axes = Mesh::axes()
            .to_builder(
                device,
                device_mem_properties,
                command_pool,
                graphics_queue,
                render_pass,
                per_frame_copies,
                include_shader!("colored.vert"),
                include_shader!("colored.frag"),
            )
            .with_uniform_buffer(UniformBufferType::ModelViewProj(mvp))
            .build();

        axes.set_update_data_cb(|mesh, camera, _win_size, current_frame| {
            if let Some(UniformBufferType::ModelViewProj(m)) = mesh.uniform_buffer_mut() {
                m.view = *camera.view();
                m.proj = *camera.proj();
            }

            mesh.copy_to_uniform_mapping(current_frame);
        });

        axes
    };

    let quad = {
        let mvp = ModelViewProjUBO {
            model: Mat4::IDENTITY,
            view: Mat4::IDENTITY,
            proj: Mat4::IDENTITY,
        };

        let mut quad = Mesh::textured_quad()
            .to_builder(
                device,
                device_mem_properties,
                command_pool,
                graphics_queue,
                render_pass,
                per_frame_copies,
                include_shader!("quad.vert"),
                include_shader!("quad.frag"),
            )
            .with_uniform_buffer(UniformBufferType::ModelViewProj(mvp))
            .with_texture(texture)
            .build();

        quad.set_update_data_cb(|mesh, camera, _win_size, current_frame| {
            if let Some(UniformBufferType::ModelViewProj(m)) = mesh.uniform_buffer_mut() {
                m.view = *camera.view();
                m.proj = *camera.proj();
            }

            mesh.copy_to_uniform_mapping(current_frame);
        });

        quad
    };

    let cube = {
        let mvp = ModelViewProjUBO {
            model: Mat4::from_translation(vec3(0.0, 2.0, 0.0)),
            view: Mat4::IDENTITY,
            proj: Mat4::IDENTITY,
        };

        let mut cube = Mesh::cube(1.0)
            .to_builder(
                device,
                device_mem_properties,
                command_pool,
                graphics_queue,
                render_pass,
                per_frame_copies,
                include_shader!("colored.vert"),
                include_shader!("colored.frag"),
            )
            .with_uniform_buffer(UniformBufferType::ModelViewProj(mvp))
            .build();

        cube.set_update_data_cb(|mesh, camera, _win_size, current_frame| {
            if let Some(UniformBufferType::ModelViewProj(m)) = mesh.uniform_buffer_mut() {
                m.view = *camera.view();
                m.proj = *camera.proj();
            }

            mesh.copy_to_uniform_mapping(current_frame);
        });

        cube
    };

    vec![]
}
