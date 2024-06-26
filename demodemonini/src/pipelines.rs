use std::borrow::Cow;

use wgpu::util::DeviceExt;

pub fn load_png_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bytes: &[u8],
) -> anyhow::Result<wgpu::Texture> {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)?;
    let image_bytes = &buf[..info.buffer_size()];

    Ok(device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: info.width,
                height: info.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        image_bytes,
    ))
}

pub struct TexturePipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl TexturePipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        let label = Some("texture");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "./shaders/textured.wgsl"
            ))),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 4 * 2 * 2,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // position
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        // texture coords
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 4 * 2,
                            shader_location: 1,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: super::SWAPCHAIN_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: super::MULTISAMPLE_STATE,
            multiview: None,
        });

        Self {
            pipeline,
            bind_group_layout,
        }
    }

    /// Create a bind group with a texture and a sampler
    /// compatible with this pipeline.
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

pub struct VertexColorPipeline {
    pub pipeline: wgpu::RenderPipeline,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColoredVertex {
    pub pos: [f32; 2],
    pub col: [f32; 4],
}

impl VertexColorPipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        let label = Some("vertex colors");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "./shaders/vert_colors.wgsl"
            ))),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 2 * 4 + 4 * 4,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // position
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        // color
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 4 * 2,
                            shader_location: 1,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: super::SWAPCHAIN_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: super::MULTISAMPLE_STATE,
            multiview: None,
        });

        Self { pipeline }
    }
}

pub struct PostprocessPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub gbuf_bind_group_layout: wgpu::BindGroupLayout,
    pub time_buffer: wgpu::Buffer,
    pub time_bind_group: wgpu::BindGroup,
}

impl PostprocessPipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("postprocess"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                "./shaders/postprocess.wgsl"
            ))),
        });

        let gbuf_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("postprocess gbuffer binding"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let time_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("global time"),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            // on webgl, buffers must be 16 byte aligned
            size: 16,
            mapped_at_creation: false,
        });

        let time_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("postprocess uniforms"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        min_binding_size: wgpu::BufferSize::new(16),
                        has_dynamic_offset: false,
                    },
                    count: None,
                }],
            });

        let time_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("postprocess uniforms"),
            layout: &time_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(time_buffer.as_entire_buffer_binding()),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("postprocess"),
            bind_group_layouts: &[&gbuf_bind_group_layout, &time_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("postprocess"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: super::SWAPCHAIN_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Self {
            pipeline,
            gbuf_bind_group_layout,
            time_buffer,
            time_bind_group,
        }
    }

    pub fn upload_time(&self, queue: &wgpu::Queue, t: f32) {
        // pad to 16 bytes
        queue.write_buffer(&self.time_buffer, 0, bytemuck::cast_slice(&[t, 0., 0., 0.]));
    }

    /// Create a bind group with a texture and a sampler
    /// compatible with this pipeline.
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.gbuf_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}
