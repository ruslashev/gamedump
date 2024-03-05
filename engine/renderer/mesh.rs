use std::mem::size_of;

use ash::vk;

use super::pipeline::*;
use super::vulkan::*;
use super::*;
use crate::utils::*;

pub struct Mesh {
    vertices: Vec<f32>,
    indices: Vec<u16>,
    builder: PipelineModifier,
}

pub struct MeshDataBuilder<'m, 'p> {
    mesh: &'m Mesh,
    device: ash::Device,
    device_mem_properties: &'p vk::PhysicalDeviceMemoryProperties,
    command_pool: vk::CommandPool,
    graphics_queue: vk::Queue,
    render_pass: vk::RenderPass,
    vert_shader_compiled: &'p [u8],
    frag_shader_compiled: &'p [u8],
    advances_subpass: bool,
    per_frame_copies: usize,
    shader_attachments: Vec<ShaderAttachment<'p>>,
    push_consts: Option<PushConstants>,
    builder: Option<PipelineModifier>,
}

pub struct MeshData {
    device: ash::Device,
    vertex_buffer: vk::Buffer,
    vertex_buffer_memory: vk::DeviceMemory,
    index_buffer: vk::Buffer,
    index_buffer_memory: vk::DeviceMemory,
    index_count: u32,
    pipeline: Pipeline,
    push_consts: Option<PushConstants>,
    uniform_buffer: Option<UniformBuffer>,
    update_data: Option<UpdateDataCb>,
    advances_subpass: bool,
}

pub struct ComputeTarget {
    device: ash::Device,
    color: Vec<Texture>,
    depth: Vec<Texture>,
    cmd_pool: vk::CommandPool,
    cmd_buffers: Vec<vk::CommandBuffer>,
    comp_finished_sem: Vec<vk::Semaphore>,
    comp_finished_fences: Vec<vk::Fence>,
    pipeline: Pipeline,
    queue: vk::Queue,
    graphics_queue_idx: u32,
    compute_queue_idx: u32,
    push_consts: Option<PushConstants>,
    buffers: Vec<ComputeBufferReadOnlyMemory>,
    update_data_cb: UpdateCompDataCb,
    desc_sets: Vec<vk::DescriptorSet>,
    desc_set_layout: vk::DescriptorSetLayout,
    desc_pool: vk::DescriptorPool,
    width: u32,
    height: u32,
    local_size_x: u32,
    local_size_y: u32,
    clear_color: bool,
}

struct UniformBuffer {
    device: ash::Device,
    desc_set_layout: vk::DescriptorSetLayout,
    desc_pool: vk::DescriptorPool,
    desc_sets: Vec<vk::DescriptorSet>,
    buf_mem: Option<BufferMemory>,
}

struct BufferMemory {
    device: ash::Device,
    buffers: Vec<vk::Buffer>,
    memories: Vec<vk::DeviceMemory>,
    mappings: Vec<*mut UniformBufferType>,
    data: Box<UniformBufferType>,
    size: u64,
}

struct ComputeBufferReadOnlyMemory {
    device: ash::Device,
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    mapping: *mut u32,
    size: u64,
}

