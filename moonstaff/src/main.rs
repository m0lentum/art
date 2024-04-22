mod particles;
use particles::Particle;

use rand::Rng;
use starframe as sf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // original resolution the picture was painted at
    // with a bit of height cropped off to make space for animation
    let native_res = (6080., 3820.);
    let res_scale = 0.2;

    let window = sf::winit::window::WindowBuilder::new()
        .with_title("moonstaff")
        .with_inner_size(sf::winit::dpi::LogicalSize {
            width: res_scale * native_res.0,
            height: res_scale * native_res.1,
        });

    sf::Game::run::<State>(sf::GameParams {
        window,
        fps: 60,
        ..Default::default()
    })?;

    Ok(())
}

pub const MOON_POS: sf::Vec3 = sf::Vec3::new(0.2, 0.084, 30.);
pub const MOON_RADIUS: f32 = 0.28;

pub struct State {
    camera: sf::Camera,
    mesh_renderer: sf::MeshRenderer,
    line_renderer: sf::LineRenderer,

    particles: Vec<Particle>,
}

impl sf::GameState for State {
    fn init(game: &mut sf::Game) -> Self {
        game.graphics
            .load_gltf("moonstaff/moonstaff.glb")
            .expect("Failed to load 3D assets");

        // starframe doesn't support automatically spawning the contents of a gltf file yet,
        // and I can't be bothered to implement that right now,
        // so spawn all the stuff manually
        for (name, pos) in [
            ("moonstaff.sky", [0., 0., 50.]),
            ("moonstaff.char", [-0.25419, -0.12275, 0.]),
            ("moonstaff.cliff", [-0.354709, -0.384041, 0.02]),
            // TODO: these animated clouds should be placed at (0, 0),
            // but instead they need to be offset by the negative of the root bone's position
            // to show up in the right spot.
            // this is a Starframe bug, figure it out
            ("moonstaff.clouds_back", [0.6932588, 0.17770857, 30.]),
            ("moonstaff.clouds_mid", [0.6762213, 0.29390264, 30.]),
            ("moonstaff.clouds_front", [-0.65202147, 0.449862, 30.]),
            ("moonstaff.staffbg", [0.012096, 0.095921, 0.1]),
            ("moonstaff.staffmoon", [0.009397, 0.093279, 0.05]),
        ] {
            game.world.spawn((
                sf::Pose::new(sf::Vec2::new(pos[0], pos[1]), sf::Angle::default())
                    .with_depth(pos[2]),
                game.graphics.get_mesh_id(name).unwrap(),
            ));
        }

        for anim_name in [
            "moonstaff.clouds_front_sway",
            "moonstaff.clouds_mid_sway",
            "moonstaff.clouds_back_sway",
        ] {
            game.graphics.insert_animator(sf::Animator::new(
                game.graphics.get_animation_id(anim_name).unwrap(),
            ));
        }

        let mut camera = sf::Camera::new();
        camera.view_width = 1.56;
        camera.view_height = 0.96;

        Self {
            camera,
            mesh_renderer: sf::MeshRenderer::new(game),
            line_renderer: sf::LineRenderer::new(game),
            particles: Vec::new(),
        }
    }

    fn tick(&mut self, game: &mut sf::Game) -> Option<()> {
        if game.input.button(sf::Key::Q.into()) {
            return None;
        }

        let mut rng = rand::thread_rng();

        // spawn particles on mouse click
        if game.input.button(sf::MouseButton::Left.into()) {
            let xy = self
                .camera
                .point_screen_to_world(game.input.cursor_position());
            let pos = sf::Vec3::new(xy.x, xy.y, 30.);
            let particle = Particle::new(pos);
            self.particles.push(particle);
        }

        // also spawn random particles
        if rng.gen_bool(0.05) {
            // generate a uniform distribution in a circle;
            // see https://stackoverflow.com/questions/5837572/generate-a-random-point-within-a-circle-uniformly
            let radius = MOON_RADIUS * rng.gen::<f32>().sqrt();
            let angle = rng.gen::<f32>() * std::f32::consts::TAU;
            let pos = sf::Vec3::new(
                MOON_POS.x + radius * angle.cos(),
                MOON_POS.y + radius * angle.sin(),
                MOON_POS.z,
            );
            self.particles.push(Particle::new(pos));
        }

        // simulate particles
        for particle in &mut self.particles {
            particle.tick(game.dt_fixed as f32);
        }
        Particle::remove_completed(&mut self.particles);

        Some(())
    }

    fn draw(&mut self, game: &mut sf::Game, dt: f32) {
        self.camera.upload();
        // slow down the animation in code here
        // because I can't be bothered to adjust it in blender
        game.graphics.update_animations(0.5 * dt);

        let mut deferred = game.renderer.begin_frame();
        {
            let mut pass = deferred.pass();
            self.mesh_renderer
                .draw(&mut pass, &mut game.graphics, &mut game.world, &self.camera);
        }

        let mut shade = deferred.shade();
        shade.set_fullbright();
        shade.extend_point_lights(self.particles.iter().map(|p| sf::PointLight {
            position: p.position,
            color: p.light_color,
            radius: 3.,
            ..Default::default()
        }));
        let mut forward = shade.finish(&self.camera);

        // particle trails
        {
            let mut pass = forward.pass();
            for particle in &self.particles {
                self.line_renderer.draw(
                    &mut pass,
                    &self.camera,
                    &game.graphics,
                    &particle.trail_strip,
                );
            }
        }
    }
}
