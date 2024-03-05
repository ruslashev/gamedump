use std::ffi::{c_void, CStr, CString};
use std::mem::size_of_val;
use std::ptr;

use anyhow::Result;
use ash::extensions::ext::DebugUtils;
use ash::extensions::khr::{Surface as VkSurface, Swapchain as VkSwapchain};
use ash::vk;
use log::{debug, error, info, warn};

use super::*;
use crate::logger;
use crate::utils::*;
use crate::window::Window;

const API_VER_MAJOR: u32 = 1;
const API_VER_MINOR: u32 = 2;
const API_VER_PATCH: u32 = 0;

const REQ_VALIDATION_LAYERS: &[&str] = &[
    "VK_LAYER_KHRONOS_validation",
    // "VK_LAYER_LUNARG_gfxreconstruct",
];

const REQ_DEVICE_EXTENSIONS: &[&str] = &[
    "VK_KHR_swapchain",
    #[cfg(target_os = "macos")]
    "VK_KHR_portability_subset",
    #[cfg(debug_assertions)]
    "VK_KHR_shader_non_semantic_info",
];

pub const BASE_SUBRESOURCE_RANGE: vk::ImageSubresourceRange = vk::ImageSubresourceRange {
    aspect_mask: vk::ImageAspectFlags::COLOR,
    base_mip_level: 0,
    level_count: 1,
    base_array_layer: 0,
    layer_count: 1,
};

pub const CLEAR_COLOR: vk::ClearValue = vk::ClearValue {
    color: vk::ClearColorValue {
        float32: [0.0, 0.0, 0.0, 0.0],
    },
};

pub const CLEAR_DEPTH: vk::ClearValue = vk::ClearValue {
    depth_stencil: vk::ClearDepthStencilValue {
        depth: 1.0,
        stencil: 0,
    },
};

pub const ONE_TIME_SUBMIT: vk::CommandBufferBeginInfo = vk::CommandBufferBeginInfo {
    flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
    s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
    p_next: ptr::null(),
    p_inheritance_info: ptr::null(),
};

pub struct Surface {
    loader: VkSurface,
    handle: vk::SurfaceKHR,
}

pub struct Swapchain {
    device: ash::Device,
    pub loader: VkSwapchain,
    pub handle: vk::SwapchainKHR,
    pub format: vk::SurfaceFormatKHR,
    pub extent: vk::Extent2D,
    pub image_views: Vec<vk::ImageView>,
}

pub struct Texture {
    device: ash::Device,
    pub image: vk::Image,
    memory: vk::DeviceMemory,
    pub image_view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub layout: vk::ImageLayout,
    pub format: vk::Format,
}

pub struct FramebufferAttachment {
    device: ash::Device,
    image: vk::Image,
    memory: vk::DeviceMemory,
    pub image_view: vk::ImageView,
    pub format: vk::Format,
}

#[derive(Default, Clone, Copy)]
pub struct QueueFamilyIndices {
    pub graphics: u32,
    pub compute: u32,
    pub present: u32,
    pub transfer: u32,
}

pub struct Queues {
    pub graphics: vk::Queue,
    pub compute: vk::Queue,
    pub present: vk::Queue,
}

#[derive(Clone, Copy)]
pub struct PhysDeviceInfo {
    pub phys_device: vk::PhysicalDevice,
    properties: vk::PhysicalDeviceProperties,
    pub queue_family_indices: QueueFamilyIndices,
}

#[derive(Default)]
struct QueueFamilyData {
    graphics: bool,
    compute: bool,
    present: bool,
    transfer: bool,
    special_granularity: Option<vk::Extent3D>,
}

impl Surface {
    pub fn new(entry: &ash::Entry, instance: &ash::Instance, window: &Window) -> Result<Self> {
        let loader = VkSurface::new(entry, instance);
        let handle = window.create_surface(instance)?;

        Ok(Self { loader, handle })
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            self.loader.destroy_surface(self.handle, None);
        }
    }
}

impl Swapchain {
    pub fn new(
        phys_device: vk::PhysicalDevice,
        surface: &Surface,
        swapchain_format: vk::SurfaceFormatKHR,
        win_width: u32,
        win_height: u32,
        instance: &ash::Instance,
        device: &ash::Device,
        queue_family_indices: &QueueFamilyIndices,
    ) -> Self {
        let surface_capabilities = get_surface_capabilities(phys_device, surface);
        let extent = choose_swapchain_extent(win_width, win_height, &surface_capabilities);
        let loader = VkSwapchain::new(instance, device);
        let present_mode = choose_swapchain_present_mode(phys_device, surface);
        let handle = create_swapchain(
            surface,
            present_mode,
            &surface_capabilities,
            swapchain_format,
            extent,
            &loader,
            queue_family_indices,
        );
        let images = get_swapchain_images(&loader, handle);
        let image_views = create_swapchain_image_views(device, swapchain_format.format, &images);

        Self {
            device: device.clone(),
            format: swapchain_format,
            loader,
            extent,
            handle,
            image_views,
        }
    }

    pub fn present(
        &self,
        wait_semaphore: vk::Semaphore,
        image_index: u32,
        present: vk::Queue,
    ) -> Result<bool, ash::vk::Result> {
        let present_info = vk::PresentInfoKHR {
            wait_semaphore_count: 1,
            p_wait_semaphores: &wait_semaphore,
            swapchain_count: 1,
            p_swapchains: &self.handle,
            p_image_indices: &image_index,
            ..Default::default()
        };

        unsafe { self.loader.queue_present(present, &present_info) }
    }

    pub unsafe fn destroy(&mut self) {
        for image_view in &self.image_views {
            self.device.destroy_image_view(*image_view, None);
        }

        self.loader.destroy_swapchain(self.handle, None);
    }
}

impl Texture {
    pub fn new(
        device: &ash::Device,
        device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
        path: &'static str,
    ) -> Self {
        let (image, memory, layout) =
            create_texture_image(device, device_mem_properties, command_pool, queue, path);
        let format = vk::Format::R8G8B8A8_SRGB;
        let image_view = create_image_view(device, image, format, vk::ImageAspectFlags::COLOR, 1);
        let sampler = create_texture_sampler(device);

        Self {
            device: device.clone(),
            image,
            memory,
            image_view,
            sampler,
            layout,
            format,
        }
    }