pub enum ShaderAttachment<'t> {
    UniformBuffer(UniformBufferType),
    Texture(&'t Texture),
    Textures(&'t [Texture]),
    InputAttachment(&'t FramebufferAttachment),
}

#[derive(Clone, Copy)]
pub enum UniformBufferType {
    ModelViewProj(ModelViewProjUBO),
}

pub struct PushConstants {
    data: Box<PushConstType>,
    stage: vk::ShaderStageFlags,
    range: vk::PushConstantRange,
}

pub enum PushConstType {
    RayCast(RayCastPushConstants),
    Skybox(SkyboxPushConstants),
    Crosshair(CrosshairPushConstants),
    RayTrace(RayTracePushConstants),
}

type PipelineModifier = fn(&mut PipelineBuilder) -> &mut PipelineBuilder;
type UpdateDataCb = fn(&mut MeshData, &mut Camera, (f32, f32), usize);
type UpdateCompDataCb = fn(&mut ComputeTarget, &mut Camera, &mut World, usize);

impl Mesh {
    pub fn screen_rect() -> Self {
        Self {
            vertices: vec![-1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0],
            indices: vec![0, 1, 2, 2, 3, 0],
            builder: |b| b.with_2_vertices(),
        }
    }

    pub fn grid(res: f32, cells: u16) -> Self {
        let fcells = f32::from(cells);
        let min = -(res * fcells);
        let max = res * fcells;

        let mut x_off = -(res * fcells);
        let mut y_off = -(res * fcells);

        let mut vertices = vec![];
        let mut indices = vec![];

        let mut idx = 0;

        for _ in 0..=(cells * 2) {
            vertices.push(min);
            vertices.push(y_off);
            vertices.push(max);
            vertices.push(y_off);

            indices.push(idx);
            indices.push(idx + 1);

            idx += 2;
            y_off += res;

            vertices.push(x_off);
            vertices.push(min);
            vertices.push(x_off);
            vertices.push(max);

            indices.push(idx);
            indices.push(idx + 1);

            idx += 2;
            x_off += res;
        }

        Self {
            vertices,
            indices,
            builder: |b| b.with_topology(vk::PrimitiveTopology::LINE_LIST).with_2_vertices(),
        }
    }

    pub fn crosshair(length: f32, thickness: f32) -> Self {
        let near = thickness;
        let far = near + length;

        let vertices = vec![
            // Up
            0.0,
            near + 0.5,
            0.0,
            far + 0.5,
            // Right
            near + 0.5,
            0.0,
            far + 0.5,
            0.0,
            // Down
            0.0,
            -near - 0.5,
            0.0,
            -far - 0.5,
            // Left
            -near - 0.5,
            0.0,
            -far - 0.5,
            0.0,
        ];

        let mut indices = vec![];

        for i in 0..16 {
            indices.push(i);
        }

        Self {
            vertices,
            indices,
            builder: |b| b.with_topology(vk::PrimitiveTopology::LINE_LIST).with_2_vertices(),
        }
    }

    pub fn cube(size: f32) -> Self {
        let p = size / 2.0;
        let n = -p;

        /*      6                7
         *        o------------o
         *      / : \___     / |
         * 5  /   :     \  /   |
         *   o------------o    |
         *   |    :       | 4  |
         *   |    :       |    |
         *   |  2 :       |    | 3
         *   |    o-------|----o
         *   |  /   \___  |  /
         *   |/         \ |/
         *   o------------o
         * 1               0
         */

        let vertices = vec![
            p, n, p, 1., 0., 1., // 0
            n, n, p, 0., 0., 1., // 1
            n, n, n, 0., 0., 0., // 2
            p, n, n, 1., 0., 0., // 3
            p, p, p, 1., 1., 1., // 4
            n, p, p, 0., 1., 1., // 5
            n, p, n, 0., 1., 0., // 6
            p, p, n, 1., 1., 0., // 7
        ];

        let indices = vec![
            0, 1, 2, 0, 2, 3, // bottom
            4, 6, 5, 4, 7, 6, // top
            4, 0, 3, 4, 3, 7, // right
            6, 7, 3, 6, 3, 2, // back
            5, 6, 2, 1, 5, 2, // left
            0, 4, 5, 0, 5, 1, // front
        ];

        Self {
            vertices,
            indices,
            builder: |b| {
                let pos_desc = vk::VertexInputAttributeDescription {
                    location: 0,
                    binding: 0,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: 0,
                };

                let col_desc = vk::VertexInputAttributeDescription {
                    location: 1,
                    binding: 0,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: 3 * SIZE_F32,
                };

                b.with_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
                    .with_stride_in_f32s(6)
                    .add_vertex_desc(pos_desc)
                    .add_vertex_desc(col_desc)
            },
        }
    }

    pub fn cube_lines(size: f32) -> Self {
        let s = size / 2.0;

        let vertices = vec![
            s, -s, -s, -s, -s, -s, -s, -s, s, s, -s, s, // lower
            s, s, -s, -s, s, -s, -s, s, s, s, s, s, // upper
        ];

        let indices = vec![
            0, 1, 1, 2, 2, 3, 3, 0, // bottom
            4, 5, 5, 6, 6, 7, 7, 4, // top
            0, 4, 1, 5, 2, 6, 3, 7, // in-between
        ];

        Self {
            vertices,
            indices,
            builder: |b| b.with_topology(vk::PrimitiveTopology::LINE_LIST).with_3_vertices(),
        }
    }

    pub fn axes() -> Self {
        let vertices = vec![
            0., 0., 0., 1., 0., 0., // 0, r
            1., 0., 0., 1., 0., 0., // x, r
            0., 0., 0., 0., 1., 0., // 0, g
            0., 1., 0., 0., 1., 0., // y, g
            0., 0., 0., 0., 0., 1., // 0, b
            0., 0., 1., 0., 0., 1., // z, b
        ];
        let indices = vec![0, 1, 2, 3, 4, 5];

        Self {
            vertices,
            indices,
            builder: |b| {
                let pos_desc = vk::VertexInputAttributeDescription {
                    location: 0,
                    binding: 0,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: 0,
                };

                let col_desc = vk::VertexInputAttributeDescription {
                    location: 1,
                    binding: 0,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: 3 * SIZE_F32,
                };

                b.with_topology(vk::PrimitiveTopology::LINE_LIST)
                    .with_stride_in_f32s(6)
                    .add_vertex_desc(pos_desc)
                    .add_vertex_desc(col_desc)
            },
        }
    }

    pub fn textured_quad() -> Self {
        let vertices = vec![
            -0.5, -0.5, 0.0, 1.0, // BL
            0.5, -0.5, 1.0, 1.0, // BR
            0.5, 0.5, 1.0, 0.0, // TR
            -0.5, 0.5, 0.0, 0.0, // TL
        ];
        let indices = vec![0, 1, 2, 2, 3, 0];

        Self {
            vertices,
            indices,
            builder: |b| {
                let pos_desc = vk::VertexInputAttributeDescription {
                    location: 0,
                    binding: 0,
                    format: vk::Format::R32G32_SFLOAT,
                    offset: 0,
                };

                let uv_desc = vk::VertexInputAttributeDescription {
                    location: 1,
                    binding: 0,
                    format: vk::Format::R32G32_SFLOAT,
                    offset: 2 * SIZE_F32,
                };

                b.with_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
                    .with_stride_in_f32s(4)
                    .add_vertex_desc(pos_desc)
                    .add_vertex_desc(uv_desc)
            },
        }
    }

    pub fn textured_screen_quad() -> Self {
        let vertices = vec![
            -1.0, 1.0, 0.0, 1.0, // 0
            1.0, 1.0, 1.0, 1.0, // 1
            1.0, -1.0, 1.0, 0.0, // 2
            -1.0, -1.0, 0.0, 0.0, // 3
        ];
        let indices = vec![0, 1, 2, 2, 3, 0];

        Self {
            vertices,
            indices,
            builder: |b| {
                let pos_desc = vk::VertexInputAttributeDescription {
                    location: 0,
                    binding: 0,
                    format: vk::Format::R32G32_SFLOAT,
                    offset: 0,
                };

                let uv_desc = vk::VertexInputAttributeDescription {
                    location: 1,
                    binding: 0,
                    format: vk::Format::R32G32_SFLOAT,
                    offset: 2 * SIZE_F32,
                };

                b.with_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
                    .with_stride_in_f32s(4)
                    .add_vertex_desc(pos_desc)
                    .add_vertex_desc(uv_desc)
            },
        }
    }

    pub fn to_builder<'m, 'p>(
        &'m self,
        device: &ash::Device,
        device_mem_properties: &'p vk::PhysicalDeviceMemoryProperties,
        command_pool: vk::CommandPool,
        graphics_queue: vk::Queue,
        render_pass: vk::RenderPass,
        per_frame_copies: usize,
        vert_shader_compiled: &'p [u8],
        frag_shader_compiled: &'p [u8],
    ) -> MeshDataBuilder<'m, 'p> {
        MeshDataBuilder {
            mesh: self,
            device: device.clone(),
            device_mem_properties,
            command_pool,
            graphics_queue,
            render_pass,
            vert_shader_compiled,
            frag_shader_compiled,
            advances_subpass: false,
            per_frame_copies,
            shader_attachments: vec![],
            push_consts: None,
            builder: None,
        }
    }
}

impl<'m, 'p> MeshDataBuilder<'m, 'p> {
    pub fn with_push_consts(mut self, data: PushConstType, stage: vk::ShaderStageFlags) -> Self {
        let data = Box::new(data);
        let size = to_u32(data.size());
        let range = create_push_const_range(size, stage);
        let push_consts = PushConstants { data, stage, range };

        self.push_consts = Some(push_consts);
        self
    }

    pub fn with_uniform_buffer(mut self, buf: UniformBufferType) -> Self {
        self.shader_attachments.push(ShaderAttachment::UniformBuffer(buf));
        self
    }

    pub fn with_texture(mut self, texture: &'p Texture) -> Self {
        self.shader_attachments.push(ShaderAttachment::Texture(texture));
        self
    }

    pub fn with_textures(mut self, textures: &'p [Texture]) -> Self {
        self.shader_attachments.push(ShaderAttachment::Textures(textures));
        self
    }

    pub fn with_input_attachment(mut self, attachment: &'p FramebufferAttachment) -> Self {
        self.shader_attachments.push(ShaderAttachment::InputAttachment(attachment));
        self
    }

    pub fn advances_subpass(mut self) -> Self {
        self.advances_subpass = true;
        self
    }

    pub fn modify_builder(mut self, cb: PipelineModifier) -> Self {
        self.builder = Some(cb);
        self
    }

    pub fn build(self) -> MeshData {
        let (vertex_buffer, vertex_buffer_memory) = create_buffer_of_type(
            &self.device,
            self.device_mem_properties,
            self.command_pool,
            self.graphics_queue,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            &self.mesh.vertices,
        );

        let (index_buffer, index_buffer_memory) = create_buffer_of_type(
            &self.device,
            self.device_mem_properties,
            self.command_pool,
            self.graphics_queue,
            vk::BufferUsageFlags::INDEX_BUFFER,
            &self.mesh.indices,
        );

        let index_count = to_u32(self.mesh.indices.len());

        let vert_shader = ShaderModule::new(&self.device, self.vert_shader_compiled);
        let frag_shader = ShaderModule::new(&self.device, self.frag_shader_compiled);

        let push_const_range = self.push_consts.as_ref().map(|r| r.range);

        let uniform_buffer = build_uniform_buffer(
            &self.device,
            self.device_mem_properties,
            &self.shader_attachments,
            self.per_frame_copies,
        );

        let desc_set_layout = uniform_buffer.as_ref().map(|r| r.desc_set_layout);

        let mut builder = PipelineBuilder::new(
            &self.device,
            self.render_pass,
            push_const_range.as_ref(),
            desc_set_layout.as_ref(),
        );

        let modify_builder = self.mesh.builder;

        modify_builder(&mut builder);

        if let Some(cb) = self.builder {
            cb(&mut builder);
        }

        let pipeline = builder.build(&vert_shader, &frag_shader);

        MeshData {
            device: self.device,
            vertex_buffer,
            vertex_buffer_memory,
            index_buffer,
            index_buffer_memory,
            index_count,
            pipeline,
            push_consts: self.push_consts,
            uniform_buffer,
            update_data: None,
            advances_subpass: self.advances_subpass,
        }
    }
}

impl MeshData {
    pub unsafe fn record_draw_commands(&self, cmd_buffer: vk::CommandBuffer, current_frame: usize) {
        if self.advances_subpass {
            self.device.cmd_next_subpass(cmd_buffer, vk::SubpassContents::INLINE);
        }

        self.device.cmd_bind_pipeline(
            cmd_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.pipeline.inner,
        );
        self.device.cmd_bind_vertex_buffers(cmd_buffer, 0, &[self.vertex_buffer], &[0]);
        self.device.cmd_bind_index_buffer(cmd_buffer, self.index_buffer, 0, vk::IndexType::UINT16);

        if let Some(p) = &self.push_consts {
            let bytes = p.data.as_bytes();

            self.device.cmd_push_constants(cmd_buffer, self.pipeline.layout, p.stage, 0, bytes);
        }

        if let Some(u) = &self.uniform_buffer {
            let desc_set = u.desc_sets[current_frame];

            self.device.cmd_bind_descriptor_sets(
                cmd_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline.layout,
                0,
                &[desc_set],
                &[],
            );
        }

        self.device.cmd_draw_indexed(cmd_buffer, self.index_count, 1, 0, 0, 0);
    }

    pub fn copy_to_uniform_mapping(&mut self, current_frame: usize) {
        let Some(u) = &self.uniform_buffer else {
            return;
        };

        let Some(m) = &u.buf_mem else {
            return;
        };

        unsafe {
            m.mappings[current_frame].copy_from_nonoverlapping(m.data.as_ref(), 1);
        }
    }

    pub fn uniform_buffer_mut(&mut self) -> Option<&mut UniformBufferType> {
        let Some(u) = &mut self.uniform_buffer else {
            return None;
        };

        let Some(m) = &mut u.buf_mem else {
            return None;
        };

        Some(m.data.as_mut())
    }

    pub fn push_const_mut(&mut self) -> Option<&mut PushConstType> {
        self.push_consts.as_mut().map(|x| x.data.as_mut())
    }

    pub fn set_update_data_cb(&mut self, cb: UpdateDataCb) {
        self.update_data = Some(cb);
    }

    pub fn update_data(&mut self, camera: &mut Camera, win_size: (f32, f32), current_frame: usize) {
        if let Some(cb) = self.update_data {
            cb(self, camera, win_size, current_frame);
        }
    }
}

impl Drop for MeshData {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_buffer(self.index_buffer, None);
            self.device.free_memory(self.index_buffer_memory, None);
            self.device.destroy_buffer(self.vertex_buffer, None);
            self.device.free_memory(self.vertex_buffer_memory, None);
        }
    }
}

