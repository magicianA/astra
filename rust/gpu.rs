//! Vulkan resources shared by the renderer and spectral transport generator.
use anyhow::{Context, Result, bail};
use ash::{Entry, vk};
use std::{
    ffi::CStr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

pub struct Gpu {
    _entry: Entry,
    pub instance: ash::Instance,
    pub device: ash::Device,
    pub physical: vk::PhysicalDevice,
    pub queue: vk::Queue,
    pub family: u32,
    pub pool: vk::CommandPool,
    pub name: String,
    pub manual_filter: bool,
    pub anisotropy: f32,
    debug: Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
    errors: Arc<AtomicUsize>,
    submission: Mutex<()>,
}
// These handles are created before Gpu can own them. Roll them back on every
// fallible bootstrap step while the Vulkan loader is still alive.
struct GpuConstruction {
    instance: Option<ash::Instance>,
    device: Option<ash::Device>,
    pool: vk::CommandPool,
    debug: Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
}
impl GpuConstruction {
    fn new(instance: ash::Instance) -> Self {
        Self {
            instance: Some(instance),
            device: None,
            pool: vk::CommandPool::null(),
            debug: None,
        }
    }
    fn disarm(&mut self) {
        self.instance = None;
        self.device = None;
        self.pool = vk::CommandPool::null();
        self.debug = None;
    }
}
impl Drop for GpuConstruction {
    fn drop(&mut self) {
        unsafe {
            if let Some(device) = &self.device {
                if self.pool != vk::CommandPool::null() {
                    device.destroy_command_pool(self.pool, None);
                }
                device.destroy_device(None);
            }
            if let Some((loader, messenger)) = &self.debug {
                loader.destroy_debug_utils_messenger(*messenger, None);
            }
            if let Some(instance) = &self.instance {
                instance.destroy_instance(None);
            }
        }
    }
}

/// Device objects whose constructors need rollback before their final owner exists.
#[derive(Clone, Copy)]
pub enum OwnHandle {
    DescriptorPool(vk::DescriptorPool),
    DescriptorSetLayout(vk::DescriptorSetLayout),
    Pipeline(vk::Pipeline),
    PipelineLayout(vk::PipelineLayout),
    RenderPass(vk::RenderPass),
    Framebuffer(vk::Framebuffer),
    ShaderModule(vk::ShaderModule),
}
macro_rules! own_handle {
    ($($name:ident),* $(,)?) => {$(
        impl From<vk::$name> for OwnHandle {
            fn from(handle: vk::$name) -> Self { Self::$name(handle) }
        }
    )*};
}
own_handle!(
    DescriptorPool,
    DescriptorSetLayout,
    Pipeline,
    PipelineLayout,
    RenderPass,
    Framebuffer,
    ShaderModule
);

/// Registers raw Vulkan objects immediately, then releases them in reverse order.
/// Call disarm only after transferring every registered object to its final owner.
pub struct HandleArena {
    gpu: Arc<Gpu>,
    handles: Vec<OwnHandle>,
}
impl HandleArena {
    pub fn new(gpu: &Arc<Gpu>) -> Self {
        Self {
            gpu: gpu.clone(),
            handles: Vec::new(),
        }
    }
    pub fn own<T: Into<OwnHandle> + Copy>(&mut self, handle: T) -> T {
        self.handles.push(handle.into());
        handle
    }
    pub fn disarm(&mut self) {
        self.handles.clear();
    }
}
impl Drop for HandleArena {
    fn drop(&mut self) {
        unsafe {
            for handle in self.handles.drain(..).rev() {
                match handle {
                    OwnHandle::DescriptorPool(h) => {
                        self.gpu.device.destroy_descriptor_pool(h, None)
                    }
                    OwnHandle::DescriptorSetLayout(h) => {
                        self.gpu.device.destroy_descriptor_set_layout(h, None)
                    }
                    OwnHandle::Pipeline(h) => self.gpu.device.destroy_pipeline(h, None),
                    OwnHandle::PipelineLayout(h) => {
                        self.gpu.device.destroy_pipeline_layout(h, None)
                    }
                    OwnHandle::RenderPass(h) => self.gpu.device.destroy_render_pass(h, None),
                    OwnHandle::Framebuffer(h) => self.gpu.device.destroy_framebuffer(h, None),
                    OwnHandle::ShaderModule(h) => self.gpu.device.destroy_shader_module(h, None),
                }
            }
        }
    }
}

unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    user: *mut std::ffi::c_void,
) -> vk::Bool32 {
    unsafe {
        if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
            (*(user as *const AtomicUsize)).fetch_add(1, Ordering::Relaxed);
        }
        if !data.is_null() {
            eprintln!(
                "[Vulkan] {}",
                CStr::from_ptr((*data).p_message).to_string_lossy()
            );
        }
    }
    vk::FALSE
}
impl Gpu {
    pub fn new(validation: bool) -> Result<Arc<Self>> {
        unsafe {
            let bundled = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("../Frameworks/libvulkan.dylib")));
            let entry = Entry::load()
                .or_else(|e| bundled.as_ref().map_or(Err(e), |p| Entry::load_from(p)))
                .or_else(|_| Entry::load_from("/opt/homebrew/lib/libvulkan.dylib"))
                .context("Vulkan loader unavailable")?;
            let available = entry.enumerate_instance_extension_properties(None)?;
            let has = |name: &CStr| {
                available
                    .iter()
                    .any(|e| CStr::from_ptr(e.extension_name.as_ptr()) == name)
            };
            let mut extensions = Vec::new();
            let mut flags = vk::InstanceCreateFlags::empty();
            if has(ash::khr::portability_enumeration::NAME) {
                extensions.push(ash::khr::portability_enumeration::NAME.as_ptr());
                flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
            }
            let errors = Arc::new(AtomicUsize::new(0));
            let mut dbg = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(debug_callback))
                .user_data(Arc::as_ptr(&errors) as *mut _);
            let layers =
                if validation {
                    let layers = entry.enumerate_instance_layer_properties()?;
                    anyhow::ensure!(
                        layers.iter().any(|l| CStr::from_ptr(l.layer_name.as_ptr())
                            == c"VK_LAYER_KHRONOS_validation"),
                        "Validation layer unavailable; check VK_LAYER_PATH"
                    );
                    extensions.push(ash::ext::debug_utils::NAME.as_ptr());
                    vec![c"VK_LAYER_KHRONOS_validation".as_ptr()]
                } else {
                    vec![]
                };
            let app = vk::ApplicationInfo::default()
                .application_name(c"Astra Rust")
                .api_version(vk::API_VERSION_1_2);
            let mut info = vk::InstanceCreateInfo::default()
                .application_info(&app)
                .enabled_extension_names(&extensions)
                .enabled_layer_names(&layers)
                .flags(flags);
            if validation {
                info = info.push_next(&mut dbg);
            }
            let instance = entry.create_instance(&info, None)?;
            let mut cleanup = GpuConstruction::new(instance.clone());
            let debug = if validation {
                let loader = ash::ext::debug_utils::Instance::new(&entry, &instance);
                let messenger = loader.create_debug_utils_messenger(&dbg, None)?;
                Some((loader, messenger))
            } else {
                None
            };
            cleanup.debug = debug.clone();
            let found = instance
                .enumerate_physical_devices()?
                .into_iter()
                .find_map(|p| {
                    let props = instance.get_physical_device_properties(p);
                    if props.api_version < vk::API_VERSION_1_2 {
                        return None;
                    }
                    instance
                        .get_physical_device_queue_family_properties(p)
                        .iter()
                        .position(|f| {
                            f.queue_flags
                                .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
                        })
                        .map(|f| (p, f as u32))
                });
            let Some((physical, family)) = found else {
                bail!("Vulkan 1.2 graphics/compute device required");
            };
            let available = instance.enumerate_device_extension_properties(physical)?;
            let extensions: Vec<_> = available
                .iter()
                .filter(|e| {
                    CStr::from_ptr(e.extension_name.as_ptr()) == ash::khr::portability_subset::NAME
                })
                .map(|_| ash::khr::portability_subset::NAME.as_ptr())
                .collect();
            let supported = instance.get_physical_device_features(physical);
            let features = vk::PhysicalDeviceFeatures::default()
                .sampler_anisotropy(supported.sampler_anisotropy != 0);
            let priorities = [1.0];
            let queues = [vk::DeviceQueueCreateInfo::default()
                .queue_family_index(family)
                .queue_priorities(&priorities)];
            let device = instance.create_device(
                physical,
                &vk::DeviceCreateInfo::default()
                    .queue_create_infos(&queues)
                    .enabled_extension_names(&extensions)
                    .enabled_features(&features),
                None,
            )?;
            cleanup.device = Some(device.clone());
            let queue = device.get_device_queue(family, 0);
            let pool = device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(family)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?;
            cleanup.pool = pool;
            let props = instance.get_physical_device_properties(physical);
            let format = instance
                .get_physical_device_format_properties(physical, vk::Format::R32G32B32A32_SFLOAT);
            let manual_filter = !format
                .optimal_tiling_features
                .contains(vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR)
                || std::env::var_os("ASTRA_ATMOSPHERE_MANUAL_FILTER").is_some();
            let anisotropy = if supported.sampler_anisotropy != 0 {
                props.limits.max_sampler_anisotropy.min(8.)
            } else {
                1.
            };
            cleanup.disarm();
            Ok(Arc::new(Self {
                _entry: entry,
                instance,
                device,
                physical,
                queue,
                family,
                pool,
                name: CStr::from_ptr(props.device_name.as_ptr())
                    .to_string_lossy()
                    .into_owned(),
                manual_filter,
                anisotropy,
                debug,
                errors,
                submission: Mutex::new(()),
            }))
        }
    }
    pub fn errors(&self) -> usize {
        self.errors.load(Ordering::Relaxed)
    }
    pub fn memory_type(&self, bits: u32, flags: vk::MemoryPropertyFlags) -> Result<u32> {
        unsafe {
            let p = self
                .instance
                .get_physical_device_memory_properties(self.physical);
            (0..p.memory_type_count)
                .find(|&i| {
                    bits & (1 << i) != 0
                        && p.memory_types[i as usize].property_flags.contains(flags)
                })
                .context("No compatible Vulkan memory")
        }
    }
    pub fn submit(&self, record: impl FnOnce(vk::CommandBuffer) -> Result<()>) -> Result<()> {
        // Vulkan externally synchronizes both the queue and its command pool.
        let _submission = self.submission.lock().unwrap_or_else(|p| p.into_inner());
        unsafe {
            let cmd = self.device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(self.pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )?[0];
            let result = (|| {
                self.device.begin_command_buffer(
                    cmd,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )?;
                record(cmd)?;
                self.device.end_command_buffer(cmd)?;
                let commands = [cmd];
                self.device.queue_submit(
                    self.queue,
                    &[vk::SubmitInfo::default().command_buffers(&commands)],
                    vk::Fence::null(),
                )?;
                self.device.queue_wait_idle(self.queue)?;
                Ok(())
            })();
            self.device.free_command_buffers(self.pool, &[cmd]);
            result
        }
    }
    pub fn wait_idle(&self) -> Result<()> {
        let _submission = self.submission.lock().unwrap_or_else(|p| p.into_inner());
        unsafe {
            self.device.device_wait_idle()?;
        }
        Ok(())
    }
    pub fn shader(&self, bytes: &[u8]) -> Result<vk::ShaderModule> {
        let code = ash::util::read_spv(&mut std::io::Cursor::new(bytes))?;
        unsafe {
            Ok(self
                .device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code), None)?)
        }
    }
}
impl Drop for Gpu {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_device(None);
            if let Some((loader, m)) = &self.debug {
                loader.destroy_debug_utils_messenger(*m, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}

pub struct Buffer {
    gpu: Arc<Gpu>,
    pub handle: vk::Buffer,
    memory: vk::DeviceMemory,
    pub size: u64,
    pub mapped: *mut u8,
}
impl Buffer {
    pub fn new(gpu: &Arc<Gpu>, size: u64, usage: vk::BufferUsageFlags) -> Result<Self> {
        unsafe {
            let mut result = Self {
                gpu: gpu.clone(),
                handle: vk::Buffer::null(),
                memory: vk::DeviceMemory::null(),
                size: size.max(4),
                mapped: std::ptr::null_mut(),
            };
            result.handle = gpu.device.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(result.size)
                    .usage(usage)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )?;
            let req = gpu.device.get_buffer_memory_requirements(result.handle);
            result.memory = gpu.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(req.size)
                    .memory_type_index(gpu.memory_type(
                        req.memory_type_bits,
                        vk::MemoryPropertyFlags::HOST_VISIBLE
                            | vk::MemoryPropertyFlags::HOST_COHERENT,
                    )?),
                None,
            )?;
            gpu.device
                .bind_buffer_memory(result.handle, result.memory, 0)?;
            result.mapped = gpu.device.map_memory(
                result.memory,
                0,
                vk::WHOLE_SIZE,
                vk::MemoryMapFlags::empty(),
            )? as *mut u8;
            Ok(result)
        }
    }
    pub fn write(&self, bytes: &[u8]) {
        assert!(bytes.len() as u64 <= self.size);
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.mapped, bytes.len());
        }
    }
    pub fn read(&self) -> Vec<u8> {
        unsafe { std::slice::from_raw_parts(self.mapped, self.size as usize).to_vec() }
    }
}
impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            if !self.mapped.is_null() {
                self.gpu.device.unmap_memory(self.memory);
            }
            if self.handle != vk::Buffer::null() {
                self.gpu.device.destroy_buffer(self.handle, None);
            }
            if self.memory != vk::DeviceMemory::null() {
                self.gpu.device.free_memory(self.memory, None);
            }
        }
    }
}

