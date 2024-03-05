use ash::vk;

use crate::utils::*;

pub struct RenderPassBuilder {
    attachments: Vec<vk::AttachmentDescription>,
    subpasses: Vec<SubpassBuilder>,
    dependencies: Vec<vk::SubpassDependency>,
}

pub struct SubpassBuilder {
    color_attachment: Option<vk::AttachmentReference>,
    depth_stencil_attachment: Option<vk::AttachmentReference>,
    input_attachments: Vec<vk::AttachmentReference>,
}

pub struct DependencyBuilder(vk::SubpassDependency);

impl RenderPassBuilder {
    pub fn new() -> Self {
        Self {
            attachments: vec![],
            subpasses: vec![],
            dependencies: vec![],
        }
    }

    pub fn with_attachment(
        &mut self,
        format: vk::Format,
        load_op: vk::AttachmentLoadOp,
        store_op: vk::AttachmentStoreOp,
        final_layout: vk::ImageLayout,
    ) -> &mut Self {
        let attachment = vk::AttachmentDescription {
            flags: vk::AttachmentDescriptionFlags::empty(),
            format,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op,
            store_op,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout,
        };

        self.attachments.push(attachment);
        self
    }

    pub fn with_subpass(&mut self, subpass: SubpassBuilder) -> &mut Self {
        self.subpasses.push(subpass);
        self
    }

    pub fn with_dependency(&mut self, dependency: vk::SubpassDependency) -> &mut Self {
        self.dependencies.push(dependency);
        self
    }

    pub fn build(&mut self, device: &ash::Device) -> vk::RenderPass {
        let subpasses = self.subpasses.iter().map(SubpassBuilder::build).collect::<Vec<_>>();

        let create_info = vk::RenderPassCreateInfo {
            attachment_count: to_u32(self.attachments.len()),
            p_attachments: self.attachments.as_ptr(),
            subpass_count: to_u32(self.subpasses.len()),
            p_subpasses: subpasses.as_ptr(),
            dependency_count: to_u32(self.dependencies.len()),
            p_dependencies: self.dependencies.as_ptr(),
            ..Default::default()
        };

        unsafe { device.create_render_pass(&create_info, None) }.check_err("create render pass")
    }
}

impl SubpassBuilder {
    pub fn new() -> Self {
        Self {
            color_attachment: None,
            depth_stencil_attachment: None,
            input_attachments: vec![],
        }
    }

    pub fn with_color_attachment(mut self, attachment: u32) -> Self {
        let layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
        let attachment_ref = vk::AttachmentReference { attachment, layout };

        self.color_attachment = Some(attachment_ref);
        self
    }

    pub fn with_depth_attachment(mut self, attachment: u32) -> Self {
        let layout = vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;
        let attachment_ref = vk::AttachmentReference { attachment, layout };

        self.depth_stencil_attachment = Some(attachment_ref);
        self
    }

    pub fn with_input_attachment(mut self, attachment: u32, layout: vk::ImageLayout) -> Self {
        let attachment_ref = vk::AttachmentReference { attachment, layout };
        self.input_attachments.push(attachment_ref);
        self
    }

    pub fn build(&self) -> vk::SubpassDescription {
        let color_attachment_count = u32::from(self.color_attachment.is_some());
        let p_color_attachments = opt_to_ptr(&self.color_attachment);
        let p_depth_stencil_attachment = opt_to_ptr(&self.depth_stencil_attachment);

        vk::SubpassDescription {
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
            input_attachment_count: to_u32(self.input_attachments.len()),
            p_input_attachments: self.input_attachments.as_ptr(),
            color_attachment_count,
            p_color_attachments,
            p_depth_stencil_attachment,
            ..Default::default()
        }
    }
}

impl DependencyBuilder {
    pub fn new() -> Self {
        Self(vk::SubpassDependency::default())
    }

    pub fn subpasses(mut self, src: u32, dst: u32) -> Self {
        self.0.src_subpass = src;
        self.0.dst_subpass = dst;
        self
    }