impl ComputeTarget {
    pub fn new(
        instance: &ash::Instance,
        phys_device_info: &PhysDeviceInfo,
        device: &ash::Device,
        device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
        primary_command_pool: vk::CommandPool,
        queues: &Queues,
        width: u32,
        height: u32,
        compiled_shader: &[u8],
        local_size_x: u32,
        local_size_y: u32,
        push_const_type: Option<PushConstType>,
        update_data_cb: UpdateCompDataCb,
        per_frame_copies: usize,
    ) -> Self {
        let queue_indices = &phys_device_info.queue_family_indices;
        let phys_device = phys_device_info.phys_device;

        let graphics_queue_idx = queue_indices.graphics;
        let compute_queue_idx = queue_indices.compute;

        let color_formats = &[vk::Format::R8G8B8A8_UNORM];
        let depth_formats = &[vk::Format::R32_SFLOAT];

        let mut color = Vec::with_capacity(per_frame_copies);
        let mut depth = Vec::with_capacity(per_frame_copies);

        for _ in 0..per_frame_copies {
            let ct = Texture::new_compute(
                instance,
                phys_device,
                color_formats,
                device,
                device_mem_properties,
                primary_command_pool,
                queues.graphics,
                width,
                height,
            );

            let dt = Texture::new_compute(
                instance,
                phys_device,
                depth_formats,
                device,
                device_mem_properties,
                primary_command_pool,
                queues.graphics,
                width,
                height,
            );

            color.push(ct);
            depth.push(dt);
        }

        let len_sizes = world::MAX_SIZE_X * world::MAX_SIZE_Z;
        let len_spans = world::MAX_SIZE_X * world::MAX_SIZE_Y * world::MAX_SIZE_Z;

        let sizes = ComputeBufferReadOnlyMemory::new(device, device_mem_properties, len_sizes);
        let spans = ComputeBufferReadOnlyMemory::new(device, device_mem_properties, len_spans);

        let buffers = vec![sizes, spans];

        let bindings = [
            storage_image_binding(0),  // colorImage
            storage_image_binding(1),  // depthImage
            storage_buffer_binding(2), // worldSizes
            storage_buffer_binding(3), // worldSpans
        ];
        let pool_sizes = [storage_image_pool_size(2), storage_buffer_pool_size(2)];

        let mut num_sets = per_frame_copies;
        if cfg!(debug_assertions) {
            // debug printf extension
            num_sets += 1;
        }

        let desc_set_layout = create_desc_set_layout(device, &bindings);
        let desc_pool = create_desc_pool(device, &pool_sizes, num_sets);
        let desc_sets = alloc_desc_sets(device, desc_pool, desc_set_layout, num_sets);

        for i in 0..per_frame_copies {
            let color_desc_info = sampler_desc_info(&color[i]);
            let depth_desc_info = sampler_desc_info(&depth[i]);
            let sizes_desc_info = buffer_desc_info(buffers[0].buffer, buffers[0].size);
            let spans_desc_info = buffer_desc_info(buffers[1].buffer, buffers[1].size);

            let color_desc_write = storage_img_desc_write(desc_sets[i], 0, &color_desc_info);
            let depth_desc_write = storage_img_desc_write(desc_sets[i], 1, &depth_desc_info);
            let sizes_desc_write = ssbo_desc_write(desc_sets[i], 2, &sizes_desc_info);
            let spans_desc_write = ssbo_desc_write(desc_sets[i], 3, &spans_desc_info);

            let writes = [
                color_desc_write,
                depth_desc_write,
                sizes_desc_write,
                spans_desc_write,
            ];

            unsafe { device.update_descriptor_sets(&writes, &[]) };
        }

        let push_consts = push_const_type.map(|pt| {
            let stage = vk::ShaderStageFlags::COMPUTE;
            let size = to_u32(pt.size());
            let data = Box::new(pt);
            let range = create_push_const_range(size, stage);
            PushConstants { data, stage, range }
        });
        let push_const_range = push_consts.as_ref().map(|r| r.range);

        let pipeline = Pipeline::new_compute(
            device,
            push_const_range.as_ref(),
            desc_set_layout,
            compiled_shader,
        );
        let cmd_pool = create_command_pool(device, compute_queue_idx, true);
        let cmd_buffers = alloc_command_buffers(device, cmd_pool, per_frame_copies);

        let comp_finished_fences = create_fences(device, true, per_frame_copies);
        let comp_finished_sem = create_semaphores(device, per_frame_copies);

        Self {
            device: device.clone(),
            color,
            depth,
            cmd_pool,
            cmd_buffers,
            comp_finished_sem,
            comp_finished_fences,
            pipeline,
            queue: queues.compute,
            graphics_queue_idx,
            compute_queue_idx,
            push_consts,
            buffers,
            update_data_cb,
            desc_sets,
            desc_set_layout,
            desc_pool,
            width,
            height,
            local_size_x,
            local_size_y,
            clear_color: true,
        }
    }