    pub fn new_compute(
        instance: &ash::Instance,
        phys_device: vk::PhysicalDevice,
        format_candidates: &[vk::Format],
        device: &ash::Device,
        device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
        command_pool: vk::CommandPool,
        queue: vk::Queue,
        width: u32,
        height: u32,
    ) -> Self {
        let format = find_supported_format(
            instance,
            phys_device,
            format_candidates,
            true,
            vk::FormatFeatureFlags::STORAGE_IMAGE,
        )
        .check_err("find compute texture format");

        // Cleared as transfer dest, stored in compute shader, sampled in fragment shader
        let usage = vk::ImageUsageFlags::TRANSFER_DST
            | vk::ImageUsageFlags::STORAGE
            | vk::ImageUsageFlags::SAMPLED;
        let (image, memory) =
            create_image(device, device_mem_properties, format, width, height, usage);

        // Must be GENERAL because of STORAGE_IMAGE
        let layout = vk::ImageLayout::GENERAL;

        transition_image_layout(
            device,
            command_pool,
            queue,
            image,
            vk::ImageLayout::UNDEFINED,
            layout,
        );

        let image_view = create_image_view(device, image, format, vk::ImageAspectFlags::COLOR, 1);
        let sampler = create_texture_sampler(device);

        Self {
            device: device.clone(),
            image,
            memory,
            image_view,
            sampler,
            layout,
            format,
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_sampler(self.sampler, None);
            self.device.destroy_image_view(self.image_view, None);
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.memory, None);
        }
    }
}

impl FramebufferAttachment {
    pub fn new(
        device: &ash::Device,
        device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
        extent: vk::Extent2D,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        aspect_mask: vk::ImageAspectFlags,
    ) -> Self {
        if !usage.contains(vk::ImageUsageFlags::INPUT_ATTACHMENT) {
            warn!("FramebufferAttachment has no INPUT_ATTACHMENT usage");
        }

        let (image, memory) =
            create_image(device, device_mem_properties, format, extent.width, extent.height, usage);

        let image_view = create_image_view(device, image, format, aspect_mask, 1);

        Self {
            device: device.clone(),
            format,
            image,
            memory,
            image_view,
        }
    }
}

impl Drop for FramebufferAttachment {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_image_view(self.image_view, None);
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.memory, None);
        }
    }
}

pub fn create_instance(app_name: &str, entry: &ash::Entry, window: &Window) -> ash::Instance {
    let app_cstring = CString::new(app_name).check_err("convert app_name to CString");
    let app_cstr = app_cstring.as_c_str();

    let engine_name = cstr(b"engine\0");

    let (eng_ver_major, eng_ver_minor, eng_ver_patch) = get_version();
    let engine_version = vk::make_api_version(0, eng_ver_major, eng_ver_minor, eng_ver_patch);

    let api_version = vk::make_api_version(0, API_VER_MAJOR, API_VER_MINOR, API_VER_PATCH);

    let app_info = vk::ApplicationInfo {
        p_application_name: app_cstr.as_ptr(),
        application_version: 0,
        p_engine_name: engine_name.as_ptr(),
        engine_version,
        api_version,
        ..Default::default()
    };

    print_instance_version(entry);

    let validation_layers = get_validation_layers(entry);

    let req_layers_owned = convert_to_strings(&validation_layers);
    let req_layers_cstrs = convert_to_c_strs(&req_layers_owned);
    let req_layers_cptrs = convert_to_c_ptrs(&req_layers_cstrs);

    let req_inst_exts_owned = window.get_required_extensions();
    let req_inst_exts_cstrs = convert_to_c_strs(&req_inst_exts_owned);

    #[allow(unused_mut)]
    let mut req_inst_exts_cptrs = convert_to_c_ptrs(&req_inst_exts_cstrs);

    let flags = if cfg!(target_os = "macos") {
        req_inst_exts_cptrs.push(vk::KhrPortabilityEnumerationFn::name().as_ptr());
        req_inst_exts_cptrs.push(vk::KhrGetPhysicalDeviceProperties2Fn::name().as_ptr());

        std::env::set_var("MVK_CONFIG_USE_METAL_ARGUMENT_BUFFERS", "0");

        vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
    } else {
        vk::InstanceCreateFlags::default()
    };

    if cfg!(debug_assertions) {
        req_inst_exts_cptrs.push(vk::ExtDebugUtilsFn::name().as_ptr());
    }

    print_instance_extensions(entry, &req_inst_exts_cptrs);

    let val_features = [vk::ValidationFeatureEnableEXT::DEBUG_PRINTF];

    let val_features_info = vk::ValidationFeaturesEXT {
        enabled_validation_feature_count: to_u32(val_features.len()),
        p_enabled_validation_features: val_features.as_ptr(),
        ..Default::default()
    };

    let create_info = vk::InstanceCreateInfo {
        p_application_info: &app_info,
        enabled_layer_count: to_u32(req_layers_cptrs.len()),
        pp_enabled_layer_names: req_layers_cptrs.as_ptr(),
        enabled_extension_count: to_u32(req_inst_exts_cptrs.len()),
        pp_enabled_extension_names: req_inst_exts_cptrs.as_ptr(),
        flags,
        p_next: ptr::addr_of!(val_features_info).cast(),
        ..Default::default()
    };

    unsafe { entry.create_instance(&create_info, None) }.check_err("create instance")
}

const fn get_version() -> (u32, u32, u32) {
    let major = str_to_u32(env!("CARGO_PKG_VERSION_MAJOR"));
    let minor = str_to_u32(env!("CARGO_PKG_VERSION_MINOR"));
    let patch = str_to_u32(env!("CARGO_PKG_VERSION_PATCH"));

    (major, minor, patch)
}

fn print_instance_version(entry: &ash::Entry) {
    match entry.try_enumerate_instance_version().check_err("get instance version") {
        Some(version) => {
            let major = vk::api_version_major(version);
            let minor = vk::api_version_minor(version);
            let patch = vk::api_version_patch(version);
            debug!("Instance version: {}.{}.{}", major, minor, patch);
        }
        None => {
            debug!("Instance version: 1.0");
        }
    }
}

