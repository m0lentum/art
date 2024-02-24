use itertools::iproduct;
use rand::Rng;

/// "Doom fire"
/// based on this: https://fabiensanglard.net/doom_fire_psx/
#[derive(Clone, Debug)]
pub struct Fire {
    pub width: usize,
    pub height: usize,
    // heat value in the range [0, 1] for every pixel in the fire
    heat_buf: Vec<f32>,
    // average amount of cooling per row propagated upwards
    pub cooling_rate: f32,
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
            heat_buf,
            cooling_rate,
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
                let temp_u8 = (temp * 255.0).round() as u8;
                [temp_u8, 0, 0, 255]
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