    pub fn wait(&self, current_frame: usize) {
        let fence = self.comp_finished_fences[current_frame];

        unsafe {
            self.device
                .wait_for_fences(&[fence], true, DRAW_TIMEOUT_NS)
                .check_err("wait for compute fence");

            self.device.reset_fences(&[fence]).check_err("reset compute fence");
        }
    }

    pub fn record_compute_commands(&self, current_frame: usize) {
        let cmd_buffer = self.cmd_buffers[current_frame];

        let begin_info = ONE_TIME_SUBMIT;

        let clear_color_value = vk::ClearColorValue {
            float32: [0.0, 0.0, 0.0, 0.0],
        };

        let clear_depth_value = vk::ClearColorValue {
            float32: [1.0, 1.0, 1.0, 1.0],
        };

        let desc_set = self.desc_sets[current_frame];

        let group_count_x = self.width.div_ceil(self.local_size_x);
        let group_count_y = self.height.div_ceil(self.local_size_y);

        unsafe {
            self.device
                .reset_command_buffer(cmd_buffer, vk::CommandBufferResetFlags::empty())
                .check_err("reset compute command buffer");

            self.device
                .begin_command_buffer(cmd_buffer, &begin_info)
                .check_err("begin compute command buffer");

            self.device.cmd_bind_pipeline(
                cmd_buffer,
                vk::PipelineBindPoint::COMPUTE,
                self.pipeline.inner,
            );

            if self.clear_color {
                self.device.cmd_clear_color_image(
                    cmd_buffer,
                    self.color[current_frame].image,
                    self.color[current_frame].layout,
                    &clear_color_value,
                    &[BASE_SUBRESOURCE_RANGE],
                );
            }

            self.device.cmd_clear_color_image(
                cmd_buffer,
                self.depth[current_frame].image,
                self.depth[current_frame].layout,
                &clear_depth_value,
                &[BASE_SUBRESOURCE_RANGE],
            );

            self.clear_color_barrier(cmd_buffer);

            if let Some(p) = &self.push_consts {
                let bytes = p.data.as_bytes();
                self.device.cmd_push_constants(cmd_buffer, self.pipeline.layout, p.stage, 0, bytes);
            }

            self.device.cmd_bind_descriptor_sets(
                cmd_buffer,
                vk::PipelineBindPoint::COMPUTE,
                self.pipeline.layout,
                0,
                &[desc_set],
                &[],
            );

            self.device.cmd_dispatch(cmd_buffer, group_count_x, group_count_y, 1);

            self.release_barrier_for_compute_queue(cmd_buffer, current_frame);

            self.device.end_command_buffer(cmd_buffer).check_err("end compute command buffer");
        }
    }

