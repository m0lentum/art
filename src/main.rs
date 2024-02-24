use std::time::Instant;

use anyhow::anyhow;
use futures::executor::block_on;
use wgpu::util::DeviceExt;
use winit::{
    event::{Event, VirtualKeyCode, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

//

mod pipelines;
use pipelines::{load_png_texture, TexturePipeline};

mod fire;
use fire::Fire;

// constants for quick globally accessible configuration

const SWAPCHAIN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
// TODO: actually set up msaa
const MSAA_SAMPLES: u32 = 1;
const MULTISAMPLE_STATE: wgpu::MultisampleState = wgpu::MultisampleState {
    count: MSAA_SAMPLES,
    mask: !0,
    alpha_to_coverage_enabled: false,
};

fn main() -> anyhow::Result<()> {
    //
    // winit & wgpu setup
    //

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("demodemons")
        .with_inner_size(winit::dpi::LogicalSize {
            width: 1600,
            height: 1200,
        })
        .build(&event_loop)?;

    let instance = wgpu::Instance::default();
    let surface = unsafe { instance.create_surface(&window)? };

    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .ok_or(anyhow!("Failed to get adapter"))?;

    let (device, queue) = block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::default(),
            label: None,
        },
        None,
    ))?;

    let initial_window_size = window.inner_size();

    let swapchain_capabilities = surface.get_capabilities(&adapter);

    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: SWAPCHAIN_FORMAT,
        width: initial_window_size.width,
        height: initial_window_size.height,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: swapchain_capabilities.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &surface_config);

    //
    // pipelines and textures
    //

    let tex_pl = TexturePipeline::new(&device);
    let characters_tex = load_png_texture(&device, &queue, "characters.png")?;
    let characters_tex_view = characters_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let filtering_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let characters_bind_group =
        tex_pl.create_bind_group(&device, &characters_tex_view, &filtering_sampler);

    // fullscreen quad for the main image
    let characters_verts = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&[
            // position    tex_coords
            [[-1f32, -1.], [0., 1.]],
            [[1., -1.], [1., 1.]],
            [[1., 1.], [1., 0.]],
            [[-1., -1.], [0., 1.]],
            [[1., 1.], [1., 0.]],
            [[-1., 1.], [0., 0.]],
        ]),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let mut fire = Fire::new(250, 150, 1. / 120.);
    let fire_tex = fire.create_texture(&device);
    let fire_tex_view = fire_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let closest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let fire_bind_group = tex_pl.create_bind_group(&device, &fire_tex_view, &closest_sampler);

    // rectangular quad for the fire
    let fire_base_y = -0.5;
    // height that makes square pixels at 4:3 aspect ratio
    let fire_height = (2. / fire.width as f32) * fire.height as f32 * 4. / 3.;
    let fire_top_y = fire_base_y + fire_height;
    let fire_verts = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&[
            // position         tex_coords
            [[-1., fire_base_y], [0., 1.]],
            [[1., fire_base_y], [1., 1.]],
            [[1., fire_top_y], [1., 0.]],
            [[-1., fire_base_y], [0., 1.]],
            [[1., fire_top_y], [1., 0.]],
            [[-1., fire_top_y], [0., 0.]],
        ]),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let fire_dt = 1. / 20.;

    //
    // run event loop
    //

    // frame timing for the fire simulation
    let mut frame_start_t = Instant::now();
    let mut time_in_frame = 0.;
    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();
        match event {
            //
            // render loop
            //
            Event::MainEventsCleared => {
                // simulate fire

                let since_last_draw = frame_start_t.elapsed().as_secs_f64();
                time_in_frame += since_last_draw;
                let mut fire_updated = false;
                // limit maximum steps per frame to avoid spiral of death
                for _ in 0..4 {
                    if time_in_frame < fire_dt {
                        break;
                    }
                    fire.propagate();
                    fire_updated = true;
                    time_in_frame -= fire_dt;
                }

                frame_start_t = Instant::now();

                // setup

                let surface_tex = surface
                    .get_current_texture()
                    .expect("Failed to get swapchain texture");
                let surface_view = surface_tex
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });

                // draw

                if fire_updated {
                    fire.write_texture(&queue, &fire_tex);
                }

                pass.set_pipeline(&tex_pl.pipeline);

                pass.set_bind_group(0, &fire_bind_group, &[]);
                pass.set_vertex_buffer(0, fire_verts.slice(..));
                pass.draw(0..6, 0..1);

                pass.set_bind_group(0, &characters_bind_group, &[]);
                pass.set_vertex_buffer(0, characters_verts.slice(..));
                pass.draw(0..6, 0..1);

                // finalize

                drop(pass);
                queue.submit(Some(encoder.finish()));
                surface_tex.present();
            }
            //
            // handle window events
            //
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    control_flow.set_exit();
                }
                WindowEvent::Resized(new_size) => {
                    surface_config.width = new_size.width;
                    surface_config.height = new_size.height;
                    surface.configure(&device, &surface_config);
                }
                WindowEvent::KeyboardInput {
                    input:
                        winit::event::KeyboardInput {
                            state: winit::event::ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Q),
                            ..
                        },
                    ..
                } => {
                    control_flow.set_exit();
                }
                _ => {}
            },
            _ => {}
        };
    });
}
