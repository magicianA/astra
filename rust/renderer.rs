//! Original HDR Vulkan sky pipelines, rendered to an exportable RGBA image.
use crate::{
    atmosphere::AtmosphereLut,
    calibration::BACKGROUND_RADIANCE_SCALE,
    gpu::{Buffer, Gpu, Texture, copy_region},
    math::{PI, RAD, Vec3},
    photometry as photo,
    render_scene::RenderScene,
};
use anyhow::{Result, ensure};
use ash::vk;
use std::{path::Path, sync::Arc};
const HDR: vk::Format = vk::Format::R32G32B32A32_SFLOAT;
const OUTPUT: vk::Format = vk::Format::R8G8B8A8_UNORM;
struct Targets {
    gpu: Arc<Gpu>,
    hdr: Texture,
    output: Texture,
    hdr_fb: vk::Framebuffer,
    output_fb: vk::Framebuffer,
    readback: Buffer,
}
impl Drop for Targets {
    fn drop(&mut self) {
        unsafe {
            self.gpu.device.destroy_framebuffer(self.hdr_fb, None);
            self.gpu.device.destroy_framebuffer(self.output_fb, None);
        }
    }
}
pub struct Renderer {
    pub gpu: Arc<Gpu>,
    pub effective_exposure: f32,
    displayed_gain: [f64; 2],
    exposure_started: bool,
    atmosphere: Vec<AtmosphereLut>,
    _textures: Vec<Texture>,
    pool: vk::DescriptorPool,
    sets: [vk::DescriptorSet; 3],
    atmosphere_sets: [vk::DescriptorSet; 2],
    feature_sets: [vk::DescriptorSet; 2],
    set_layouts: Vec<vk::DescriptorSetLayout>,
    layouts: [vk::PipelineLayout; 3],
    pipelines: [vk::Pipeline; 4],
    passes: [vk::RenderPass; 2],
    uniforms: [Buffer; 2],
    features: [Buffer; 2],
    meter: Buffer,
    vertices: Buffer,
    targets: Option<Targets>,
    adaptive: [f64; 2],
    metered: [bool; 2],
    enabled: [bool; 2],
    view_count: usize,
}
impl Renderer {
    pub fn new(data: &Path, validation: bool) -> Result<Self> {
        unsafe {
            let gpu = Gpu::new(validation)?;
            let mut arena = crate::gpu::HandleArena::new(&gpu);
            let properties = gpu
                .instance
                .get_physical_device_format_properties(gpu.physical, HDR);
            ensure!(
                properties.optimal_tiling_features.contains(
                    vk::FormatFeatureFlags::COLOR_ATTACHMENT
                        | vk::FormatFeatureFlags::COLOR_ATTACHMENT_BLEND
                        | vk::FormatFeatureFlags::SAMPLED_IMAGE
                ),
                "RGBA32F blending required"
            );
            let mut atmosphere = Vec::new();
            for (i, name) in ["clear.bin", "hazy.bin"].iter().enumerate() {
                atmosphere.push(AtmosphereLut::load(
                    &gpu,
                    &data.join("atmosphere").join(name),
                    i as u32,
                    8,
                )?);
            }
            let background = load_image(&gpu, &data.join("background/milky-way.png"), true, true)?;
            let moon_albedo = load_image(&gpu, &data.join("moon/albedo.png"), true, true)?;
            let moon_height = load_image(&gpu, &data.join("moon/height.png"), false, false)?;
            let textures = vec![background, moon_albedo, moon_height];
            let bindings = |types: &[vk::DescriptorType], stage| -> Vec<_> {
                types
                    .iter()
                    .enumerate()
                    .map(|(i, &ty)| {
                        vk::DescriptorSetLayoutBinding::default()
                            .binding(i as u32)
                            .descriptor_count(1)
                            .descriptor_type(ty)
                            .stage_flags(stage)
                    })
                    .collect()
            };
            let sampler = vk::DescriptorType::COMBINED_IMAGE_SAMPLER;
            let uniform = vk::DescriptorType::UNIFORM_BUFFER;
            let mut set_layouts = Vec::new();
            for types in [
                vec![sampler],
                vec![
                    sampler, sampler, sampler, sampler, sampler, sampler, sampler, sampler, uniform,
                ],
                vec![sampler, sampler, uniform],
                vec![sampler, vk::DescriptorType::STORAGE_BUFFER],
            ] {
                let stage = if set_layouts.len() == 3 {
                    vk::ShaderStageFlags::COMPUTE
                } else {
                    vk::ShaderStageFlags::FRAGMENT
                };
                let b = bindings(&types, stage);
                set_layouts.push(arena.own(gpu.device.create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&b),
                    None,
                )?));
            }
            let sizes = [
                vk::DescriptorPoolSize {
                    ty: sampler,
                    descriptor_count: 32,
                },
                vk::DescriptorPoolSize {
                    ty: uniform,
                    descriptor_count: 4,
                },
                vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::STORAGE_BUFFER,
                    descriptor_count: 1,
                },
            ];
            let pool = arena.own(
                gpu.device.create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .max_sets(8)
                        .pool_sizes(&sizes),
                    None,
                )?,
            );
            let allocated = gpu.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&[
                        set_layouts[0],
                        set_layouts[0],
                        set_layouts[3],
                        set_layouts[1],
                        set_layouts[1],
                        set_layouts[2],
                        set_layouts[2],
                    ]),
            )?;
            let sets = [allocated[0], allocated[1], allocated[2]];
            let atmosphere_sets = [allocated[3], allocated[4]];
            let feature_sets = [allocated[5], allocated[6]];
            let uniforms = [
                Buffer::new(&gpu, 80, vk::BufferUsageFlags::UNIFORM_BUFFER)?,
                Buffer::new(&gpu, 80, vk::BufferUsageFlags::UNIFORM_BUFFER)?,
            ];
            let features = [
                Buffer::new(&gpu, 748 * 4, vk::BufferUsageFlags::UNIFORM_BUFFER)?,
                Buffer::new(&gpu, 748 * 4, vk::BufferUsageFlags::UNIFORM_BUFFER)?,
            ];
            let meter = Buffer::new(&gpu, 8192 * 4, vk::BufferUsageFlags::STORAGE_BUFFER)?;
            let vertices = Buffer::new(&gpu, 48, vk::BufferUsageFlags::VERTEX_BUFFER)?;
            write_image(
                &gpu,
                sets[0],
                0,
                textures[0].descriptor(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
            );
            for v in 0..2 {
                for i in 0..8 {
                    write_image(
                        &gpu,
                        atmosphere_sets[v],
                        i as u32,
                        atmosphere[i / 4].images[i % 4].descriptor(vk::ImageLayout::GENERAL),
                    );
                }
                write_buffer(&gpu, atmosphere_sets[v], 8, &uniforms[v], uniform);
                for i in 0..2 {
                    write_image(
                        &gpu,
                        feature_sets[v],
                        i as u32,
                        textures[i + 1].descriptor(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
                    );
                }
                write_buffer(&gpu, feature_sets[v], 2, &features[v], uniform);
            }
            write_buffer(&gpu, sets[2], 1, &meter, vk::DescriptorType::STORAGE_BUFFER);
            let passes = [
                arena.own(render_pass(
                    &gpu,
                    HDR,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                )?),
                arena.own(render_pass(
                    &gpu,
                    OUTPUT,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                )?),
            ];
            let stages = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
            let ranges = [vk::PushConstantRange::default()
                .size(128)
                .stage_flags(stages)];
            let sky_layout = arena.own(
                gpu.device.create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&set_layouts[..3])
                        .push_constant_ranges(&ranges),
                    None,
                )?,
            );
            let tone_range = [vk::PushConstantRange::default()
                .size(12)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
            let tone_layout = arena.own(
                gpu.device.create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&set_layouts[..1])
                        .push_constant_ranges(&tone_range),
                    None,
                )?,
            );
            let meter_range = [vk::PushConstantRange::default()
                .size(12)
                .stage_flags(vk::ShaderStageFlags::COMPUTE)];
            let meter_layout = arena.own(
                gpu.device.create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&set_layouts[3..])
                        .push_constant_ranges(&meter_range),
                    None,
                )?,
            );
            let sky = arena.own(pipeline(
                &gpu,
                include_bytes!(concat!(env!("OUT_DIR"), "/sky.vert.spv")),
                include_bytes!(concat!(env!("OUT_DIR"), "/sky.frag.spv")),
                sky_layout,
                passes[0],
                false,
                true,
            )?);
            let stars = arena.own(pipeline(
                &gpu,
                include_bytes!(concat!(env!("OUT_DIR"), "/stars.vert.spv")),
                include_bytes!(concat!(env!("OUT_DIR"), "/stars.frag.spv")),
                sky_layout,
                passes[0],
                true,
                true,
            )?);
            let tone = arena.own(pipeline(
                &gpu,
                include_bytes!(concat!(env!("OUT_DIR"), "/sky.vert.spv")),
                include_bytes!(concat!(env!("OUT_DIR"), "/tonemap.frag.spv")),
                tone_layout,
                passes[1],
                false,
                false,
            )?);
            let mut shaders = crate::gpu::HandleArena::new(&gpu);
            let shader = shaders.own(gpu.shader(include_bytes!(concat!(
                env!("OUT_DIR"),
                "/exposure.comp.spv"
            )))?);
            let stage = vk::PipelineShaderStageCreateInfo::default()
                .module(shader)
                .stage(vk::ShaderStageFlags::COMPUTE)
                .name(c"main");
            let meter_pipeline = arena.own(
                gpu.device
                    .create_compute_pipelines(
                        vk::PipelineCache::null(),
                        &[vk::ComputePipelineCreateInfo::default()
                            .layout(meter_layout)
                            .stage(stage)],
                        None,
                    )
                    .map_err(|(pipelines, e)| {
                        for pipeline in pipelines {
                            gpu.device.destroy_pipeline(pipeline, None);
                        }
                        e
                    })?[0],
            );
            arena.disarm();
            Ok(Self {
                gpu,
                effective_exposure: 40.,
                displayed_gain: [40.; 2],
                exposure_started: false,
                atmosphere,
                _textures: textures,
                pool,
                sets,
                atmosphere_sets,
                feature_sets,
                set_layouts,
                layouts: [sky_layout, tone_layout, meter_layout],
                pipelines: [sky, stars, tone, meter_pipeline],
                passes,
                uniforms,
                features,
                meter,
                vertices,
                targets: None,
                adaptive: [40.; 2],
                metered: [false; 2],
                enabled: [false; 2],
                view_count: 1,
            })
        }
    }
    pub fn render(
        &mut self,
        primary: &RenderScene,
        comparison: Option<&RenderScene>,
        width: u32,
        height: u32,
        dt: f64,
    ) -> Result<image::RgbaImage> {
        unsafe {
            ensure!(
                width > 0 && height > 0 && width <= 16384 && height <= 16384,
                "Invalid output size"
            );
            if self
                .targets
                .as_ref()
                .is_none_or(|t| t.hdr.extent.width != width || t.hdr.extent.height != height)
            {
                self.targets = None;
                let extent = vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                };
                let hdr = Texture::new(
                    &self.gpu,
                    extent,
                    HDR,
                    vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                    1,
                    false,
                    false,
                    1.,
                )?;
                let output = Texture::new(
                    &self.gpu,
                    extent,
                    OUTPUT,
                    vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC,
                    1,
                    false,
                    false,
                    1.,
                )?;
                let fb = |pass, view| {
                    self.gpu.device.create_framebuffer(
                        &vk::FramebufferCreateInfo::default()
                            .render_pass(pass)
                            .attachments(&[view])
                            .width(width)
                            .height(height)
                            .layers(1),
                        None,
                    )
                };
                let mut frames = crate::gpu::HandleArena::new(&self.gpu);
                let hdr_fb = frames.own(fb(self.passes[0], hdr.view)?);
                let output_fb = frames.own(fb(self.passes[1], output.view)?);
                let readback = Buffer::new(
                    &self.gpu,
                    width as u64 * height as u64 * 4,
                    vk::BufferUsageFlags::TRANSFER_DST,
                )?;
                write_image(
                    &self.gpu,
                    self.sets[1],
                    0,
                    hdr.descriptor(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
                );
                write_image(
                    &self.gpu,
                    self.sets[2],
                    0,
                    hdr.descriptor(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
                );
                frames.disarm();
                self.targets = Some(Targets {
                    gpu: self.gpu.clone(),
                    hdr,
                    output,
                    hdr_fb,
                    output_fb,
                    readback,
                });
                self.metered = [false; 2];
            }
            let scenes: Vec<_> = std::iter::once(primary).chain(comparison).collect();
            if scenes.len() != self.view_count {
                self.metered = [false; 2];
                self.view_count = scenes.len();
            }
            let mut tone = [
                primary.exposure,
                comparison.map_or(primary.exposure, |s| s.exposure),
                if comparison.is_some() {
                    (width / 2) as f32
                } else {
                    width as f32
                },
            ];
            let first = primary.points.len();
            let count = first + comparison.map_or(0, |s| s.points.len());
            let bytes = (count * 48).max(48) as u64;
            if self.vertices.size < bytes {
                self.vertices =
                    Buffer::new(&self.gpu, bytes * 2, vk::BufferUsageFlags::VERTEX_BUFFER)?;
            }
            let mut point_bytes = Vec::with_capacity(count * 48);
            for s in &scenes {
                for p in &s.points {
                    point_bytes.extend_from_slice(bytemuck::cast_slice(&[
                        p.x,
                        p.y,
                        p.radius,
                        p.kind,
                        p.r,
                        p.g,
                        p.b,
                        p.brightness,
                        p.phase,
                        p.limb,
                        p.pad0,
                        p.pad1,
                    ]));
                }
            }
            self.vertices.write(&point_bytes);
            for (v, s) in scenes.iter().enumerate() {
                if self.enabled[v] != s.adaptive_exposure {
                    self.metered[v] = false;
                    self.adaptive[v] = if self.exposure_started {
                        self.displayed_gain[v] / primary.exposure as f64
                    } else {
                        0.0
                    };
                    self.enabled[v] = s.adaptive_exposure;
                }
                if s.adaptive_exposure && self.metered[v] {
                    let all = self.meter.read();
                    let mut values: Vec<f32> = all[v * 4096 * 4..(v + 1) * 4096 * 4]
                        .chunks_exact(4)
                        .map(|b| f32::from_ne_bytes(b.try_into().unwrap()))
                        .collect();
                    let target = photo::view_exposure_gain(&mut values);
                    let mut remaining = dt.max(0.);
                    while remaining > 1e-9 {
                        let step = remaining.min(0.1);
                        self.adaptive[v] = photo::adapt_exposure(self.adaptive[v], target, step);
                        remaining -= step;
                    }
                }
                let (uniform, gain) =
                    atmosphere_uniform(s, &self.atmosphere[s.atmosphere_preset as usize]);
                if s.adaptive_exposure {
                    tone[v] *= (self.adaptive[v] / photo::NIGHT_EXPOSURE_GAIN) as f32;
                }
                if v == 0 {
                    self.effective_exposure = gain * tone[v];
                }
                self.displayed_gain[v] = gain as f64 * tone[v] as f64;
                self.uniforms[v].write(bytemuck::cast_slice(&uniform));
                let mut features = [0f32; 748];
                for i in 0..3 {
                    for j in 0..3 {
                        features[i * 4 + j] = s.lunar.fixed_from_enu.a[i][j] as f32;
                    }
                    features[12 + i] = s.lunar.sun_km[i] as f32;
                    features[16 + i] = s.lunar.earth_km[i] as f32;
                }
                features[20] = u32::from(s.moon_surface) as f32;
                features[21] = s.lunar.earthshine_lux as f32;
                features[24] = if v == 1 { (width / 2) as f32 } else { 0. };
                features[28..].copy_from_slice(&s.horizon.degrees);
                self.features[v].write(bytemuck::cast_slice(&features));
            }
            self.exposure_started = true;
            let t = self.targets.as_ref().unwrap();
            let gpu = &self.gpu;
            let d = &gpu.device;
            gpu.submit(|cmd| {
                let clear = [vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [0., 0., 0., 1.],
                    },
                }];
                let area = vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D { width, height },
                };
                let pass = vk::RenderPassBeginInfo::default()
                    .render_pass(self.passes[0])
                    .framebuffer(t.hdr_fb)
                    .render_area(area)
                    .clear_values(&clear);
                d.cmd_begin_render_pass(cmd, &pass, vk::SubpassContents::INLINE);
                for (v, s) in scenes.iter().enumerate() {
                    let origin = if v == 1 { width / 2 } else { 0 };
                    let w = if scenes.len() == 2 {
                        if v == 0 { width / 2 } else { width - origin }
                    } else {
                        width
                    };
                    d.cmd_set_viewport(
                        cmd,
                        0,
                        &[vk::Viewport {
                            x: origin as f32,
                            y: 0.,
                            width: w as f32,
                            height: height as f32,
                            min_depth: 0.,
                            max_depth: 1.,
                        }],
                    );
                    d.cmd_set_scissor(
                        cmd,
                        0,
                        &[vk::Rect2D {
                            offset: vk::Offset2D {
                                x: origin as i32,
                                y: 0,
                            },
                            extent: vk::Extent2D { width: w, height },
                        }],
                    );
                    let mut p = [0f32; 32];
                    let copy = |p: &mut [f32], v: Vec3| {
                        p[0] = v.x as f32;
                        p[1] = v.y as f32;
                        p[2] = v.z as f32;
                    };
                    copy(&mut p[0..], s.camera.right());
                    p[3] = s.camera.pixels_per_radian() as f32;
                    copy(&mut p[4..], s.camera.up());
                    p[7] = (s.camera.fov / 2.) as f32;
                    copy(&mut p[8..], s.camera.forward());
                    p[11] = s.camera.projection as i32 as f32;
                    copy(&mut p[12..], s.sun);
                    p[15] = u32::from(s.atmosphere) as f32;
                    copy(&mut p[16..], s.moon);
                    p[19] = u32::from(s.ground) as f32;
                    p[20] = s.pollution;
                    p[21] = s.moon_phase;
                    p[22] = u32::from(s.milky_way) as f32;
                    p[23] = s.extinction;
                    p[24] = s.camera.width as f32;
                    p[25] = s.camera.height as f32;
                    p[26] = w as f32;
                    p[27] = height as f32;
                    p[28..].copy_from_slice(&s.galactic_rotation);
                    d.cmd_bind_descriptor_sets(
                        cmd,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.layouts[0],
                        0,
                        &[self.sets[0], self.atmosphere_sets[v], self.feature_sets[v]],
                        &[],
                    );
                    d.cmd_push_constants(
                        cmd,
                        self.layouts[0],
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0,
                        bytemuck::cast_slice(&p),
                    );
                    d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines[0]);
                    d.cmd_draw(cmd, 3, 1, 0, 0);
                    if !s.points.is_empty() {
                        d.cmd_bind_pipeline(
                            cmd,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.pipelines[1],
                        );
                        d.cmd_bind_vertex_buffers(
                            cmd,
                            0,
                            &[self.vertices.handle],
                            &[if v == 0 { 0 } else { (first * 48) as u64 }],
                        );
                        d.cmd_draw(cmd, 6, s.points.len() as u32, 0, 0);
                    }
                }
                d.cmd_end_render_pass(cmd);
                let barrier = [vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)];
                d.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::PipelineStageFlags::FRAGMENT_SHADER
                        | vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &barrier,
                    &[],
                    &[],
                );
                for (v, s) in scenes.iter().enumerate() {
                    if s.adaptive_exposure {
                        let origin = if v == 1 { width / 2 } else { 0 };
                        let w = if scenes.len() == 2 && v == 0 {
                            width / 2
                        } else {
                            width - origin
                        };
                        d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.pipelines[3]);
                        d.cmd_bind_descriptor_sets(
                            cmd,
                            vk::PipelineBindPoint::COMPUTE,
                            self.layouts[2],
                            0,
                            &[self.sets[2]],
                            &[],
                        );
                        d.cmd_push_constants(
                            cmd,
                            self.layouts[2],
                            vk::ShaderStageFlags::COMPUTE,
                            0,
                            bytemuck::cast_slice(&[origin as i32, w as i32, v as i32]),
                        );
                        d.cmd_dispatch(cmd, 8, 8, 1);
                    }
                }
                d.cmd_set_viewport(
                    cmd,
                    0,
                    &[vk::Viewport {
                        x: 0.,
                        y: 0.,
                        width: width as f32,
                        height: height as f32,
                        min_depth: 0.,
                        max_depth: 1.,
                    }],
                );
                d.cmd_set_scissor(cmd, 0, &[area]);
                let pass = vk::RenderPassBeginInfo::default()
                    .render_pass(self.passes[1])
                    .framebuffer(t.output_fb)
                    .render_area(area)
                    .clear_values(&clear);
                d.cmd_begin_render_pass(cmd, &pass, vk::SubpassContents::INLINE);
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.pipelines[2]);
                d.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layouts[1],
                    0,
                    &[self.sets[1]],
                    &[],
                );
                d.cmd_push_constants(
                    cmd,
                    self.layouts[1],
                    vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::cast_slice(&tone),
                );
                d.cmd_draw(cmd, 3, 1, 0, 0);
                d.cmd_end_render_pass(cmd);
                let barrier = [vk::MemoryBarrier::default()
                    .src_access_mask(
                        vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::SHADER_WRITE,
                    )
                    .dst_access_mask(vk::AccessFlags::TRANSFER_READ | vk::AccessFlags::HOST_READ)];
                d.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        | vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::PipelineStageFlags::TRANSFER | vk::PipelineStageFlags::HOST,
                    vk::DependencyFlags::empty(),
                    &barrier,
                    &[],
                    &[],
                );
                d.cmd_copy_image_to_buffer(
                    cmd,
                    t.output.image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    t.readback.handle,
                    &[copy_region(t.output.extent, 0, 0)],
                );
                Ok(())
            })?;
            for (v, s) in scenes.iter().enumerate() {
                self.metered[v] = s.adaptive_exposure;
            }
            image::RgbaImage::from_raw(width, height, t.readback.read())
                .ok_or_else(|| anyhow::anyhow!("Invalid readback"))
        }
    }
}
impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.gpu.wait_idle();
            self.targets = None;
            for p in self.pipelines {
                self.gpu.device.destroy_pipeline(p, None);
            }
            for l in self.layouts {
                self.gpu.device.destroy_pipeline_layout(l, None);
            }
            self.gpu.device.destroy_descriptor_pool(self.pool, None);
            for &l in &self.set_layouts {
                self.gpu.device.destroy_descriptor_set_layout(l, None);
            }
            for p in self.passes {
                self.gpu.device.destroy_render_pass(p, None);
            }
        }
    }
}
fn write_image(g: &Gpu, set: vk::DescriptorSet, binding: u32, info: vk::DescriptorImageInfo) {
    unsafe {
        g.device.update_descriptor_sets(
            &[vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&[info])],
            &[],
        );
    }
}
fn write_buffer(g: &Gpu, set: vk::DescriptorSet, binding: u32, b: &Buffer, ty: vk::DescriptorType) {
    unsafe {
        g.device.update_descriptor_sets(
            &[vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(binding)
                .descriptor_type(ty)
                .buffer_info(&[vk::DescriptorBufferInfo {
                    buffer: b.handle,
                    offset: 0,
                    range: b.size,
                }])],
            &[],
        );
    }
}
fn render_pass(g: &Gpu, format: vk::Format, end: vk::ImageLayout) -> Result<vk::RenderPass> {
    unsafe {
        let attachments = [vk::AttachmentDescription::default()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(end)];
        let references = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];
        let subpasses = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&references)];
        let dependencies = [
            vk::SubpassDependency::default()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(vk::PipelineStageFlags::ALL_COMMANDS)
                .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .src_access_mask(vk::AccessFlags::MEMORY_READ)
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE),
            vk::SubpassDependency::default()
                .src_subpass(0)
                .dst_subpass(vk::SUBPASS_EXTERNAL)
                .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(vk::PipelineStageFlags::ALL_COMMANDS)
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::MEMORY_READ),
        ];
        Ok(g.device.create_render_pass(
            &vk::RenderPassCreateInfo::default()
                .attachments(&attachments)
                .subpasses(&subpasses)
                .dependencies(&dependencies),
            None,
        )?)
    }
}
fn pipeline(
    g: &Arc<Gpu>,
    vertex: &[u8],
    fragment: &[u8],
    layout: vk::PipelineLayout,
    pass: vk::RenderPass,
    stars: bool,
    filtering: bool,
) -> Result<vk::Pipeline> {
    unsafe {
        let mut shaders = crate::gpu::HandleArena::new(g);
        let vs = shaders.own(g.shader(vertex)?);
        let fs = shaders.own(g.shader(fragment)?);
        let entries = [vk::SpecializationMapEntry {
            constant_id: 0,
            offset: 0,
            size: 4,
        }];
        let data = [u32::from(g.manual_filter)];
        let spec = vk::SpecializationInfo::default()
            .map_entries(&entries)
            .data(bytemuck::cast_slice(&data));
        let mut fragment = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fs)
            .name(c"main");
        if filtering {
            fragment = fragment.specialization_info(&spec);
        }
        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vs)
                .name(c"main"),
            fragment,
        ];
        let binding = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: 48,
            input_rate: vk::VertexInputRate::INSTANCE,
        }];
        let attrs: Vec<_> = (0..3)
            .map(|i| vk::VertexInputAttributeDescription {
                location: i,
                binding: 0,
                format: vk::Format::R32G32B32A32_SFLOAT,
                offset: i * 16,
            })
            .collect();
        let mut vi = vk::PipelineVertexInputStateCreateInfo::default();
        if stars {
            vi = vi
                .vertex_binding_descriptions(&binding)
                .vertex_attribute_descriptions(&attrs);
        }
        let ia = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let raster = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.);
        let ms = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let attachment = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(stars)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD)];
        let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachment);
        let dynamic = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]);
        let info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vi)
            .input_assembly_state(&ia)
            .viewport_state(&viewport)
            .rasterization_state(&raster)
            .multisample_state(&ms)
            .color_blend_state(&blend)
            .dynamic_state(&dynamic)
            .layout(layout)
            .render_pass(pass);
        let result = g
            .device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
            .map_err(|(pipelines, e)| {
                for pipeline in pipelines {
                    g.device.destroy_pipeline(pipeline, None);
                }
                e
            });

        Ok(result?[0])
    }
}
fn load_image(gpu: &Arc<Gpu>, path: &Path, srgb: bool, mips: bool) -> Result<Texture> {
    let image = image::open(path)?.into_rgba8();
    let maximum = unsafe { gpu.instance.get_physical_device_properties(gpu.physical) }
        .limits
        .max_image_dimension2_d;
    let chain = if mips {
        Some(crate::image_mips::make_srgb_mips(
            image.width(),
            image.height(),
            image.as_raw(),
        )?)
    } else {
        None
    };
    let (bytes, regions, extent, levels) = if let Some(chain) = chain {
        let first = chain
            .levels
            .iter()
            .position(|l| l.width <= maximum && l.height <= maximum)
            .ok_or_else(|| anyhow::anyhow!("No supported image mip"))?;
        let base = chain.levels[first];
        let regions = chain.levels[first..]
            .iter()
            .enumerate()
            .map(|(i, l)| {
                copy_region(
                    vk::Extent3D {
                        width: l.width,
                        height: l.height,
                        depth: 1,
                    },
                    i as u32,
                    (l.offset - base.offset) as u64,
                )
            })
            .collect::<Vec<_>>();
        let extent = vk::Extent3D {
            width: base.width,
            height: base.height,
            depth: 1,
        };
        (
            chain.pixels[base.offset..].to_vec(),
            regions,
            extent,
            (chain.levels.len() - first) as u32,
        )
    } else {
        ensure!(
            image.width() <= maximum && image.height() <= maximum,
            "GPU cannot represent packed lunar height map without changing its encoding"
        );
        let extent = vk::Extent3D {
            width: image.width(),
            height: image.height(),
            depth: 1,
        };
        (image.into_raw(), vec![copy_region(extent, 0, 0)], extent, 1)
    };
    let texture = Texture::new(
        gpu,
        extent,
        if srgb {
            vk::Format::R8G8B8A8_SRGB
        } else {
            vk::Format::R8G8B8A8_UNORM
        },
        vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
        levels,
        true,
        true,
        if mips { gpu.anisotropy } else { 1. },
    )?;
    texture.upload(&bytes, &regions, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)?;
    Ok(texture)
}

