use std::ffi::CStr;
use std::ptr;

use ash::vk;

use super::{CheckError, SIZE_F32};
use crate::utils::*;

const SHADER_ENTRYPOINT: &CStr = cstr(b"main\0");

pub struct PipelineBuilder {
    device: ash::Device,
    stride: u32,
    vertex_descs: Vec<vk::VertexInputAttributeDescription>,
    topology: vk::PrimitiveTopology,
    polygon_mode: vk::PolygonMode,
    pipeline_layout: vk::PipelineLayout,
    render_pass: vk::RenderPass,
    subpass: u32,
}

pub struct Pipeline {
    device: ash::Device,
    pub inner: vk::Pipeline,
    pub layout: vk::PipelineLayout,
}

pub struct ShaderModule {
    device: ash::Device,
    inner: vk::ShaderModule,
}

impl Pipeline {
    pub fn new_compute(
        device: &ash::Device,
        push_const_range: Option<&vk::PushConstantRange>,
        desc_set_layout: vk::DescriptorSetLayout,
        shader_compiled: &[u8],
    ) -> Self {
        let shader = ShaderModule::new(device, shader_compiled);
        let stage = shader_stage_info(&shader, vk::ShaderStageFlags::COMPUTE);
        let layout = create_pipeline_layout(device, push_const_range, Some(&desc_set_layout));

        let create_info = vk::ComputePipelineCreateInfo {
            stage,
            layout,
            ..Default::default()
        };

        let res = unsafe {
            device.create_compute_pipelines(vk::PipelineCache::null(), &[create_info], None)
        };

        let inner = match res {
            Ok(pipelines) => pipelines[0],
            Err((_pipelines, err)) => panic!("failed to create compute pipeline: {}", err),
        };

        Self {
            device: device.clone(),
            inner,
            layout,
        }
    }
}

impl PipelineBuilder {
    pub fn new(
        device: &ash::Device,
        render_pass: vk::RenderPass,
        push_const_range: Option<&vk::PushConstantRange>,
        desc_set_layout: Option<&vk::DescriptorSetLayout>,
    ) -> Self {
        let device = device.clone();
        let stride = 0;
        let vertex_descs = vec![];
        let topology = vk::PrimitiveTopology::TRIANGLE_LIST;
        let polygon_mode = vk::PolygonMode::FILL;
        let pipeline_layout = create_pipeline_layout(&device, push_const_range, desc_set_layout);
        let subpass = 0;

        Self {
            device,
            stride,
            vertex_descs,
            topology,
            polygon_mode,
            pipeline_layout,
            render_pass,
            subpass,
        }
    }

    pub fn with_2_vertices(&mut self) -> &mut Self {
        let desc = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 0,
        };