    fn clear_color_barrier(&self, cmd_buffer: vk::CommandBuffer) {
        self.memory_barrier(
            cmd_buffer,
            vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::SHADER_WRITE,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::COMPUTE_SHADER,
        );
    }

    fn memory_barrier(
        &self,
        cmd_buffer: vk::CommandBuffer,
        src_access_mask: vk::AccessFlags,
        dst_access_mask: vk::AccessFlags,
        src_stage_mask: vk::PipelineStageFlags,
        dst_stage_mask: vk::PipelineStageFlags,
    ) {
        let barrier = vk::MemoryBarrier {
            src_access_mask,
            dst_access_mask,
            ..Default::default()
        };

        unsafe {
            self.device.cmd_pipeline_barrier(
                cmd_buffer,
                src_stage_mask,
                dst_stage_mask,
                vk::DependencyFlags::empty(),
                &[barrier],
                &[],
                &[],
            );
        }
    }

    fn image_barrier(
        &self,
        cmd_buffer: vk::CommandBuffer,
        current_frame: usize,
        src_queue_family_index: u32,
        dst_queue_family_index: u32,
        src_access_mask: vk::AccessFlags,
        dst_access_mask: vk::AccessFlags,
        src_stage_mask: vk::PipelineStageFlags,
        dst_stage_mask: vk::PipelineStageFlags,
    ) {
        let barrier = vk::ImageMemoryBarrier {
            src_access_mask,
            dst_access_mask,
            src_queue_family_index,
            dst_queue_family_index,
            old_layout: vk::ImageLayout::GENERAL,
            new_layout: vk::ImageLayout::GENERAL,
            image: self.color[current_frame].image,
            subresource_range: BASE_SUBRESOURCE_RANGE,
            ..Default::default()
        };

        unsafe {
            self.device.cmd_pipeline_barrier(
                cmd_buffer,
                src_stage_mask,
                dst_stage_mask,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }
    }

    pub fn submit(&self, current_frame: usize) {
        let semaphore = self.comp_finished_sem[current_frame];
        let fence = self.comp_finished_fences[current_frame];

        let submit_info = vk::SubmitInfo {
            command_buffer_count: 1,
            p_command_buffers: &self.cmd_buffers[current_frame],
            signal_semaphore_count: 1,
            p_signal_semaphores: &semaphore,
            ..Default::default()
        };

        unsafe {
            self.device
                .queue_submit(self.queue, &[submit_info], fence)
                .check_err("submit to compute queue");
        }
    }

    fn release_barrier_for_compute_queue(
        &self,
        cmd_buffer: vk::CommandBuffer,
        current_frame: usize,
    ) {
        if self.compute_queue_idx == self.graphics_queue_idx {
            return;
        }

        self.image_barrier(
            cmd_buffer,
            current_frame,
            self.compute_queue_idx,
            self.graphics_queue_idx,
            vk::AccessFlags::SHADER_WRITE,
            vk::AccessFlags::empty(),
            vk::PipelineStageFlags::COMPUTE_SHADER,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
        );
    }

    pub fn acquire_barrier_for_graphics_queue(
        &self,
        cmd_buffer: vk::CommandBuffer,
        current_frame: usize,
    ) {
        let (src_queue_family_index, dst_queue_family_index) =
            if self.graphics_queue_idx == self.compute_queue_idx {
                (vk::QUEUE_FAMILY_IGNORED, vk::QUEUE_FAMILY_IGNORED)
            } else {
                (self.compute_queue_idx, self.graphics_queue_idx)
            };

        self.image_barrier(
            cmd_buffer,
            current_frame,
            src_queue_family_index,
            dst_queue_family_index,
            vk::AccessFlags::SHADER_WRITE,
            vk::AccessFlags::SHADER_READ,
            vk::PipelineStageFlags::COMPUTE_SHADER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
        );
    }

    pub fn textures(&self) -> &[Texture] {
        &self.color
    }

    pub fn compute_finished(&self, current_frame: usize) -> vk::Semaphore {
        self.comp_finished_sem[current_frame]
    }

    pub fn push_const_mut(&mut self) -> Option<&mut PushConstType> {
        self.push_consts.as_mut().map(|x| x.data.as_mut())
    }

    pub fn update_data(&mut self, camera: &mut Camera, world: &mut World, current_frame: usize) {
        (self.update_data_cb)(self, camera, world, current_frame);
    }

    pub fn copy_to_buffer(&mut self, idx: usize, data: &[u32]) {
        let mapping = self.buffers[idx].mapping;

        unsafe {
            mapping.copy_from_nonoverlapping(data.as_ptr(), data.len());
        }
    }
}

impl Drop for ComputeTarget {
    fn drop(&mut self) {
        unsafe {
            for fence in &self.comp_finished_fences {
                self.device.destroy_fence(*fence, None);
            }

            for sem in &self.comp_finished_sem {
                self.device.destroy_semaphore(*sem, None);
            }

            self.device.destroy_command_pool(self.cmd_pool, None);
            self.device.destroy_descriptor_pool(self.desc_pool, None);
            self.device.destroy_descriptor_set_layout(self.desc_set_layout, None);
        }
    }
}

impl Drop for UniformBuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_descriptor_pool(self.desc_pool, None);
            self.device.destroy_descriptor_set_layout(self.desc_set_layout, None);
        }
    }
}

