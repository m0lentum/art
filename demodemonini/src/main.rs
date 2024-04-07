use instant::Instant;

use wgpu::util::DeviceExt;
use winit::{
    event::{Event, VirtualKeyCode, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

//

mod pipelines;
use pipelines::{load_png_texture, PostprocessPipeline, TexturePipeline, VertexColorPipeline};

mod fire;
use fire::Fire;

mod triangle_grid;
use triangle_grid::TriangleGrid;

// constants for quick globally accessible configuration

#[cfg(not(target_arch = "wasm32"))]
const SWAPCHAIN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
#[cfg(target_arch = "wasm32")]
const SWAPCHAIN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
// MSAA doesn't work on webgl, and it doesn't really make a difference in this anyway,
// so just disable it everywhere
const MSAA_SAMPLES: u32 = 1;
const MULTISAMPLE_STATE: wgpu::MultisampleState = wgpu::MultisampleState {
    count: MSAA_SAMPLES,
    mask: !0,
    alpha_to_coverage_enabled: false,
};

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    futures::executor::block_on(main_async())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    wasm_bindgen_futures::spawn_local(main_async_unwrap());
}

#[cfg(target_arch = "wasm32")]
async fn main_async_unwrap() {
    main_async().await.unwrap();
}

async fn main_async() -> anyhow::Result<()> {
    //
    // winit & wgpu setup
    //

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("demodemonini")
        .with_inner_size(winit::dpi::LogicalSize {
            width: 1080 * 4 / 3,
            height: 1080,
        })
        .build(&event_loop)?;
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init().expect("Failed to initialize console logger");
        use winit::platform::web::WindowExtWebSys;
        let canvas = web_sys::Element::from(window.canvas());
        web_sys::window()
            .and_then(|win| win.document())
            // made-up convention for putting the game in a specific spot in the DOM
            .and_then(|doc| match doc.get_element_by_id("wgpu-canvas") {
                Some(parent) => parent.append_child(&canvas).ok(),
                None => doc.body().and_then(|body| body.append_child(&canvas).ok()),
            })
            .expect("couldn't append canvas to document body")
    };

    let instance = wgpu::Instance::default();
    let surface = unsafe { instance.create_surface(&window)? };

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await
        .ok_or(anyhow::anyhow!("Adapter request failed"))?;

    #[cfg(not(target_arch = "wasm32"))]
    let limits = wgpu::Limits::default();
    #[cfg(target_arch = "wasm32")]
    let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                limits,
                label: None,
            },
            None,
        )
        .await
        // for some reason this map is needed on wasm
        .map_err(|e| anyhow::anyhow!("Failed to get device: {e}"))?;

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

    fn create_screen_texture(
        device: &wgpu::Device,
        window_size: winit::dpi::PhysicalSize<u32>,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: window_size.width,
                height: window_size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SWAPCHAIN_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    }

    // main image is draw into a gbuffer for postprocessing
    let mut gbuffer = create_screen_texture(&device, initial_window_size);

    //
    // pipelines and textures
    //

    let color_pl = VertexColorPipeline::new(&device);
    let mut background_grid = TriangleGrid::generate(&device);

    let tex_pl = TexturePipeline::new(&device);
    let characters_tex = load_png_texture(&device, &queue, include_bytes!("../characters.png"))?;
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
    let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let fire_bind_group = tex_pl.create_bind_group(&device, &fire_tex_view, &nearest_sampler);

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

    // reflection squished to look in perspective and smoothed by a filtering sampler
    let fire_reflection_bind_group =
        tex_pl.create_bind_group(&device, &fire_tex_view, &filtering_sampler);

    let refl_bottom_y = fire_base_y - 0.4 * fire_height;
    let fire_reflection_verts = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&[
            // position         tex_coords
            [[-1., fire_base_y], [0., 1.]],
            [[1., fire_base_y], [1., 1.]],
            [[1., refl_bottom_y], [1., 0.]],
            [[-1., fire_base_y], [0., 1.]],
            [[1., refl_bottom_y], [1., 0.]],
            [[-1., refl_bottom_y], [0., 0.]],
        ]),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let fire_dt = 1. / 20.;

    let postprocess_pl = PostprocessPipeline::new(&device);

    //
    // run event loop
    //

    // interactive controls to toggle parts of the picture, just for fun
    let mut draw_characters = true;
    let mut draw_fire = true;
    let mut draw_postprocess = true;

    // frame timing for the fire simulation
    let mut frame_start_t = Instant::now();
    let mut time_in_frame = 0.;
    // global time for time-dependent effects
    let start_t = Instant::now();
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
                let gbuf_view = gbuffer.create_view(&wgpu::TextureViewDescriptor::default());
                let gbuf_bind_group =
                    postprocess_pl.create_bind_group(&device, &gbuf_view, &filtering_sampler);
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: if draw_postprocess {
                            &gbuf_view
                        } else {
                            &surface_view
                        },
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });

                // draw

                let t = start_t.elapsed().as_secs_f32();
                postprocess_pl.upload_time(&queue, t);

                if fire_updated {
                    fire.write_texture(&queue, &fire_tex);
                }

                background_grid.update(&queue, t);

                pass.set_pipeline(&color_pl.pipeline);
                pass.set_vertex_buffer(0, background_grid.vertex_buf.slice(..));
                pass.draw(0..background_grid.vertex_count, 0..1);

                pass.set_pipeline(&tex_pl.pipeline);

                if draw_fire {
                    pass.set_bind_group(0, &fire_bind_group, &[]);
                    pass.set_vertex_buffer(0, fire_verts.slice(..));
                    pass.draw(0..6, 0..1);

                    pass.set_bind_group(0, &fire_reflection_bind_group, &[]);
                    pass.set_vertex_buffer(0, fire_reflection_verts.slice(..));
                    pass.draw(0..6, 0..1);
                }

                if draw_characters {
                    pass.set_bind_group(0, &characters_bind_group, &[]);
                    pass.set_vertex_buffer(0, characters_verts.slice(..));
                    pass.draw(0..6, 0..1);
                }

                drop(pass);

                // postprocessing pass

                if draw_postprocess {
                    let mut postprocess_pass =
                        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

                    postprocess_pass.set_pipeline(&postprocess_pl.pipeline);
                    postprocess_pass.set_bind_group(0, &gbuf_bind_group, &[]);
                    postprocess_pass.set_bind_group(1, &postprocess_pl.time_bind_group, &[]);
                    postprocess_pass.draw(0..3, 0..1);
                }

                // finalize

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
                    gbuffer = create_screen_texture(&device, new_size);
                }
                WindowEvent::KeyboardInput {
                    input:
                        winit::event::KeyboardInput {
                            state: winit::event::ElementState::Pressed,
                            virtual_keycode: Some(key),
                            ..
                        },
                    ..
                } => {
                    use VirtualKeyCode::*;
                    match key {
                        Q => {
                            control_flow.set_exit();
                        }
                        F => {
                            draw_fire = !draw_fire;
                        }
                        C => {
                            draw_characters = !draw_characters;
                        }
                        P => {
                            draw_postprocess = !draw_postprocess;
                        }
                        _ => {}
                    }
                }
                _ => {}
            },
            _ => {}
        };
    });
}
