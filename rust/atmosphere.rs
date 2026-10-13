use crate::gpu::{Buffer, Gpu, HandleArena, Texture, copy_region};
use anyhow::{Context, Result, ensure};
use ash::vk;
use std::{path::Path, sync::Arc};
const DIMENSIONS: [(u32, u32, u32); 9] = [
    (256, 64, 1),
    (256, 128, 32),
    (256, 128, 32),
    (64, 16, 1),
    (64, 16, 1),
    (256, 128, 32),
    (256, 128, 32),
    (256, 128, 32),
    (256, 128, 32),
];
fn extent(i: usize) -> vk::Extent3D {
    let (width, height, depth) = DIMENSIONS[i];
    vk::Extent3D {
        width,
        height,
        depth,
    }
}
fn size(i: usize) -> usize {
    let (w, h, d) = DIMENSIONS[i];
    (w * h * d * 16) as usize
}
fn crc(bytes: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = (c >> 1) ^ if c & 1 != 0 { 0xedb88320 } else { 0 };
        }
        *t = c;
    }
    let mut c = !0u32;
    for &v in bytes {
        c = table[((c ^ v as u32) & 255) as usize] ^ (c >> 8);
    }
    !c
}
// Match std::lerp's endpoint and monotonicity guarantees at the original
// calculation precision, including zero-valued LUT samples.
macro_rules! lerp {
    ($name:ident, $float:ty) => {
        fn $name(a: $float, b: $float, t: $float) -> $float {
            if (a <= 0.0 && b >= 0.0) || (a >= 0.0 && b <= 0.0) {
                t * b + (1.0 - t) * a
            } else if t == 1.0 {
                b
            } else {
                let x = a + t * (b - a);
                if (t > 1.0) == (b > a) {
                    b.max(x)
                } else {
                    b.min(x)
                }
            }
        }
    };
}
lerp!(lerp_f32, f32);
lerp!(lerp_f64, f64);