impl BufferMemory {
    fn new(
        device: &ash::Device,
        device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
        ub_type: &UniformBufferType,
        copies: usize,
    ) -> Self {
        let (buffers, memories, mappings) =
            create_uniform_buffers(device, device_mem_properties, copies);
        let data = Box::new(*ub_type);
        let size = ub_type.get_size() as u64;

        Self {
            device: device.clone(),
            buffers,
            memories,
            mappings,
            data,
            size,
        }
    }
}

impl Drop for BufferMemory {
    fn drop(&mut self) {
        unsafe {
            for buf in &self.buffers {
                self.device.destroy_buffer(*buf, None);
            }

            for mem in &self.memories {
                self.device.free_memory(*mem, None);
            }
        }
    }
}

impl ComputeBufferReadOnlyMemory {
    fn new(
        device: &ash::Device,
        device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
        items: u32,
    ) -> Self {
        let size = u64::from(items) * size_of::<u32>() as u64;
        let usage = vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST;
        let (buffers, memories, mappings) =
            create_host_visible_shader_buffers(device, device_mem_properties, usage, size, 1);

        let buffer = buffers[0];
        let memory = memories[0];
        let mapping = mappings[0];

        Self {
            device: device.clone(),
            buffer,
            memory,
            mapping,
            size,
        }
    }
}

impl Drop for ComputeBufferReadOnlyMemory {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_buffer(self.buffer, None);
            self.device.free_memory(self.memory, None);
        }
    }
}

impl UniformBufferType {
    fn get_size(&self) -> usize {
        match self {
            Self::ModelViewProj(_) => size_of::<ModelViewProjUBO>(),
        }
    }
}

impl PushConstType {
    const fn as_bytes(&self) -> &[u8] {
        unsafe {
            match self {
                Self::RayCast(x) => any_as_bytes(x),
                Self::Skybox(x) => any_as_bytes(x),
                Self::Crosshair(x) => any_as_bytes(x),
                Self::RayTrace(x) => any_as_bytes(x),
            }
        }
    }