fn atmosphere_uniform(s: &RenderScene, lut: &AtmosphereLut) -> ([f32; 20], f32) {
    let twilight = photo::twilight_gain(-s.geometric_sun.z.asin() / RAD);
    let pollution = photo::pollution_luminance(s.pollution as f64);
    let solar = lut.irradiance(s.height_km, s.geometric_sun.z as f32);
    let mut lunar = lut.irradiance(s.height_km, s.geometric_moon.z as f32);
    let y = [0.2126, 0.7152, 0.0722];
    let transmission = lut.transmittance(s.height_km, s.geometric_moon.z as f32);
    let visible = ((s.geometric_moon.z + 0.0047) / 0.0094).clamp(0., 1.);
    let mut moon_t = 0.;
    let mut old = 0.;
    for i in 0..3 {
        moon_t += y[i] * photo::SOLAR_RGB[i] * transmission[i] as f64 / photo::SOLAR_LUX;
        lunar[i] *= s.lunar_flux;
        old += y[i] * lunar[i] as f64 / PI;
    }
    let mut mean = 0.;
    if s.atmosphere && s.lunar_flux > 1e-10 && visible > 0. {
        for j in 0..12 {
            let mu = (j as f64 + 0.5) / 12.;
            let t = lut.transmittance(s.height_km, mu as f32);
            let view_t = (0..3)
                .map(|c| y[c] * photo::SOLAR_RGB[c] * t[c] as f64 / photo::SOLAR_LUX)
                .sum();
            for k in 0..24 {
                let cosine = mu * s.geometric_moon.z
                    + ((1. - mu * mu) * (1.0_f64 - s.geometric_moon.z.powi(2)).max(0.)).sqrt()
                        * (2. * PI * (k as f64 + 0.5) / 24.).cos();
                mean += 2. * mu / (12. * 24.)
                    * photo::lunar_sky_luminance(
                        s.lunar_flux as f64 * photo::SOLAR_LUX,
                        cosine.clamp(-1., 1.).acos(),
                        moon_t,
                        view_t,
                    );
            }
        }
    }
    mean = old * (1. - visible) + mean * visible;
    for c in &mut lunar {
        *c *= (mean / old.max(1e-20)) as f32;
    }
    let mut gain = photo::NIGHT_EXPOSURE_GAIN;
    if s.atmosphere && s.auto_exposure && !s.adaptive_exposure {
        let mut mean =
            photo::NIGHT_FLOOR + pollution * (1.5 + 0.5 * (-PI).exp()) / (1. + 2. * (-PI).exp());
        for i in 0..3 {
            mean +=
                y[i] * (solar[i] as f64 * s.solar_flux as f64 * twilight + lunar[i] as f64) / PI;
        }
        gain = photo::exposure_gain(
            mean,
            s.lunar_flux as f64 * photo::SOLAR_LUX * moon_t * visible,
        );
    }
    (
        [
            s.geometric_sun.x as f32,
            s.geometric_sun.y as f32,
            s.geometric_sun.z as f32,
            s.solar_flux,
            s.geometric_moon.x as f32,
            s.geometric_moon.y as f32,
            s.geometric_moon.z as f32,
            s.lunar_flux,
            s.height_km,
            s.atmosphere_preset as f32,
            u32::from(s.auto_exposure) as f32,
            gain as f32,
            twilight as f32,
            photo::NIGHT_FLOOR as f32,
            pollution as f32,
            BACKGROUND_RADIANCE_SCALE,
            lunar[0],
            lunar[1],
            lunar[2],
            0.,
        ],
        gain as f32,
    )
}