fn get_validation_layers(entry: &ash::Entry) -> Vec<&str> {
    if !cfg!(debug_assertions) {
        return vec![];
    }

    let req_layers = REQ_VALIDATION_LAYERS;
    let sup_layers = entry.enumerate_instance_layer_properties().check_err("get validation layers");

    if logger::verbose() {
        print_textual_items("Supported validation layers", &sup_layers, |layer| {
            layer.layer_name.as_ptr()
        });
    }

    let mut layers = vec![];

    // Ensure all required validation layers are supported
    for sup in sup_layers {
        let cstr = unsafe { CStr::from_ptr(sup.layer_name.as_ptr()) };
        let name = cstr.to_str().unwrap_or("unknown");

        for req in req_layers {
            if name == *req {
                layers.push(*req);
            }
        }
    }

    layers
}

fn print_instance_extensions(entry: &ash::Entry, req_inst_exts_cptrs: &[*const i8]) {
    print_textual_items("Required instance extensions", req_inst_exts_cptrs, |x| *x);

    if logger::verbose() {
        let extensions = entry
            .enumerate_instance_extension_properties(None)
            .check_err("enumerate instance extensions");

        print_textual_items("Supported instance extensions", &extensions, |ext| {
            ext.extension_name.as_ptr()
        });
    }
}

pub fn create_debug_data(entry: &ash::Entry, instance: &ash::Instance) -> Option<DebugData> {
    if !cfg!(debug_assertions) {
        return None;
    }

    let message_severity = vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
        | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;

    let message_type = vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE;

    let debug_info = vk::DebugUtilsMessengerCreateInfoEXT {
        message_severity,
        message_type,
        pfn_user_callback: Some(debug_callback),
        ..Default::default()
    };

    let debug_utils_loader = DebugUtils::new(entry, instance);

    let debug_messenger =
        unsafe { debug_utils_loader.create_debug_utils_messenger(&debug_info, None) }
            .check_err("create debug messenger");

    let data = DebugData {
        debug_utils_loader,
        debug_messenger,
    };

    Some(data)
}

unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    ty: vk::DebugUtilsMessageTypeFlagsEXT,
    cb_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut c_void,
) -> vk::Bool32 {
    let cb_data = *cb_data;
    let id_name = cstr_to_cow(cb_data.p_message_id_name);
    let msg = cstr_to_cow(cb_data.p_message);

    if ty == vk::DebugUtilsMessageTypeFlagsEXT::GENERAL && id_name == "Loader Message" {
        return vk::FALSE;
    }

    let queues =
        format_debug_items("queues", cb_data.p_queue_labels, cb_data.queue_label_count, label_fmt);

    let cmd_bufs = format_debug_items(
        "cmd buffers",
        cb_data.p_cmd_buf_labels,
        cb_data.cmd_buf_label_count,
        label_fmt,
    );

    let text = if ty == vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION {
        format!("{msg}")
    } else {
        let objs = format_debug_items("objects", cb_data.p_objects, cb_data.object_count, obj_fmt);
        format!("VK [{ty:?} {id_name}]{queues}{cmd_bufs}{objs} {msg}")
    };

    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        error!("{}", text);
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        warn!("{}", text);
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::INFO) {
        info!("{}", text);
    } else {
        debug!("{}", text);
    }

    vk::FALSE
}

fn label_fmt(label: &vk::DebugUtilsLabelEXT) -> String {
    let cstr = unsafe { CStr::from_ptr(label.p_label_name) };
    format!("{:?}", cstr.to_string_lossy())
}

fn obj_fmt(obj: &vk::DebugUtilsObjectNameInfoEXT) -> String {
    format!("{:?}({:#x})", obj.object_type, obj.object_handle)
}

fn format_debug_items<T>(
    id: &str,
    data: *const T,
    len: u32,
    printer: impl Fn(&T) -> String,
) -> String {
    let labels = unsafe { std::slice::from_raw_parts(data, len as usize) };

    if labels.is_empty() {
        return String::new();
    }

    let items = labels.iter().map(printer).intersperse(", ".to_owned()).collect::<String>();

    format!(" [{}: {}]", id, items)
}

pub fn pick_phys_device(instance: &ash::Instance, surface: &Surface) -> PhysDeviceInfo {
    let phys_devices =
        unsafe { instance.enumerate_physical_devices() }.check_err("get physical devices");
    let mut phys_device_infos = gather_phys_device_infos(instance, surface, &phys_devices);

    assert!(!phys_device_infos.is_empty(), "no suitable devices found");

    phys_device_infos.sort_by_key(|d| device_type_to_priority(d.properties.device_type));

    let phys_device_info = phys_device_infos[0];

    print_phys_device_info(instance, surface, &phys_device_info);

    phys_device_info
}

fn print_phys_device_info(instance: &ash::Instance, surface: &Surface, info: &PhysDeviceInfo) {
    let phys_device = info.phys_device;
    let properties = unsafe { instance.get_physical_device_properties(phys_device) };
    let device_name = unsafe { CStr::from_ptr(properties.device_name.as_ptr()) };

    let api_version = properties.api_version;
    let major = vk::api_version_major(api_version);
    let minor = vk::api_version_minor(api_version);
    let patch = vk::api_version_patch(api_version);

    debug!("Chosen physical device {:?}:", device_name);
    debug!(
        "Type: {:?}, Vendor ID: {:#x}, Device ID: {:#x}",
        properties.device_type, properties.vendor_id, properties.device_id
    );
    debug!(
        "API version: {}.{}.{}, Driver version: {}",
        major, minor, patch, properties.driver_version
    );

    if logger::verbose() {
        print_queue_family_infos(instance, phys_device, surface);
    }

    debug!(
        "Queue family indices: graphics={}, compute={}, present={}, transfer={}",
        info.queue_family_indices.graphics,
        info.queue_family_indices.compute,
        info.queue_family_indices.present,
        info.queue_family_indices.transfer,
    );
}

fn print_queue_family_infos(
    instance: &ash::Instance,
    phys_device: vk::PhysicalDevice,
    surface: &Surface,
) {
    let families = unsafe { instance.get_physical_device_queue_family_properties(phys_device) };

    for (i, f) in families.iter().enumerate() {
        let present_support = unsafe {
            surface
                .loader
                .get_physical_device_surface_support(phys_device, to_u32(i), surface.handle)
                .check_err("get surface support")
        };

        let present_flag = if present_support { " | PRESENT" } else { "" };

        debug!(
            "Queue family #{}: {:?}{}, count = {}",
            i, f.queue_flags, present_flag, f.queue_count
        );

        let img_granularity = f.min_image_transfer_granularity;
        let width = img_granularity.width;
        let height = img_granularity.height;
        let depth = img_granularity.depth;

        if width != 1 && height != 1 && depth != 1 {
            debug!("    special min granularity: {}x{}x{}", width, height, depth);
        }
    }
}