    const fn size(&self) -> usize {
        match self {
            Self::RayCast(_) => size_of::<RayCastPushConstants>(),
            Self::Skybox(_) => size_of::<SkyboxPushConstants>(),
            Self::Crosshair(_) => size_of::<CrosshairPushConstants>(),
            Self::RayTrace(_) => size_of::<RayTracePushConstants>(),
        }
    }
}

const fn create_push_const_range(
    size: u32,
    stage_flags: vk::ShaderStageFlags,
) -> vk::PushConstantRange {
    vk::PushConstantRange {
        stage_flags,
        offset: 0,
        size,
    }
}

fn build_uniform_buffer(
    device: &ash::Device,
    device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
    shader_attachments: &[ShaderAttachment],
    copies: usize,
) -> Option<UniformBuffer> {
    if shader_attachments.is_empty() {
        return None;
    }

    let mut bindings = vec![];
    let mut pool_sizes = vec![];
    let mut buf_mem = None;

    for (binding, att) in shader_attachments.iter().enumerate() {
        let binding = to_u32(binding);

        match att {
            ShaderAttachment::UniformBuffer(u) => {
                bindings.push(uniform_binding(binding));
                pool_sizes.push(uniform_pool_size(copies));

                assert!(buf_mem.is_none(), "multiple uniform buffers are not supported");
                buf_mem = Some(BufferMemory::new(device, device_mem_properties, u, copies));
            }
            ShaderAttachment::Texture(_) | ShaderAttachment::Textures(_) => {
                bindings.push(sampler_binding(binding));
                pool_sizes.push(sampler_pool_size(copies));
            }
            ShaderAttachment::InputAttachment(_) => {
                bindings.push(input_att_binding(binding));
                pool_sizes.push(input_attachment_pool_size(copies));
            }
        }
    }

    let desc_set_layout = create_desc_set_layout(device, &bindings);
    let desc_pool = create_desc_pool(device, &pool_sizes, copies);
    let desc_sets = alloc_desc_sets(device, desc_pool, desc_set_layout, copies);

    fill_desc_set(device, shader_attachments, &desc_sets, &buf_mem, copies);

    let uniform_buffer = UniformBuffer {
        device: device.clone(),
        desc_set_layout,
        desc_pool,
        desc_sets,
        buf_mem,
    };

    Some(uniform_buffer)
}

const fn uniform_binding(binding: u32) -> vk::DescriptorSetLayoutBinding {
    desc_binding(binding, vk::DescriptorType::UNIFORM_BUFFER, vk::ShaderStageFlags::VERTEX)
}

const fn sampler_binding(binding: u32) -> vk::DescriptorSetLayoutBinding {
    desc_binding(
        binding,
        vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        vk::ShaderStageFlags::FRAGMENT,
    )
}

const fn input_att_binding(binding: u32) -> vk::DescriptorSetLayoutBinding {
    desc_binding(binding, vk::DescriptorType::INPUT_ATTACHMENT, vk::ShaderStageFlags::FRAGMENT)
}

const fn storage_image_binding(binding: u32) -> vk::DescriptorSetLayoutBinding {
    desc_binding(binding, vk::DescriptorType::STORAGE_IMAGE, vk::ShaderStageFlags::COMPUTE)
}

const fn storage_buffer_binding(binding: u32) -> vk::DescriptorSetLayoutBinding {
    desc_binding(binding, vk::DescriptorType::STORAGE_BUFFER, vk::ShaderStageFlags::COMPUTE)
}

const fn desc_binding(
    binding: u32,
    descriptor_type: vk::DescriptorType,
    stage_flags: vk::ShaderStageFlags,
) -> vk::DescriptorSetLayoutBinding {
    vk::DescriptorSetLayoutBinding {
        binding,
        descriptor_type,
        descriptor_count: 1,
        stage_flags,
        p_immutable_samplers: std::ptr::null(),
    }
}

const fn uniform_pool_size(count: usize) -> vk::DescriptorPoolSize {
    vk::DescriptorPoolSize {
        ty: vk::DescriptorType::UNIFORM_BUFFER,
        descriptor_count: to_u32(count),
    }
}

const fn sampler_pool_size(count: usize) -> vk::DescriptorPoolSize {
    vk::DescriptorPoolSize {
        ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        descriptor_count: to_u32(count),
    }
}

const fn input_attachment_pool_size(count: usize) -> vk::DescriptorPoolSize {
    vk::DescriptorPoolSize {
        ty: vk::DescriptorType::INPUT_ATTACHMENT,
        descriptor_count: to_u32(count),
    }
}

const fn storage_image_pool_size(count: usize) -> vk::DescriptorPoolSize {
    vk::DescriptorPoolSize {
        ty: vk::DescriptorType::STORAGE_IMAGE,
        descriptor_count: to_u32(count),
    }
}

const fn storage_buffer_pool_size(count: usize) -> vk::DescriptorPoolSize {
    vk::DescriptorPoolSize {
        ty: vk::DescriptorType::STORAGE_BUFFER,
        descriptor_count: to_u32(count),
    }
}

fn create_desc_set_layout(
    device: &ash::Device,
    bindings: &[vk::DescriptorSetLayoutBinding],
) -> vk::DescriptorSetLayout {
    let create_info = vk::DescriptorSetLayoutCreateInfo {
        binding_count: to_u32(bindings.len()),
        p_bindings: bindings.as_ptr(),
        ..Default::default()
    };

    unsafe { device.create_descriptor_set_layout(&create_info, None) }
        .check_err("create descriptor set layout")
}

