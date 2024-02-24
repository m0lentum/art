use enterpolation::{linear::Linear, Curve};
use itertools::{iproduct, izip};
use lazy_static::lazy_static;
use palette::{IntoColor, LinSrgba, Srgba};
use rand::Rng;

/// "Doom fire"
/// based on this: https://fabiensanglard.net/doom_fire_psx/
#[derive(Clone, Debug)]
pub struct Fire {
    pub width: usize,
    pub height: usize,
    // average amount of cooling per row propagated upwards
    pub cooling_rate: f32,
    // heat value in the range [0, 1] for every pixel in the fire
    heat_buf: Vec<f32>,
}

// generate a lookup table for the color palette
const PALETTE_SIZE: usize = 32;
lazy_static! {
    static ref PALETTE_LUT: [[u8; 4]; PALETTE_SIZE] = {
        let curve = Linear::builder()
            .elements([
                Srgba::new(0., 0., 0., 0.).into_linear(),
                Srgba::new(0.250, 0.015, 0., 0.8).into_linear(),
                Srgba::new(0.450, 0.170, 0.070, 1.).into_linear(),
                Srgba::new(0.850, 0.506, 0.161, 1.).into_linear(),
                Srgba::new(0.960, 0.812, 0.154, 1.).into_linear(),
                Srgba::new(1., 1., 1., 1.).into_linear(),
            ])
            .knots([0., 0.3, 0.5, 0.8, 0.95, 1.])
            .build()
            .unwrap();
        let vals = curve.take(PALETTE_SIZE);
        let mut lut = [[0; 4]; PALETTE_SIZE];
        for (color, lut_val) in izip!(vals, lut.iter_mut()) {
            let c_lin: LinSrgba = color.into_color();
            let as_u8 = |channel: f32| (u8::MAX as f32 * channel).round() as u8;
            *lut_val = [
                as_u8(c_lin.red),
                as_u8(c_lin.green),
                as_u8(c_lin.blue),
                as_u8(c_lin.alpha),
            ];
        }
        lut
    };
}

impl Fire {
    pub fn new(width: usize, height: usize, cooling_rate: f32) -> Self {
        let cell_count = width * height;
        let mut heat_buf = vec![0.; cell_count];
        // set the bottom row to full heat
        for cell in &mut heat_buf[(cell_count - width)..cell_count] {
            *cell = 1.;
        }

        Self {
            width,
            height,
            cooling_rate,
            heat_buf,
        }
    }

    pub fn propagate(&mut self) {
        // random jitter in the amount of cooling
        let cooling_variance = self.cooling_rate * 0.9;
        let cooling_range =
            self.cooling_rate - cooling_variance..=self.cooling_rate + cooling_variance;
        let mut rng = rand::thread_rng();

        for (x, y) in iproduct!(0..self.width, 1..self.height) {
            let source_idx = y * self.width + x;
            let target_idx = {
                let above = source_idx - self.width;
                let wind: isize = rng.gen_range(-1..=2);
                (above as isize + wind).max(0) as usize
            };
            let cooling = rng.gen_range(cooling_range.clone());
            self.heat_buf[target_idx] = (self.heat_buf[source_idx] - cooling).max(0.);
        }
    }

    pub fn create_texture(&self, device: &wgpu::Device) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: self.width as u32,
                height: self.height as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    pub fn write_texture(&self, queue: &wgpu::Queue, texture: &wgpu::Texture) {
        // TODO: color with a palette
        let color_data: Vec<[u8; 4]> = self
            .heat_buf
            .iter()
            .map(|&temp| {
                let lut_idx = ((temp * PALETTE_SIZE as f32) as usize).min(PALETTE_SIZE - 1);
                PALETTE_LUT[lut_idx]
            })
            .collect();

        queue.write_texture(
            texture.as_image_copy(),
            bytemuck::cast_slice(&color_data),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.width as u32),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: self.width as u32,
                height: self.height as u32,
                depth_or_array_layers: 1,
            },
        );
    }
}