fn gather_phys_device_infos(
    instance: &ash::Instance,
    surface: &Surface,
    phys_devices: &[vk::PhysicalDevice],
) -> Vec<PhysDeviceInfo> {
    let mut phys_device_infos = Vec::with_capacity(phys_devices.len());

    for device_ref in phys_devices {
        let phys_device = *device_ref;
        let properties = unsafe { instance.get_physical_device_properties(phys_device) };
        let data = get_queue_family_data(instance, phys_device, surface);
        let extensions = unsafe { instance.enumerate_device_extension_properties(phys_device) }
            .check_err("enumerate device extensions");

        if supports_required_queues(&data) && supports_required_extensions(&extensions) {
            let queue_family_indices = get_queue_family_indices(&data);

            let info = PhysDeviceInfo {
                phys_device,
                properties,
                queue_family_indices,
            };

            phys_device_infos.push(info);
        }
    }

    phys_device_infos
}

fn supports_required_queues(data: &[QueueFamilyData]) -> bool {
    data.iter().any(|d| d.graphics)
        && data.iter().any(|d| d.present)
        && data.iter().any(|d| d.compute)
}

fn supports_required_extensions(exts: &[vk::ExtensionProperties]) -> bool {
    let req_dev_exts_owned = convert_to_strings(REQ_DEVICE_EXTENSIONS);
    let req_dev_exts_cstrs = convert_to_c_strs(&req_dev_exts_owned);

    let mut support_found = vec![false; req_dev_exts_cstrs.len()];

    for (i, req_ext) in req_dev_exts_cstrs.into_iter().enumerate() {
        for ext in exts {
            let name = unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) };

            if name == req_ext.as_c_str() {
                support_found[i] = true;
            }
        }
    }

    support_found.into_iter().all(|found| found)
}

const fn device_type_to_priority(type_: vk::PhysicalDeviceType) -> i32 {
    match type_ {
        vk::PhysicalDeviceType::DISCRETE_GPU => 1,
        vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
        vk::PhysicalDeviceType::VIRTUAL_GPU => 3,
        vk::PhysicalDeviceType::CPU => 4,
        _ => 5,
    }
}

pub fn choose_swapchain_format(
    phys_device: vk::PhysicalDevice,
    surface: &Surface,
) -> vk::SurfaceFormatKHR {
    let formats =
        unsafe { surface.loader.get_physical_device_surface_formats(phys_device, surface.handle) }
            .check_err("get surface formats");

    print_item_list("Supported surface formats", &formats, |f| {
        format!("format: {:?}, color space: {:?}", f.format, f.color_space)
    });

    for format in &formats {
        if format.format == vk::Format::B8G8R8A8_SRGB
            && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
        {
            return *format;
        }
    }

    formats[0]
}

fn get_surface_capabilities(
    phys_device: vk::PhysicalDevice,
    surface: &Surface,
) -> vk::SurfaceCapabilitiesKHR {
    unsafe { surface.loader.get_physical_device_surface_capabilities(phys_device, surface.handle) }
        .check_err("get surface capabilities")
}

fn choose_swapchain_extent(
    win_width: u32,
    win_height: u32,
    capabilities: &vk::SurfaceCapabilitiesKHR,
) -> vk::Extent2D {
    if capabilities.current_extent.width != u32::MAX {
        return capabilities.current_extent;
    }

    let min = capabilities.min_image_extent;
    let max = capabilities.max_image_extent;

    vk::Extent2D {
        width: win_width.clamp(min.width, max.width),
        height: win_height.clamp(min.height, max.height),
    }
}

fn get_queue_family_data(
    instance: &ash::Instance,
    phys_device: vk::PhysicalDevice,
    surface: &Surface,
) -> Vec<QueueFamilyData> {
    let all_families = unsafe { instance.get_physical_device_queue_family_properties(phys_device) };

    let mut families = Vec::with_capacity(all_families.len());

    for (i, f) in all_families.iter().enumerate() {
        let mut data = QueueFamilyData::default();

        if f.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
            data.graphics = true;
        }

        if f.queue_flags.contains(vk::QueueFlags::COMPUTE) {
            data.compute = true;
        }

        if f.queue_flags.contains(vk::QueueFlags::TRANSFER) {
            data.transfer = true;
        }

        let present_support = unsafe {
            surface
                .loader
                .get_physical_device_surface_support(phys_device, to_u32(i), surface.handle)
                .check_err("get surface support")
        };

        if present_support {
            data.present = true;
        }

        let img_granularity = f.min_image_transfer_granularity;
        let min_granularity = vk::Extent3D {
            width: 1,
            height: 1,
            depth: 1,
        };

        if img_granularity != min_granularity {
            data.special_granularity = Some(img_granularity);
        }

        families.push(data);
    }

    families
}

#[allow(clippy::if_not_else)]
fn get_queue_family_indices(data: &[QueueFamilyData]) -> QueueFamilyIndices {
    // Try to find 4 separate queues for graphics, compute, present and transfer, if possible.
    let (graphics, compute, present, transfer) = bools_to_indices(data);

    // Pick the first queue that supports graphics
    let g = graphics[0];

    // Try to find a queue for compute separate from graphics
    let sep_compute = list_without_element(&compute, g);

    // If there's a separate queue with compute support, use it. Otherwise, use what's available.
    let c = if !sep_compute.is_empty() {
        sep_compute[0]
    } else {
        compute[0]
    };

    // Likewise, try to find queue for present separate from graphics and compute
    let sep_present = list_without_elements(&present, &[g, c]);

    let p = if !sep_present.is_empty() {
        sep_present[0]
    } else {
        // Try a queue separate from compute
        let sep_present = list_without_element(&present, c);

        if !sep_present.is_empty() {
            sep_present[0]
        } else {
            // Try separate from graphics
            let sep_present = list_without_element(&present, g);

            if !sep_present.is_empty() {
                sep_present[0]
            } else {
                present[0]
            }
        }
    };

    // Try other indices for transfer queue
    let sep_transfer = list_without_elements(&transfer, &[g, c, p]);

    let t = if !sep_transfer.is_empty() {
        sep_transfer[0]
    } else {
        transfer[0]
    };

    QueueFamilyIndices {
        graphics: g,
        compute: c,
        present: p,
        transfer: t,
    }
}