fn create_desc_pool(
    device: &ash::Device,
    pool_sizes: &[vk::DescriptorPoolSize],
    max_sets: usize,
) -> vk::DescriptorPool {
    let create_info = vk::DescriptorPoolCreateInfo {
        max_sets: to_u32(max_sets),
        pool_size_count: to_u32(pool_sizes.len()),
        p_pool_sizes: pool_sizes.as_ptr(),
        ..Default::default()
    };

    unsafe { device.create_descriptor_pool(&create_info, None) }.check_err("create descriptor pool")
}

fn alloc_desc_sets(
    device: &ash::Device,
    descriptor_pool: vk::DescriptorPool,
    desc_set_layout: vk::DescriptorSetLayout,
    copies: usize,
) -> Vec<vk::DescriptorSet> {
    let layouts = vec![desc_set_layout; copies];

    let alloc_info = vk::DescriptorSetAllocateInfo {
        descriptor_pool,
        descriptor_set_count: to_u32(copies),
        p_set_layouts: layouts.as_ptr(),
        ..Default::default()
    };

    unsafe { device.allocate_descriptor_sets(&alloc_info) }.check_err("allocate descriptor sets")
}

fn fill_desc_set(
    device: &ash::Device,
    shader_attachments: &[ShaderAttachment],
    desc_sets: &[vk::DescriptorSet],
    buf_mem: &Option<BufferMemory>,
    copies: usize,
) {
    #[allow(clippy::needless_range_loop)]
    for frame in 0..copies {
        let mut buf_infos = vec![];
        let mut img_infos = vec![];
        let mut desc_writes = vec![];

        for (binding, att) in shader_attachments.iter().enumerate() {
            let binding = to_u32(binding);

            match att {
                ShaderAttachment::UniformBuffer(_) => {
                    let buf_mem = buf_mem.as_ref().unwrap_or_else(|| unreachable!());
                    let buf_info = buffer_desc_info(buf_mem.buffers[frame], buf_mem.size);
                    let buf_info = store(&mut buf_infos, buf_info);
                    let buf_write = buffer_desc_write(desc_sets[frame], binding, buf_info);

                    desc_writes.push(buf_write);
                }
                ShaderAttachment::Texture(t) => {
                    let img_info = sampler_desc_info(t);
                    let img_info = store(&mut img_infos, img_info);
                    let img_write = sampler_desc_write(desc_sets[frame], binding, img_info);

                    desc_writes.push(img_write);
                }
                ShaderAttachment::Textures(ts) => {
                    let t = &ts[frame];
                    let img_info = sampler_desc_info(t);
                    let img_info = store(&mut img_infos, img_info);
                    let img_write = sampler_desc_write(desc_sets[frame], binding, img_info);

                    desc_writes.push(img_write);
                }
                ShaderAttachment::InputAttachment(a) => {
                    let img_info = input_att_desc_info(a);
                    let img_info = store(&mut img_infos, img_info);
                    let att_write = input_att_desc_write(desc_sets[frame], binding, img_info);

                    desc_writes.push(att_write);
                }
            }
        }

        unsafe { device.update_descriptor_sets(&desc_writes, &[]) };
    }
}

fn store<T>(items: &mut Vec<T>, item: T) -> &T {
    items.push(item);
    unsafe { items.last().unwrap_unchecked() }
}

fn buffer_desc_info(buffer: vk::Buffer, range: u64) -> vk::DescriptorBufferInfo {
    vk::DescriptorBufferInfo {
        buffer,
        offset: 0,
        range,
    }
}

fn sampler_desc_info(texture: &Texture) -> vk::DescriptorImageInfo {
    vk::DescriptorImageInfo {
        sampler: texture.sampler,
        image_view: texture.image_view,
        image_layout: texture.layout,
    }
}

fn input_att_desc_info(attachment: &FramebufferAttachment) -> vk::DescriptorImageInfo {
    vk::DescriptorImageInfo {
        sampler: vk::Sampler::null(),
        image_view: attachment.image_view,
        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    }
}

fn buffer_desc_write(
    dst_set: vk::DescriptorSet,
    dst_binding: u32,
    buffer_info: &vk::DescriptorBufferInfo,
) -> vk::WriteDescriptorSet {
    vk::WriteDescriptorSet {
        dst_set,
        dst_binding,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
        p_buffer_info: buffer_info,
        ..Default::default()
    }
}

fn ssbo_desc_write(
    dst_set: vk::DescriptorSet,
    dst_binding: u32,
    buffer_info: &vk::DescriptorBufferInfo,
) -> vk::WriteDescriptorSet {
    vk::WriteDescriptorSet {
        dst_set,
        dst_binding,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
        p_buffer_info: buffer_info,
        ..Default::default()
    }
}

fn sampler_desc_write(
    dst_set: vk::DescriptorSet,
    dst_binding: u32,
    image_info: &vk::DescriptorImageInfo,
) -> vk::WriteDescriptorSet {
    vk::WriteDescriptorSet {
        dst_set,
        dst_binding,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        p_image_info: image_info,
        ..Default::default()
    }
}

fn input_att_desc_write(
    dst_set: vk::DescriptorSet,
    dst_binding: u32,
    image_info: &vk::DescriptorImageInfo,
) -> vk::WriteDescriptorSet {
    vk::WriteDescriptorSet {
        dst_set,
        dst_binding,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::INPUT_ATTACHMENT,
        p_image_info: image_info,
        ..Default::default()
    }
}

fn storage_img_desc_write(
    dst_set: vk::DescriptorSet,
    dst_binding: u32,
    image_info: &vk::DescriptorImageInfo,
) -> vk::WriteDescriptorSet {
    vk::WriteDescriptorSet {
        dst_set,
        dst_binding,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
        p_image_info: image_info,
        ..Default::default()
    }
}