    pub fn stage_masks(mut self, src: vk::PipelineStageFlags, dst: vk::PipelineStageFlags) -> Self {
        self.0.src_stage_mask = src;
        self.0.dst_stage_mask = dst;
        self
    }

    pub fn access_masks(mut self, src: vk::AccessFlags, dst: vk::AccessFlags) -> Self {
        self.0.src_access_mask = src;
        self.0.dst_access_mask = dst;
        self
    }

    pub fn build(self) -> vk::SubpassDependency {
        self.0
    }
}

pub fn create_render_pass_with_attachments(
    device: &ash::Device,
    swapchain_format: vk::Format,
    depth_format: vk::Format,
) -> vk::RenderPass {
    RenderPassBuilder::new()
        // Final swapchain image color attachment
        .with_attachment(
            swapchain_format,
            vk::AttachmentLoadOp::CLEAR,
            vk::AttachmentStoreOp::STORE,
            vk::ImageLayout::PRESENT_SRC_KHR,
        )
        // Input attachments: these will be written to in the first subpass, then transitioned to
        // input attachments and read in the second subpass
        .with_attachment(
            swapchain_format,
            vk::AttachmentLoadOp::CLEAR,
            vk::AttachmentStoreOp::DONT_CARE,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )
        .with_attachment(
            depth_format,
            vk::AttachmentLoadOp::CLEAR,
            vk::AttachmentStoreOp::DONT_CARE,
            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        )
        // First subpass: write to color and depth attachments
        .with_subpass(SubpassBuilder::new().with_color_attachment(1).with_depth_attachment(2))
        // Second subpass: read color and depth attachments and write to swapchain color attachment
        .with_subpass(
            SubpassBuilder::new()
                .with_color_attachment(0)
                .with_input_attachment(1, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .with_input_attachment(2, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
        )
        // Subpass dependencies for layout transitions
        .with_dependency(
            DependencyBuilder::new()
                .subpasses(vk::SUBPASS_EXTERNAL, 0)
                .stage_masks(
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                )
                .access_masks(
                    vk::AccessFlags::MEMORY_READ,
                    vk::AccessFlags::COLOR_ATTACHMENT_READ
                        | vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                        | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                )
                .build(),
        )
        // Transition input attachment from color attachment to shader read
        .with_dependency(
            DependencyBuilder::new()
                .subpasses(0, 1)
                .stage_masks(
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                )
                .access_masks(vk::AccessFlags::COLOR_ATTACHMENT_WRITE, vk::AccessFlags::SHADER_READ)
                .build(),
        )
        .with_dependency(
            DependencyBuilder::new()
                .subpasses(0, vk::SUBPASS_EXTERNAL)
                .stage_masks(
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                )
                .access_masks(
                    vk::AccessFlags::COLOR_ATTACHMENT_READ
                        | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                    vk::AccessFlags::MEMORY_READ,
                )
                .build(),
        )
        .build(device)
}

pub fn create_render_pass_no_attachments(
    device: &ash::Device,
    swapchain_format: vk::Format,
    depth_format: vk::Format,
) -> vk::RenderPass {
    RenderPassBuilder::new()
        .with_attachment(
            swapchain_format,
            vk::AttachmentLoadOp::CLEAR,
            vk::AttachmentStoreOp::STORE,
            vk::ImageLayout::PRESENT_SRC_KHR,
        )
        .with_attachment(
            depth_format,
            vk::AttachmentLoadOp::CLEAR,
            vk::AttachmentStoreOp::DONT_CARE,
            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        )
        .with_subpass(SubpassBuilder::new().with_color_attachment(0).with_depth_attachment(1))
        .with_dependency(
            DependencyBuilder::new()
                .subpasses(vk::SUBPASS_EXTERNAL, 0)
                .stage_masks(
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                )
                .access_masks(
                    vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                        | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                )
                .build(),
        )
        .build(device)
}