fn bools_to_indices(data: &[QueueFamilyData]) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut graphics = vec![];
    let mut compute = vec![];
    let mut present = vec![];
    let mut transfer = vec![];

    for (i, d) in data.iter().enumerate() {
        let i = to_u32(i);

        if d.graphics {
            graphics.push(i);
        }

        if d.compute {
            compute.push(i);
        }

        if d.present {
            present.push(i);
        }

        if d.transfer {
            transfer.push(i);
        }
    }

    assert_not_empty(&graphics, "graphics");
    assert_not_empty(&compute, "compute");
    assert_not_empty(&present, "present");
    assert_not_empty(&transfer, "transfer");

    (graphics, compute, present, transfer)
}

fn assert_not_empty(indices: &[u32], purpose: &str) {
    assert!(!indices.is_empty(), "could not find queue with {} support", purpose);
}

pub fn create_logical_device(instance: &ash::Instance, info: &PhysDeviceInfo) -> ash::Device {
    let mut unique_families = vec![
        info.queue_family_indices.graphics,
        info.queue_family_indices.compute,
        info.queue_family_indices.present,
    ];

    unique_families.sort_unstable();
    unique_families.dedup();

    let mut queue_create_infos = Vec::with_capacity(unique_families.len());
    let queue_priorities = [1.0];

    for queue_family_index in unique_families {
        let queue_create_info = vk::DeviceQueueCreateInfo {
            queue_family_index,
            p_queue_priorities: queue_priorities.as_ptr(),
            queue_count: to_u32(queue_priorities.len()),
            ..Default::default()
        };

        queue_create_infos.push(queue_create_info);
    }

    let features = vk::PhysicalDeviceFeatures {
        // fill_mode_non_solid: 1,
        shader_clip_distance: 1,
        ..Default::default()
    };

    let req_dev_exts_owned = convert_to_strings(REQ_DEVICE_EXTENSIONS);
    let req_dev_exts_cstrs = convert_to_c_strs(&req_dev_exts_owned);
    let req_dev_exts_cptrs = convert_to_c_ptrs(&req_dev_exts_cstrs);

    print_device_extensions(instance, info, &req_dev_exts_cptrs);

    let create_info = vk::DeviceCreateInfo {
        queue_create_info_count: to_u32(queue_create_infos.len()),
        p_queue_create_infos: queue_create_infos.as_ptr(),
        enabled_extension_count: to_u32(req_dev_exts_cptrs.len()),
        pp_enabled_extension_names: req_dev_exts_cptrs.as_ptr(),
        p_enabled_features: &features,
        ..Default::default()
    };

    let _features = unsafe { instance.get_physical_device_features(info.phys_device) };

    unsafe { instance.create_device(info.phys_device, &create_info, None) }
        .check_err("create device")
}

fn print_device_extensions(
    instance: &ash::Instance,
    info: &PhysDeviceInfo,
    req_dev_exts_cptrs: &[*const i8],
) {
    print_textual_items("Required device extensions", req_dev_exts_cptrs, |x| *x);

    if logger::verbose() {
        let extensions = unsafe {
            instance
                .enumerate_device_extension_properties(info.phys_device)
                .check_err("enumerate device extensions")
        };

        print_textual_items("Supported device extensions", &extensions, |ext| {
            ext.extension_name.as_ptr()
        });
    }
}

pub fn get_queues(device: &ash::Device, indices: &QueueFamilyIndices) -> Queues {
    let graphics_queue_idx = indices.graphics;
    let compute_queue_idx = indices.compute;
    let present_queue_idx = indices.present;

    unsafe {
        let graphics = device.get_device_queue(graphics_queue_idx, 0);
        let compute = device.get_device_queue(compute_queue_idx, 0);
        let present = device.get_device_queue(present_queue_idx, 0);

        Queues {
            graphics,
            compute,
            present,
        }
    }
}

fn create_swapchain(
    surface: &Surface,
    present_mode: vk::PresentModeKHR,
    surface_capabilities: &vk::SurfaceCapabilitiesKHR,
    swapchain_format: vk::SurfaceFormatKHR,
    swapchain_extent: vk::Extent2D,
    swapchain_loader: &VkSwapchain,
    queue_family_indices: &QueueFamilyIndices,
) -> vk::SwapchainKHR {
    let mut image_count = surface_capabilities.min_image_count + 1;
    let max_image_count = surface_capabilities.max_image_count;

    if image_count > max_image_count && max_image_count != 0 {
        image_count = max_image_count;
    }

    let gfx_queue_idx = queue_family_indices.graphics;
    let present_queue_idx = queue_family_indices.present;

    let (image_sharing_mode, queue_family_index_count, queue_family_indices) =
        if gfx_queue_idx == present_queue_idx {
            (vk::SharingMode::EXCLUSIVE, 0, vec![])
        } else {
            (vk::SharingMode::CONCURRENT, 2, vec![gfx_queue_idx, present_queue_idx])
        };

    let create_info = vk::SwapchainCreateInfoKHR {
        surface: surface.handle,
        min_image_count: image_count,
        image_color_space: swapchain_format.color_space,
        image_format: swapchain_format.format,
        image_extent: swapchain_extent,
        image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
        image_sharing_mode,
        p_queue_family_indices: queue_family_indices.as_ptr(),
        queue_family_index_count,
        pre_transform: surface_capabilities.current_transform,
        composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
        present_mode,
        clipped: vk::TRUE,
        image_array_layers: 1,
        ..Default::default()
    };

    unsafe { swapchain_loader.create_swapchain(&create_info, None) }.check_err("create swapchain")
}

fn choose_swapchain_present_mode(
    phys_device: vk::PhysicalDevice,
    surface: &Surface,
) -> vk::PresentModeKHR {
    let mut modes = unsafe {
        surface.loader.get_physical_device_surface_present_modes(phys_device, surface.handle)
    }
    .check_err("get present modes");

    print_item_list("Supported present modes", &modes, |m| format!("{:?}", m));

    modes.sort_by_key(|m| present_mode_to_priority(*m));

    let mode = modes[0];

    debug!("Using present mode: {:?}", mode);

    mode
}