        self.vertex_descs = vec![desc];
        self.stride = 2 * SIZE_F32;
        self
    }

    pub fn with_3_vertices(&mut self) -> &mut Self {
        let desc = vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: 0,
        };

        self.vertex_descs = vec![desc];
        self.stride = 3 * SIZE_F32;
        self
    }

    #[allow(unused)]
    pub fn with_stride_exact(&mut self, stride: u32) -> &mut Self {
        self.stride = stride;
        self
    }

    pub fn with_stride_in_f32s(&mut self, f32s: u32) -> &mut Self {
        self.stride = f32s * SIZE_F32;
        self
    }

    pub fn add_vertex_desc(&mut self, desc: vk::VertexInputAttributeDescription) -> &mut Self {
        self.vertex_descs.push(desc);
        self
    }

    pub fn with_topology(&mut self, topology: vk::PrimitiveTopology) -> &mut Self {
        self.topology = topology;
        self
    }

    #[allow(unused)]
    pub fn with_polygon_mode(&mut self, polygon_mode: vk::PolygonMode) -> &mut Self {
        self.polygon_mode = polygon_mode;
        self
    }

    pub fn with_subpass(&mut self, subpass: u32) -> &mut Self {
        self.subpass = subpass;
        self
    }

    pub fn build(&self, vert_shader: &ShaderModule, frag_shader: &ShaderModule) -> Pipeline {
        let vert_shader_stage = shader_stage_info(vert_shader, vk::ShaderStageFlags::VERTEX);
        let frag_shader_stage = shader_stage_info(frag_shader, vk::ShaderStageFlags::FRAGMENT);

        let binding_desc = vk::VertexInputBindingDescription {
            binding: 0,
            stride: self.stride,
            input_rate: vk::VertexInputRate::VERTEX,
        };

        let shader_stages = [vert_shader_stage, frag_shader_stage];

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let vertex_input_state = vertex_input_state_info(&binding_desc, &self.vertex_descs);
        let input_assembly_state = default_input_assembly(self.topology);
        let viewport_state = viewport_state_info();
        let rasterization_state = rasterization_info(self.polygon_mode);
        let multisample_state = no_multisampling();
        let stencil_state = no_stencil_state();
        let depth_state = depth_test(stencil_state);
        let color_blend_attachment = no_color_blending();
        let color_blend_state = color_blend_info(&color_blend_attachment);
        let dynamic_state = dynamic_state_info(&dynamic_states);

        let create_info = [vk::GraphicsPipelineCreateInfo {
            stage_count: to_u32(shader_stages.len()),
            p_stages: shader_stages.as_ptr(),
            p_vertex_input_state: &vertex_input_state,
            p_input_assembly_state: &input_assembly_state,
            p_viewport_state: &viewport_state,
            p_rasterization_state: &rasterization_state,
            p_multisample_state: &multisample_state,
            p_depth_stencil_state: &depth_state,
            p_color_blend_state: &color_blend_state,
            p_dynamic_state: &dynamic_state,
            layout: self.pipeline_layout,
            render_pass: self.render_pass,
            subpass: self.subpass,
            base_pipeline_index: -1,
            ..Default::default()
        }];

        let graphics_pipelines = unsafe {
            self.device.create_graphics_pipelines(vk::PipelineCache::null(), &create_info, None)
        };

        let inner = match graphics_pipelines {
            Ok(pipelines) => pipelines[0],
            Err((_pipelines, err)) => panic!("failed to create pipeline: {}", err),
        };

        Pipeline {
            device: self.device.clone(),
            inner,
            layout: self.pipeline_layout,
        }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_pipeline(self.inner, None);
            self.device.destroy_pipeline_layout(self.layout, None);
        }
    }
}

impl ShaderModule {
    pub fn new(device: &ash::Device, code: &[u8]) -> Self {
        let transmuted_copy = pack_to_u32s(code);

        let create_info = vk::ShaderModuleCreateInfo {
            code_size: code.len(),
            p_code: transmuted_copy.as_ptr(),
            ..Default::default()
        };

        let device = device.clone();

        let inner = unsafe { device.create_shader_module(&create_info, None) }
            .check_err("create shader module");

        Self { device, inner }
    }
}

impl Drop for ShaderModule {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_shader_module(self.inner, None);
        }
    }
}

fn shader_stage_info(
    module: &ShaderModule,
    stage: vk::ShaderStageFlags,
) -> vk::PipelineShaderStageCreateInfo {
    vk::PipelineShaderStageCreateInfo {
        stage,
        module: module.inner,
        p_name: SHADER_ENTRYPOINT.as_ptr(),
        ..Default::default()
    }
}

fn create_pipeline_layout(
    device: &ash::Device,
    push_const_range: Option<&vk::PushConstantRange>,
    desc_set_layout: Option<&vk::DescriptorSetLayout>,
) -> vk::PipelineLayout {
    let (push_constant_range_count, p_push_constant_ranges) = match push_const_range {
        Some(range) => (1, range as *const _),
        None => (0, ptr::null()),
    };

    let (set_layout_count, p_set_layouts) = match desc_set_layout {
        Some(set_layout) => (1, set_layout as *const _),
        None => (0, ptr::null()),
    };

    let create_info = vk::PipelineLayoutCreateInfo {
        set_layout_count,
        p_set_layouts,
        push_constant_range_count,
        p_push_constant_ranges,
        ..Default::default()
    };

    unsafe { device.create_pipeline_layout(&create_info, None) }.check_err("create pipeline layout")
}