pub struct AtmosphereLut {
    pub images: Vec<Texture>,
    values: Vec<f32>,
}
impl AtmosphereLut {
    pub fn load(gpu: &Arc<Gpu>, path: &Path, preset: u32, orders: u32) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| {
            format!(
                "Missing atmosphere cache {}; run astra_atmosphere",
                path.display()
            )
        })?;
        let length: usize = (0..4).map(size).sum();
        ensure!(
            bytes.len() == 92 + length,
            "Invalid atmosphere cache length"
        );
        ensure!(
            &bytes[..8] == b"ASTRAAT2"
                && &bytes[8..72] == env!("ASTRA_ATMOSPHERE_ID").as_bytes()
                && bytes[72] == 0,
            "Atmosphere cache model mismatch"
        );
        let read = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        ensure!(
            read(76) == preset && read(80) == orders && read(84) == 15,
            "Incompatible atmosphere cache parameters"
        );
        ensure!(
            read(88) == crc(&bytes[92..]),
            "Atmosphere cache checksum mismatch"
        );
        let values: Vec<f32> = bytes[92..]
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        ensure!(
            values.iter().all(|v| v.is_finite()),
            "Non-finite atmosphere table"
        );
        let mut images = Vec::new();
        let mut offset = 92;
        for i in 0..4 {
            let texture = Texture::new(
                gpu,
                extent(i),
                vk::Format::R32G32B32A32_SFLOAT,
                vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
                1,
                !gpu.manual_filter,
                false,
                1.,
            )?;
            texture.upload(
                &bytes[offset..offset + size(i)],
                &[copy_region(extent(i), 0, 0)],
                vk::ImageLayout::GENERAL,
            )?;
            offset += size(i);
            images.push(texture);
        }
        Ok(Self { images, values })
    }
    pub fn irradiance(&self, height: f32, z: f32) -> [f32; 3] {
        let x = (z * 0.5 + 0.5).clamp(0., 1.) * 63.;
        let y = (height.max(0.001) / 60.).clamp(0., 1.) * 15.;
        let offset = self.values.len() - 64 * 16 * 4;
        self.sample(offset, 64, 16, x, y)
    }
    fn sample(&self, offset: usize, width: usize, height: usize, x: f32, y: f32) -> [f32; 3] {
        let ix = x as usize;
        let iy = y as usize;
        let fx = x - ix as f32;
        let fy = y - iy as f32;
        std::array::from_fn(|c| {
            let value = |dx: usize, dy: usize| {
                self.values[offset
                    + ((iy + dy).min(height - 1) * width + (ix + dx).min(width - 1)) * 4
                    + c]
            };
            let lo = lerp_f32(value(0, 0), value(1, 0), fx);
            let hi = lerp_f32(value(0, 1), value(1, 1), fx);
            lerp_f32(lo, hi, fy).max(0.)
        })
    }
    pub fn transmittance(&self, height: f32, z: f32) -> [f32; 3] {
        let r = 6360. + (height as f64).clamp(0.001, 59.999);
        let h = (6420f64.powi(2) - 6360f64.powi(2)).sqrt();
        let rho = (r * r - 6360f64.powi(2)).sqrt();
        let mu = (z as f64).max(0.);
        let d = -r * mu + (r * r * (mu * mu - 1.) + 6420f64.powi(2)).sqrt();
        let x = ((d - (6420. - r)) / (rho + h - (6420. - r))).clamp(0., 1.) * 255.;
        let y = (rho / h).clamp(0., 1.) * 63.;
        // The original CPU exposure calculation interpolates this mapping in
        // double precision, then rounds only the final table sample to f32.
        let ix = x as usize;
        let iy = y as usize;
        let fx = x - ix as f64;
        let fy = y - iy as f64;
        std::array::from_fn(|c| {
            let value = |dx: usize, dy: usize| {
                self.values[((iy + dy).min(63) * 256 + (ix + dx).min(255)) * 4 + c] as f64
            };
            let lo = lerp_f64(value(0, 0), value(1, 0), fx);
            let hi = lerp_f64(value(0, 1), value(1, 1), fx);
            lerp_f64(lo, hi, fy) as f32
        })
    }
    pub fn generate(gpu: &Arc<Gpu>, path: &Path, preset: u32, orders: u32) -> Result<()> {
        unsafe {
            ensure!(
                preset < 2 && (2..=16).contains(&orders),
                "Invalid atmosphere preset/orders"
            );
            let format = gpu.instance.get_physical_device_format_properties(
                gpu.physical,
                vk::Format::R32G32B32A32_SFLOAT,
            );
            ensure!(
                format.optimal_tiling_features.contains(
                    vk::FormatFeatureFlags::SAMPLED_IMAGE | vk::FormatFeatureFlags::STORAGE_IMAGE
                ),
                "Physical atmosphere requires RGBA32F sampled/storage images"
            );
            let props = gpu.instance.get_physical_device_properties(gpu.physical);
            ensure!(
                props.limits.max_per_stage_descriptor_storage_images >= 9,
                "Generator requires nine storage images"
            );
            let mut images = Vec::new();
            for i in 0..9 {
                images.push(Texture::new(
                    gpu,
                    extent(i),
                    vk::Format::R32G32B32A32_SFLOAT,
                    vk::ImageUsageFlags::STORAGE
                        | vk::ImageUsageFlags::SAMPLED
                        | vk::ImageUsageFlags::TRANSFER_SRC
                        | vk::ImageUsageFlags::TRANSFER_DST,
                    1,
                    !gpu.manual_filter,
                    false,
                    1.,
                )?);
            }
            gpu.submit(|cmd| {
                for image in &images {
                    image.barrier(
                        cmd,
                        vk::ImageLayout::UNDEFINED,
                        vk::ImageLayout::GENERAL,
                        vk::AccessFlags::empty(),
                        vk::AccessFlags::TRANSFER_WRITE,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::TRANSFER,
                    );
                    gpu.device.cmd_clear_color_image(
                        cmd,
                        image.image,
                        vk::ImageLayout::GENERAL,
                        &vk::ClearColorValue { float32: [0.; 4] },
                        &[vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .level_count(1)
                            .layer_count(1)],
                    );
                }
                let b = [vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)];
                gpu.device.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &b,
                    &[],
                    &[],
                );
                Ok(())
            })?;
            let bindings: Vec<_> = (0..18)
                .map(|i| {
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(i)
                        .descriptor_count(1)
                        .descriptor_type(if i < 9 {
                            vk::DescriptorType::COMBINED_IMAGE_SAMPLER
                        } else {
                            vk::DescriptorType::STORAGE_IMAGE
                        })
                        .stage_flags(vk::ShaderStageFlags::COMPUTE)
                })
                .collect();
            let mut arena = HandleArena::new(gpu);
            let sl = arena.own(gpu.device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )?);
            let sizes = [
                vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                    descriptor_count: 9,
                },
                vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::STORAGE_IMAGE,
                    descriptor_count: 9,
                },
            ];
            let pool = arena.own(
                gpu.device.create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .max_sets(1)
                        .pool_sizes(&sizes),
                    None,
                )?,
            );
            let layouts = [sl];
            let set = gpu.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(pool)
                    .set_layouts(&layouts),
            )?[0];
            let infos: Vec<_> = (0..18)
                .map(|i| {
                    let mut d = images[i % 9].descriptor(vk::ImageLayout::GENERAL);
                    if i >= 9 {
                        d.sampler = vk::Sampler::null();
                    }
                    d
                })
                .collect();
            let writes: Vec<_> = (0..18)
                .map(|i| {
                    vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(i as u32)
                        .descriptor_type(bindings[i].descriptor_type)
                        .image_info(&infos[i..i + 1])
                })
                .collect();
            gpu.device.update_descriptor_sets(&writes, &[]);
            let ranges = [vk::PushConstantRange::default()
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
                .size(20)];
            let layout = arena.own(
                gpu.device.create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&layouts)
                        .push_constant_ranges(&ranges),
                    None,
                )?,
            );
            let shader = arena.own(gpu.shader(include_bytes!(concat!(
                env!("OUT_DIR"),
                "/atmosphere.comp.spv"
            )))?);
            let filtering = [u32::from(gpu.manual_filter)];
            let entries = [vk::SpecializationMapEntry {
                constant_id: 0,
                offset: 0,
                size: 4,
            }];
            let spec = vk::SpecializationInfo::default()
                .map_entries(&entries)
                .data(bytemuck::cast_slice(&filtering));
            let stage = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(shader)
                .name(c"main")
                .specialization_info(&spec);
            let pipeline = match gpu.device.create_compute_pipelines(
                vk::PipelineCache::null(),
                &[vk::ComputePipelineCreateInfo::default()
                    .layout(layout)
                    .stage(stage)],
                None,
            ) {
                Ok(pipelines) => arena.own(pipelines[0]),
                Err((pipelines, error)) => {
                    for handle in pipelines {
                        if handle != vk::Pipeline::null() {
                            arena.own(handle);
                        }
                    }
                    return Err(error.into());
                }
            };
            let result = (|| {
                let dispatch = |step: i32, order: i32, group: i32| -> Result<()> {
                    let e = extent(if step == 0 {
                        0
                    } else if step == 1 || step == 4 {
                        3
                    } else {
                        1
                    });
                    for z in (0..e.depth).step_by(4) {
                        gpu.submit(|cmd| {
                            gpu.device.cmd_bind_pipeline(
                                cmd,
                                vk::PipelineBindPoint::COMPUTE,
                                pipeline,
                            );
                            gpu.device.cmd_bind_descriptor_sets(
                                cmd,
                                vk::PipelineBindPoint::COMPUTE,
                                layout,
                                0,
                                &[set],
                                &[],
                            );
                            gpu.device.cmd_push_constants(
                                cmd,
                                layout,
                                vk::ShaderStageFlags::COMPUTE,
                                0,
                                bytemuck::cast_slice(&[
                                    step,
                                    order,
                                    group,
                                    preset as i32,
                                    z as i32,
                                ]),
                            );
                            gpu.device.cmd_dispatch(
                                cmd,
                                e.width / 8,
                                e.height / 8,
                                (e.depth - z).min(4),
                            );
                            let barrier = [vk::MemoryBarrier::default()
                                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                                .dst_access_mask(
                                    vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE,
                                )];
                            gpu.device.cmd_pipeline_barrier(
                                cmd,
                                vk::PipelineStageFlags::COMPUTE_SHADER,
                                vk::PipelineStageFlags::COMPUTE_SHADER,
                                vk::DependencyFlags::empty(),
                                &barrier,
                                &[],
                                &[],
                            );
                            Ok(())
                        })?;
                    }
                    Ok(())
                };
                for group in 0..15 {
                    eprintln!("Preset {preset}: spectral group {}/15", group + 1);
                    dispatch(0, 0, group)?;
                    dispatch(1, 0, group)?;
                    dispatch(2, 1, group)?;
                    for order in 2..=orders {
                        dispatch(3, order as i32, group)?;
                        dispatch(4, order as i32, group)?;
                        dispatch(5, order as i32, group)?;
                    }
                }
                dispatch(0, 0, 15)?;
                let length: usize = (0..4).map(size).sum();
                let buffer = Buffer::new(gpu, length as u64, vk::BufferUsageFlags::TRANSFER_DST)?;
                gpu.submit(|cmd| {
                    let barrier = [vk::MemoryBarrier::default()
                        .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                        .dst_access_mask(vk::AccessFlags::TRANSFER_READ)];
                    gpu.device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::COMPUTE_SHADER,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::DependencyFlags::empty(),
                        &barrier,
                        &[],
                        &[],
                    );
                    let mut offset = 0;
                    for (i, img) in images.iter().take(4).enumerate() {
                        gpu.device.cmd_copy_image_to_buffer(
                            cmd,
                            img.image,
                            vk::ImageLayout::GENERAL,
                            buffer.handle,
                            &[copy_region(extent(i), 0, offset)],
                        );
                        offset += size(i) as u64;
                    }
                    let visible = [vk::MemoryBarrier::default()
                        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                        .dst_access_mask(vk::AccessFlags::HOST_READ)];
                    gpu.device.cmd_pipeline_barrier(
                        cmd,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::PipelineStageFlags::HOST,
                        vk::DependencyFlags::empty(),
                        &visible,
                        &[],
                        &[],
                    );
                    Ok(())
                })?;
                let bytes = buffer.read();
                ensure!(
                    bytes
                        .chunks_exact(4)
                        .all(|b| f32::from_le_bytes(b.try_into().unwrap()).is_finite()),
                    "Non-finite generated table"
                );
                let mut header = [0u8; 92];
                header[..8].copy_from_slice(b"ASTRAAT2");
                header[8..72].copy_from_slice(env!("ASTRA_ATMOSPHERE_ID").as_bytes());
                header[76..80].copy_from_slice(&preset.to_le_bytes());
                header[80..84].copy_from_slice(&orders.to_le_bytes());
                header[84..88].copy_from_slice(&15u32.to_le_bytes());
                header[88..92].copy_from_slice(&crc(&bytes).to_le_bytes());
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let partial = path.with_extension("bin.partial");
                use std::io::Write;
                let mut file = std::fs::File::create(&partial)?;
                file.write_all(&header)?;
                file.write_all(&bytes)?;
                file.sync_all()?;
                std::fs::rename(partial, path)?;
                Ok(())
            })();
            result
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn crc_reference() {
        assert_eq!(super::crc(b"123456789"), 0xcbf43926);
    }
}