fn present_mode_to_priority(mode: vk::PresentModeKHR) -> u32 {
    match mode {
        vk::PresentModeKHR::IMMEDIATE => 1,
        vk::PresentModeKHR::FIFO_RELAXED => 2,
        vk::PresentModeKHR::MAILBOX => 3,
        vk::PresentModeKHR::FIFO => 4,
        x => {
            warn!("Unexpected present mode: {}", x.as_raw());
            5
        }
    }
}

pub fn create_command_pool(
    device: &ash::Device,
    queue_family_index: u32,
    reset: bool,
) -> vk::CommandPool {
    let flags = if reset {
        vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER
    } else {
        vk::CommandPoolCreateFlags::empty()
    };

    let create_info = vk::CommandPoolCreateInfo {
        flags,
        queue_family_index,
        ..Default::default()
    };

    unsafe { device.create_command_pool(&create_info, None) }.check_err("create command pool")
}

pub fn alloc_command_buffers(
    device: &ash::Device,
    command_pool: vk::CommandPool,
    count: usize,
) -> Vec<vk::CommandBuffer> {
    let allocate_info = vk::CommandBufferAllocateInfo {
        command_pool,
        level: vk::CommandBufferLevel::PRIMARY,
        command_buffer_count: to_u32(count),
        ..Default::default()
    };

    unsafe { device.allocate_command_buffers(&allocate_info) }.check_err("allocate command buffers")
}

pub fn alloc_command_buffer(
    device: &ash::Device,
    command_pool: vk::CommandPool,
) -> vk::CommandBuffer {
    alloc_command_buffers(device, command_pool, 1)[0]
}

fn get_swapchain_images(
    swapchain_loader: &VkSwapchain,
    swapchain: vk::SwapchainKHR,
) -> Vec<vk::Image> {
    unsafe { swapchain_loader.get_swapchain_images(swapchain) }.check_err("get swapchain images")
}

fn create_swapchain_image_views(
    device: &ash::Device,
    format: vk::Format,
    images: &[vk::Image],
) -> Vec<vk::ImageView> {
    images
        .iter()
        .map(|&image| create_image_view(device, image, format, vk::ImageAspectFlags::COLOR, 1))
        .collect()
}

fn create_image_view(
    device: &ash::Device,
    image: vk::Image,
    format: vk::Format,
    aspect_mask: vk::ImageAspectFlags,
    mip_levels: u32,
) -> vk::ImageView {
    let components = vk::ComponentMapping::default();

    let subresource_range = vk::ImageSubresourceRange {
        aspect_mask,
        base_mip_level: 0,
        level_count: mip_levels,
        base_array_layer: 0,
        layer_count: 1,
    };

    let create_info = vk::ImageViewCreateInfo {
        image,
        view_type: vk::ImageViewType::TYPE_2D,
        format,
        components,
        subresource_range,
        ..Default::default()
    };

    unsafe { device.create_image_view(&create_info, None) }.check_err("create image view")
}

fn create_texture_sampler(device: &ash::Device) -> vk::Sampler {
    let create_info = vk::SamplerCreateInfo {
        mag_filter: vk::Filter::NEAREST,
        min_filter: vk::Filter::NEAREST,
        mipmap_mode: vk::SamplerMipmapMode::LINEAR,
        mip_lod_bias: 0.0,
        min_lod: 0.0,
        max_lod: 0.0,
        address_mode_u: vk::SamplerAddressMode::CLAMP_TO_BORDER,
        address_mode_v: vk::SamplerAddressMode::CLAMP_TO_BORDER,
        address_mode_w: vk::SamplerAddressMode::CLAMP_TO_BORDER,
        anisotropy_enable: 0,
        max_anisotropy: 0.0,
        compare_enable: 0,
        compare_op: vk::CompareOp::ALWAYS,
        border_color: vk::BorderColor::INT_TRANSPARENT_BLACK,
        unnormalized_coordinates: 0,
        ..Default::default()
    };

    unsafe { device.create_sampler(&create_info, None).check_err("create sampler") }
}

pub fn create_framebuffers(
    device: &ash::Device,
    image_views: &[vk::ImageView],
    depth_textures: &[FramebufferAttachment],
    fb_attachments: &[FramebufferAttachment],
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
) -> Vec<vk::Framebuffer> {
    if !fb_attachments.is_empty() {
        assert!(
            image_views.len() == fb_attachments.len(),
            "image view and attachment length mismatch"
        );
    }

    let num_swapchain_images = image_views.len();
    let mut framebuffers = Vec::with_capacity(num_swapchain_images);

    for i in 0..num_swapchain_images {
        let attachments = if fb_attachments.is_empty() {
            vec![image_views[i], depth_textures[i].image_view]
        } else {
            vec![
                image_views[i],
                fb_attachments[i].image_view,
                depth_textures[i].image_view,
            ]
        };

        let create_info = vk::FramebufferCreateInfo {
            render_pass,
            attachment_count: to_u32(attachments.len()),
            p_attachments: attachments.as_ptr(),
            width: extent.width,
            height: extent.height,
            layers: 1,
            ..Default::default()
        };

        let framebuffer = unsafe { device.create_framebuffer(&create_info, None) }
            .check_err("create framebuffer");

        framebuffers.push(framebuffer);
    }

    framebuffers
}