pub struct Texture {
    gpu: Arc<Gpu>,
    pub image: vk::Image,
    memory: vk::DeviceMemory,
    pub view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub extent: vk::Extent3D,
    pub levels: u32,
}
impl Texture {
    pub fn new(
        gpu: &Arc<Gpu>,
        extent: vk::Extent3D,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
        levels: u32,
        linear: bool,
        repeat: bool,
        anisotropy: f32,
    ) -> Result<Self> {
        anyhow::ensure!(
            levels > 0 && extent.width > 0 && extent.height > 0 && extent.depth > 0,
            "Invalid Vulkan image extent/mip count"
        );
        unsafe {
            let mut result = Self {
                gpu: gpu.clone(),
                image: vk::Image::null(),
                memory: vk::DeviceMemory::null(),
                view: vk::ImageView::null(),
                sampler: vk::Sampler::null(),
                extent,
                levels,
            };
            result.image = gpu.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(if extent.depth > 1 {
                        vk::ImageType::TYPE_3D
                    } else {
                        vk::ImageType::TYPE_2D
                    })
                    .format(format)
                    .extent(extent)
                    .mip_levels(levels)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(usage)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED),
                None,
            )?;
            let req = gpu.device.get_image_memory_requirements(result.image);
            result.memory = gpu.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(req.size)
                    .memory_type_index(gpu.memory_type(
                        req.memory_type_bits,
                        vk::MemoryPropertyFlags::DEVICE_LOCAL,
                    )?),
                None,
            )?;
            gpu.device
                .bind_image_memory(result.image, result.memory, 0)?;
            let range = vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .level_count(levels)
                .layer_count(1);
            result.view = gpu.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(result.image)
                    .view_type(if extent.depth > 1 {
                        vk::ImageViewType::TYPE_3D
                    } else {
                        vk::ImageViewType::TYPE_2D
                    })
                    .format(format)
                    .subresource_range(range),
                None,
            )?;
            let filter = if linear {
                vk::Filter::LINEAR
            } else {
                vk::Filter::NEAREST
            };
            result.sampler = gpu.device.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(filter)
                    .min_filter(filter)
                    .mipmap_mode(if linear {
                        vk::SamplerMipmapMode::LINEAR
                    } else {
                        vk::SamplerMipmapMode::NEAREST
                    })
                    .address_mode_u(if repeat {
                        vk::SamplerAddressMode::REPEAT
                    } else {
                        vk::SamplerAddressMode::CLAMP_TO_EDGE
                    })
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .max_lod((levels - 1) as f32)
                    .anisotropy_enable(anisotropy > 1.)
                    .max_anisotropy(anisotropy),
                None,
            )?;
            Ok(result)
        }
    }
    pub fn descriptor(&self, layout: vk::ImageLayout) -> vk::DescriptorImageInfo {
        vk::DescriptorImageInfo::default()
            .image_view(self.view)
            .sampler(self.sampler)
            .image_layout(layout)
    }
    pub fn barrier(
        &self,
        cmd: vk::CommandBuffer,
        old: vk::ImageLayout,
        new: vk::ImageLayout,
        src: vk::AccessFlags,
        dst: vk::AccessFlags,
        from: vk::PipelineStageFlags,
        to: vk::PipelineStageFlags,
    ) {
        unsafe {
            let barriers = [vk::ImageMemoryBarrier::default()
                .image(self.image)
                .old_layout(old)
                .new_layout(new)
                .src_access_mask(src)
                .dst_access_mask(dst)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(self.levels)
                        .layer_count(1),
                )];
            self.gpu.device.cmd_pipeline_barrier(
                cmd,
                from,
                to,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &barriers,
            );
        }
    }
    pub fn upload(
        &self,
        bytes: &[u8],
        regions: &[vk::BufferImageCopy],
        layout: vk::ImageLayout,
    ) -> Result<()> {
        let staging = Buffer::new(
            &self.gpu,
            bytes.len() as u64,
            vk::BufferUsageFlags::TRANSFER_SRC,
        )?;
        staging.write(bytes);
        self.gpu.submit(|cmd| {
            self.barrier(
                cmd,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::AccessFlags::empty(),
                vk::AccessFlags::TRANSFER_WRITE,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
            );
            unsafe {
                self.gpu.device.cmd_copy_buffer_to_image(
                    cmd,
                    staging.handle,
                    self.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    regions,
                );
            }
            self.barrier(
                cmd,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                layout,
                vk::AccessFlags::TRANSFER_WRITE,
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::ALL_COMMANDS,
            );
            Ok(())
        })
    }
}
impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            if self.sampler != vk::Sampler::null() {
                self.gpu.device.destroy_sampler(self.sampler, None);
            }
            if self.view != vk::ImageView::null() {
                self.gpu.device.destroy_image_view(self.view, None);
            }
            if self.image != vk::Image::null() {
                self.gpu.device.destroy_image(self.image, None);
            }
            if self.memory != vk::DeviceMemory::null() {
                self.gpu.device.free_memory(self.memory, None);
            }
        }
    }
}
pub fn copy_region(extent: vk::Extent3D, level: u32, offset: u64) -> vk::BufferImageCopy {
    vk::BufferImageCopy::default()
        .buffer_offset(offset)
        .image_subresource(
            vk::ImageSubresourceLayers::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .mip_level(level)
                .layer_count(1),
        )
        .image_extent(extent)
}
