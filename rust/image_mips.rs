use anyhow::{Result, bail};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageLevel {
    pub width: u32,
    pub height: u32,
    pub offset: usize,
}
#[derive(Clone, Debug, Default)]
pub struct ImageMipChain {
    pub pixels: Vec<u8>,
    pub levels: Vec<ImageLevel>,
}

/// RGBA8 downsampling averages RGB in linear light and alpha linearly.
pub fn make_srgb_mips(mut width: u32, mut height: u32, rgba: &[u8]) -> Result<ImageMipChain> {
    if width == 0
        || height == 0
        || width > 16384
        || height > 8192
        || rgba.len() != width as usize * height as usize * 4
    {
        bail!("Invalid RGBA8 background dimensions");
    }
    let linear: [f64; 256] = std::array::from_fn(|i| {
        let v = i as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    });
    let mut chain = ImageMipChain::default();
    let mut bytes = 0;
    loop {
        chain.levels.push(ImageLevel {
            width,
            height,
            offset: bytes,
        });
        bytes += width as usize * height as usize * 4;
        if width == 1 && height == 1 {
            break;
        }
        width = 1.max(width / 2);
        height = 1.max(height / 2);
    }
    chain.pixels.resize(bytes, 0);
    chain.pixels[..rgba.len()].copy_from_slice(rgba);
    for level in 1..chain.levels.len() {
        let (source, target) = (chain.levels[level - 1], chain.levels[level]);
        let (scale_x, scale_y) = (
            source.width as f64 / target.width as f64,
            source.height as f64 / target.height as f64,
        );
        for y in 0..target.height {
            let (top, bottom) = (y as f64 * scale_y, (y + 1) as f64 * scale_y);
            for x in 0..target.width {
                let (left, right) = (x as f64 * scale_x, (x + 1) as f64 * scale_x);
                let mut sum = [0.0; 4];
                for sy in top as u32..source.height.min(bottom.ceil() as u32) {
                    let wy = bottom.min(sy as f64 + 1.0) - top.max(sy as f64);
                    for sx in left as u32..source.width.min(right.ceil() as u32) {
                        let weight = wy * (right.min(sx as f64 + 1.0) - left.max(sx as f64));
                        let offset =
                            source.offset + (sy as usize * source.width as usize + sx as usize) * 4;
                        for (c, v) in sum.iter_mut().enumerate().take(3) {
                            *v += linear[chain.pixels[offset + c] as usize] * weight;
                        }
                        sum[3] += chain.pixels[offset + 3] as f64 / 255.0 * weight;
                    }
                }
                let offset = target.offset + (y as usize * target.width as usize + x as usize) * 4;
                for (c, sum) in sum.iter().enumerate() {
                    let mut value = sum / (scale_x * scale_y);
                    if c < 3 {
                        value = if value <= 0.0031308 {
                            value * 12.92
                        } else {
                            1.055 * value.powf(1.0 / 2.4) - 0.055
                        };
                    }
                    chain.pixels[offset + c] = (value * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
    Ok(chain)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conserve_linear_light_and_odd_edges() {
        let checker = [
            0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0,
        ];
        let mixed = make_srgb_mips(2, 2, &checker).unwrap();
        assert_eq!(&mixed.pixels[16..], &[188, 188, 188, 128]);
        assert_eq!(&mixed.pixels[..16], &checker);
        let edge = [0, 0, 0, 255, 0, 0, 0, 255, 255, 0, 0, 255];
        assert_eq!(
            &make_srgb_mips(3, 1, &edge).unwrap().pixels[12..],
            &[156, 0, 0, 255]
        );
        assert!(make_srgb_mips(2, 2, &[]).is_err());
    }
    #[test]
    fn rectangular_chain_is_contiguous_and_uniform() {
        for (w, h) in [(1, 17), (17, 1), (125, 63), (250, 125), (16384, 2)] {
            let pixels = [47, 119, 201, 255].repeat(w * h);
            let c = make_srgb_mips(w as u32, h as u32, &pixels).unwrap();
            let mut end = 0;
            for l in &c.levels {
                assert_eq!(l.offset, end);
                end += l.width as usize * l.height as usize * 4;
            }
            assert_eq!(end, c.pixels.len());
            assert!(c.pixels.chunks_exact(4).all(|c| c == [47, 119, 201, 255]));
            assert_eq!(
                (
                    c.levels.last().unwrap().width,
                    c.levels.last().unwrap().height
                ),
                (1, 1)
            );
        }
    }
}