pub fn create_buffer_of_type<T: Copy>(
    device: &ash::Device,
    device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    usage: vk::BufferUsageFlags,
    data: &[T],
) -> (vk::Buffer, vk::DeviceMemory) {
    let size = size_of_val(data) as u64;

    let (staging_buffer, staging_memory) = unsafe {
        create_buffer(
            device,
            device_mem_properties,
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE,
        )
    };

    upload_to_buffer_memory(device, staging_memory, data);

    let (buffer, memory) = unsafe {
        create_buffer(
            device,
            device_mem_properties,
            size,
            usage | vk::BufferUsageFlags::TRANSFER_DST,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
    };

    copy_buffers(device, command_pool, queue, staging_buffer, buffer, size);

    unsafe {
        device.destroy_buffer(staging_buffer, None);
        device.free_memory(staging_memory, None);
    }

    (buffer, memory)
}

pub unsafe fn create_buffer(
    device: &ash::Device,
    device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
    size: u64,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> (vk::Buffer, vk::DeviceMemory) {
    let create_info = vk::BufferCreateInfo {
        size,
        usage,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        ..Default::default()
    };

    let buffer = device.create_buffer(&create_info, None).check_err("create buffer");

    let mem_requirements = device.get_buffer_memory_requirements(buffer);

    let memory_type_index =
        find_memory_type(mem_requirements.memory_type_bits, properties, device_mem_properties)
            .check_err("find appropriate memory type");

    let alloc_info = vk::MemoryAllocateInfo {
        allocation_size: mem_requirements.size,
        memory_type_index,
        ..Default::default()
    };

    let memory = device.allocate_memory(&alloc_info, None).check_err("allocate buffer memory");

    device.bind_buffer_memory(buffer, memory, 0).check_err("bind buffer");

    (buffer, memory)
}

fn find_memory_type(
    req_type: u32,
    req_properties: vk::MemoryPropertyFlags,
    mem_properties: &vk::PhysicalDeviceMemoryProperties,
) -> Option<u32> {
    for (i, memory_type) in mem_properties.memory_types.iter().enumerate() {
        if req_type & (1 << i) == 0 {
            continue;
        }

        if !memory_type.property_flags.contains(req_properties) {
            continue;
        }

        return Some(to_u32(i));
    }

    None
}

fn upload_to_buffer_memory<T: Copy>(device: &ash::Device, memory: vk::DeviceMemory, data: &[T]) {
    let size = size_of_val(data) as u64;

    let memory_range = vk::MappedMemoryRange {
        memory,
        offset: 0,
        size,
        ..Default::default()
    };

    unsafe {
        let out_ptr = device
            .map_memory(memory, 0, size, vk::MemoryMapFlags::empty())
            .check_err("map memory")
            .cast::<T>();

        out_ptr.copy_from_nonoverlapping(data.as_ptr(), data.len());

        device.flush_mapped_memory_ranges(&[memory_range]).check_err("flush mapped memory");

        device.unmap_memory(memory);
    }
}

fn copy_buffers(
    device: &ash::Device,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    src: vk::Buffer,
    dst: vk::Buffer,
    size: u64,
) {
    let copy_region = vk::BufferCopy {
        size,
        ..Default::default()
    };

    unsafe {
        let cmd_buffer = start_single_command(device, command_pool);

        device.cmd_copy_buffer(cmd_buffer, src, dst, &[copy_region]);

        end_single_command(device, command_pool, cmd_buffer, queue);
    }
}

unsafe fn start_single_command(
    device: &ash::Device,
    command_pool: vk::CommandPool,
) -> vk::CommandBuffer {
    let cmd_buffer = alloc_command_buffer(device, command_pool);
    let begin_info = ONE_TIME_SUBMIT;

    device
        .begin_command_buffer(cmd_buffer, &begin_info)
        .check_err("begin single-use command buffer");

    cmd_buffer
}

unsafe fn end_single_command(
    device: &ash::Device,
    command_pool: vk::CommandPool,
    cmd_buffer: vk::CommandBuffer,
    queue: vk::Queue,
) {
    let submit_info = vk::SubmitInfo {
        command_buffer_count: 1,
        p_command_buffers: &cmd_buffer,
        ..Default::default()
    };

    device.end_command_buffer(cmd_buffer).check_err("end single command buffer");
    device.queue_submit(queue, &[submit_info], vk::Fence::null()).check_err("submit to queue");
    device.queue_wait_idle(queue).check_err("wait for queue");
    device.free_command_buffers(command_pool, &[cmd_buffer]);
}

fn create_texture_image(
    device: &ash::Device,
    device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
    command_pool: vk::CommandPool,
    graphics_queue: vk::Queue,
    path: &'static str,
) -> (vk::Image, vk::DeviceMemory, vk::ImageLayout) {
    let texture = Image::from_file(path).check_err("decode image");
    let format = vk::Format::R8G8B8A8_SRGB;
    let texture_size = texture.size_x * texture.size_y * 4;
    let final_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

    let (staging_buffer, staging_memory) = unsafe {
        create_buffer(
            device,
            device_mem_properties,
            texture_size.into(),
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE,
        )
    };

    upload_to_buffer_memory(device, staging_memory, &texture.data);

    let (image, image_memory) = create_image(
        device,
        device_mem_properties,
        format,
        texture.size_x,
        texture.size_y,
        vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
    );

    transition_image_layout(
        device,
        command_pool,
        graphics_queue,
        image,
        vk::ImageLayout::UNDEFINED,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    );

    copy_buffer_to_image(
        device,
        command_pool,
        graphics_queue,
        staging_buffer,
        image,
        texture.size_x,
        texture.size_y,
    );

    transition_image_layout(
        device,
        command_pool,
        graphics_queue,
        image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        final_layout,
    );

    unsafe {
        device.destroy_buffer(staging_buffer, None);
        device.free_memory(staging_memory, None);
    }

    (image, image_memory, final_layout)
}

fn create_image(
    device: &ash::Device,
    device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
    format: vk::Format,
    width: u32,
    height: u32,
    usage: vk::ImageUsageFlags,
) -> (vk::Image, vk::DeviceMemory) {
    let create_info = vk::ImageCreateInfo {
        image_type: vk::ImageType::TYPE_2D,
        format,
        extent: vk::Extent3D {
            width,
            height,
            depth: 1,
        },
        mip_levels: 1,
        array_layers: 1,
        samples: vk::SampleCountFlags::TYPE_1,
        tiling: vk::ImageTiling::OPTIMAL,
        usage,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        initial_layout: vk::ImageLayout::UNDEFINED,
        ..Default::default()
    };

    unsafe {
        let image = device.create_image(&create_info, None).check_err("create image");

        let req = device.get_image_memory_requirements(image);

        let memory_type_index = find_memory_type(
            req.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            device_mem_properties,
        )
        .check_err("find appropriate memory type");

        let alloc_info = vk::MemoryAllocateInfo {
            allocation_size: req.size,
            memory_type_index,
            ..Default::default()
        };

        let image_memory = device.allocate_memory(&alloc_info, None).check_err("allocate memory");

        device.bind_image_memory(image, image_memory, 0).check_err("bind image memory");

        (image, image_memory)
    }
}

fn transition_image_layout(
    device: &ash::Device,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    unsafe {
        let cmd_buffer = start_single_command(device, command_pool);

        record_image_layout_transition(
            device,
            cmd_buffer,
            vk::QUEUE_FAMILY_IGNORED,
            vk::QUEUE_FAMILY_IGNORED,
            image,
            old_layout,
            new_layout,
        );

        end_single_command(device, command_pool, cmd_buffer, queue);
    }
}

pub fn record_image_layout_transition(
    device: &ash::Device,
    cmd_buffer: vk::CommandBuffer,
    src_queue_family_index: u32,
    dst_queue_family_index: u32,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    let subresource_range = BASE_SUBRESOURCE_RANGE;

    let (src_access_mask, dst_access_mask, src_stage, dst_stage) =
        image_layout_transition_flags(old_layout, new_layout);

    let barrier = vk::ImageMemoryBarrier {
        src_access_mask,
        dst_access_mask,
        old_layout,
        new_layout,
        src_queue_family_index,
        dst_queue_family_index,
        image,
        subresource_range,
        ..Default::default()
    };

    unsafe {
        device.cmd_pipeline_barrier(
            cmd_buffer,
            src_stage,
            dst_stage,
            vk::DependencyFlags::BY_REGION,
            &[],
            &[],
            &[barrier],
        );
    }
}

fn image_layout_transition_flags(
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) -> (vk::AccessFlags, vk::AccessFlags, vk::PipelineStageFlags, vk::PipelineStageFlags) {
    match (old_layout, new_layout) {
        (vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL) => (
            vk::AccessFlags::empty(),
            vk::AccessFlags::TRANSFER_WRITE,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
        ),
        (vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
            vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::SHADER_READ,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
        ),
        (vk::ImageLayout::UNDEFINED, vk::ImageLayout::GENERAL) => (
            vk::AccessFlags::empty(),
            vk::AccessFlags::SHADER_READ,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::COMPUTE_SHADER,
        ),
        _ => panic!("unexpected layout transition: {:?} -> {:?}", old_layout, new_layout),
    }
}

fn copy_buffer_to_image(
    device: &ash::Device,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
    buffer: vk::Buffer,
    image: vk::Image,
    width: u32,
    height: u32,
) {
    let image_subresource = vk::ImageSubresourceLayers {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        mip_level: 0,
        base_array_layer: 0,
        layer_count: 1,
    };

    let image_offset = vk::Offset3D::default();

    let image_extent = vk::Extent3D {
        width,
        height,
        depth: 1,
    };

    let region = vk::BufferImageCopy {
        buffer_offset: 0,
        buffer_row_length: 0,
        buffer_image_height: 0,
        image_subresource,
        image_offset,
        image_extent,
    };

    unsafe {
        let cmd_buffer = start_single_command(device, command_pool);

        device.cmd_copy_buffer_to_image(
            cmd_buffer,
            buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[region],
        );

        end_single_command(device, command_pool, cmd_buffer, queue);
    }
}

pub fn create_host_visible_shader_buffers<T>(
    device: &ash::Device,
    device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
    usage: vk::BufferUsageFlags,
    size: u64,
    copies: usize,
) -> (Vec<vk::Buffer>, Vec<vk::DeviceMemory>, Vec<*mut T>) {
    let mut buffers = Vec::with_capacity(copies);
    let mut memories = Vec::with_capacity(copies);
    let mut mappings = Vec::with_capacity(copies);

    for _ in 0..copies {
        unsafe {
            let (buffer, memory) = create_buffer(
                device,
                device_mem_properties,
                size,
                usage,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            );
            let mapping = device
                .map_memory(memory, 0, size, vk::MemoryMapFlags::empty())
                .check_err("map memory")
                .cast::<T>();

            buffers.push(buffer);
            memories.push(memory);
            mappings.push(mapping);
        }
    }

    (buffers, memories, mappings)
}

pub fn create_uniform_buffers<T>(
    device: &ash::Device,
    device_mem_properties: &vk::PhysicalDeviceMemoryProperties,
    copies: usize,
) -> (Vec<vk::Buffer>, Vec<vk::DeviceMemory>, Vec<*mut T>) {
    let usage = vk::BufferUsageFlags::UNIFORM_BUFFER;
    let size = size_of::<T>() as u64;

    create_host_visible_shader_buffers(device, device_mem_properties, usage, size, copies)
}

pub fn find_depth_format(instance: &ash::Instance, phys_device: vk::PhysicalDevice) -> vk::Format {
    let candidates = [
        vk::Format::D24_UNORM_S8_UINT,
        vk::Format::D16_UNORM,
        vk::Format::D16_UNORM_S8_UINT,
        vk::Format::D32_SFLOAT,
        vk::Format::D32_SFLOAT_S8_UINT,
    ];
    let optimal_tiling = true;
    let features = vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT;

    let format =
        find_supported_format(instance, phys_device, &candidates, optimal_tiling, features)
            .check_err("find supported format");

    debug!("Using depth format: {:?}", format);

    format
}

#[allow(clippy::collapsible_else_if)]
fn find_supported_format(
    instance: &ash::Instance,
    phys_device: vk::PhysicalDevice,
    candidates: &[vk::Format],
    optimal_tiling: bool,
    features: vk::FormatFeatureFlags,
) -> Option<vk::Format> {
    for candidate in candidates {
        let props =
            unsafe { instance.get_physical_device_format_properties(phys_device, *candidate) };

        if optimal_tiling {
            if props.optimal_tiling_features.contains(features) {
                return Some(*candidate);
            }
        } else {
            if props.linear_tiling_features.contains(features) {
                return Some(*candidate);
            }
        }
    }

    None
}

pub fn create_semaphores(device: &ash::Device, copies: usize) -> Vec<vk::Semaphore> {
    let mut semaphores = Vec::with_capacity(copies);

    for _ in 0..copies {
        semaphores.push(create_semaphore(device));
    }

    semaphores
}

fn create_semaphore(device: &ash::Device) -> vk::Semaphore {
    let create_info = vk::SemaphoreCreateInfo::default();

    unsafe { device.create_semaphore(&create_info, None) }.check_err("create semaphore")
}

pub fn create_fences(device: &ash::Device, signaled: bool, copies: usize) -> Vec<vk::Fence> {
    let mut fences = Vec::with_capacity(copies);

    for _ in 0..copies {
        fences.push(create_fence(device, signaled));
    }

    fences
}

pub fn create_fence(device: &ash::Device, signaled: bool) -> vk::Fence {
    let flags = if signaled {
        vk::FenceCreateFlags::SIGNALED
    } else {
        vk::FenceCreateFlags::empty()
    };

    let create_info = vk::FenceCreateInfo {
        flags,
        ..Default::default()
    };

    unsafe { device.create_fence(&create_info, None) }.check_err("create fence")
}
