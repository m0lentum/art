use starframe as sf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = sf::winit::window::WindowBuilder::new()
        .with_title("heather")
        .with_inner_size(sf::winit::dpi::LogicalSize {
            width: 1920.,
            height: 1080.,
        });

    sf::Game::run::<State>(sf::GameParams {
        window,
        graphics: sf::GraphicsConfig {
            fps: 60,
            use_vsync: false,
            lighting_quality: sf::LightingQualityConfig::default(),
        },
        ..Default::default()
    })?;

    Ok(())
}

pub struct State {
    camera: sf::Camera,
    env_map: sf::EnvironmentMap,
}

impl sf::GameState for State {
    fn init(game: &mut sf::Game) -> Self {
        game.graphics
            .load_gltf("heather/heather.glb")
            .expect("Failed to load 3D assets");

        game.world.spawn((
            sf::Pose::default(),
            game.graphics.get_mesh_id("heather.char").unwrap(),
        ));

        let camera = sf::Camera::new();

        let env_map = sf::EnvironmentMap::preset_night();

        Self { camera, env_map }
    }

    fn tick(&mut self, game: &mut sf::Game) -> Option<()> {
        Some(())
    }

    fn draw(&mut self, game: &mut sf::Game, dt: f32) {
        self.camera.upload();
        game.graphics.update_animations(dt);
        game.renderer.set_environment_map(&self.env_map);

        let mut frame = game.renderer.begin_frame();
        frame.set_clear_color([0.00802, 0.0137, 0.02732, 1.]);
        frame.draw_meshes(&mut game.graphics, &mut game.world, &self.camera);
    }
}