fn vertex_input_state_info(
    binding_desc: &vk::VertexInputBindingDescription,
    attribute_desc: &[vk::VertexInputAttributeDescription],
) -> vk::PipelineVertexInputStateCreateInfo {
    vk::PipelineVertexInputStateCreateInfo {
        vertex_binding_description_count: 1,
        p_vertex_binding_descriptions: binding_desc,
        vertex_attribute_description_count: to_u32(attribute_desc.len()),
        p_vertex_attribute_descriptions: attribute_desc.as_ptr(),
        ..Default::default()
    }
}

fn default_input_assembly(
    topology: vk::PrimitiveTopology,
) -> vk::PipelineInputAssemblyStateCreateInfo {
    vk::PipelineInputAssemblyStateCreateInfo {
        topology,
        primitive_restart_enable: vk::FALSE,
        ..Default::default()
    }
}

fn viewport_state_info() -> vk::PipelineViewportStateCreateInfo {
    vk::PipelineViewportStateCreateInfo {
        viewport_count: 1,
        scissor_count: 1,
        ..Default::default()
    }
}

fn rasterization_info(polygon_mode: vk::PolygonMode) -> vk::PipelineRasterizationStateCreateInfo {
    vk::PipelineRasterizationStateCreateInfo {
        depth_clamp_enable: vk::FALSE,
        rasterizer_discard_enable: vk::FALSE,
        polygon_mode,
        cull_mode: vk::CullModeFlags::BACK,
        front_face: vk::FrontFace::COUNTER_CLOCKWISE,
        depth_bias_enable: vk::FALSE,
        line_width: 1.0,
        ..Default::default()
    }
}

fn no_multisampling() -> vk::PipelineMultisampleStateCreateInfo {
    vk::PipelineMultisampleStateCreateInfo {
        rasterization_samples: vk::SampleCountFlags::TYPE_1,
        sample_shading_enable: vk::FALSE,
        min_sample_shading: 0.0,
        p_sample_mask: ptr::null(),
        alpha_to_coverage_enable: vk::FALSE,
        alpha_to_one_enable: vk::FALSE,
        ..Default::default()
    }
}

const fn no_stencil_state() -> vk::StencilOpState {
    vk::StencilOpState {
        fail_op: vk::StencilOp::KEEP,
        pass_op: vk::StencilOp::KEEP,
        depth_fail_op: vk::StencilOp::KEEP,
        compare_op: vk::CompareOp::ALWAYS,
        compare_mask: 0,
        write_mask: 0,
        reference: 0,
    }
}

fn depth_test(stencil_state: vk::StencilOpState) -> vk::PipelineDepthStencilStateCreateInfo {
    vk::PipelineDepthStencilStateCreateInfo {
        depth_test_enable: vk::TRUE,
        depth_write_enable: vk::TRUE,
        depth_compare_op: vk::CompareOp::LESS,
        depth_bounds_test_enable: vk::FALSE,
        min_depth_bounds: 0.0,
        max_depth_bounds: 1.0,
        stencil_test_enable: vk::FALSE,
        front: stencil_state,
        back: stencil_state,
        ..Default::default()
    }
}

fn no_color_blending() -> vk::PipelineColorBlendAttachmentState {
    vk::PipelineColorBlendAttachmentState {
        blend_enable: vk::FALSE,
        color_write_mask: vk::ColorComponentFlags::RGBA,
        ..Default::default()
    }
}

fn color_blend_info(
    color_blend_attachment: &vk::PipelineColorBlendAttachmentState,
) -> vk::PipelineColorBlendStateCreateInfo {
    vk::PipelineColorBlendStateCreateInfo {
        logic_op_enable: vk::FALSE,
        attachment_count: 1,
        p_attachments: color_blend_attachment,
        ..Default::default()
    }
}

fn dynamic_state_info(dynamic_state: &[vk::DynamicState]) -> vk::PipelineDynamicStateCreateInfo {
    vk::PipelineDynamicStateCreateInfo {
        dynamic_state_count: to_u32(dynamic_state.len()),
        p_dynamic_states: dynamic_state.as_ptr(),
        ..Default::default()
    }
}
