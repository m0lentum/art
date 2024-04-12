use std::collections::VecDeque;

use rand::Rng;
use starframe as sf;

// particles gravitate towards the staff the character is holding,
// which is at roughly this position in the world
const TARGET_POS: sf::Vec3 = sf::Vec3::new(0.012096, 0.095921, -0.1);
const GRAVITY_STRENGTH: f32 = 10000.;
const MAX_SPEED: f32 = 10.;
const ORBIT_DISTANCE: f32 = 0.25;
const ORBIT_DROP_SPEED: f32 = 0.2;
const ORBIT_TWIST_SPEED: f32 = 0.05;
const ORBIT_DAMPING: f32 = 0.98;
const DESTROY_ORBIT_TIME: f32 = 1.5;

pub struct Particle {
    pub position: sf::Vec3,
    pub velocity: sf::Vec3,
    pub light_color: [f32; 3],
    pub trail_width: f32,
    pub trail_length: usize,
    pub trail_positions: VecDeque<sf::Vec3>,
    pub trail_strip: sf::LineStrip,
    pub orbit_time: f32,
}

impl Particle {
    pub fn new(position: sf::Vec3) -> Self {
        let mut rng = rand::thread_rng();
        let trail_length = rng.gen_range(80..160);
        let trail_width = rng.gen_range(0.005..0.015);

        let mut trail_positions = VecDeque::with_capacity(trail_length);
        trail_positions.push_front(position);

        // starting velocity that pushes away from the moon's center for a nice curving effect
        let dist = position - super::MOON_POS;
        let normal_vel = rng.gen_range(0.1..0.3);
        let velocity = normal_vel * dist + sf::Vec3::new(0., 0., -5.);

        let light_color = [0.2, 0.2, rng.gen_range(0.3..0.4)];
        Self {
            position,
            velocity,
            light_color,
            trail_width,
            trail_length,
            trail_positions,
            // write some nonsense placeholder points, we'll overwrite this before we draw
            trail_strip: sf::LineStrip::new(
                &[
                    Self::point_to_line_vertex(position, 1.),
                    Self::point_to_line_vertex(position, 1.),
                ],
                None,
            ),
            orbit_time: 0.,
        }
    }

    /// Generate a line vertex with the given position
    /// and a width modulated by the z coordinate
    /// to create an illusion of depth.
    fn point_to_line_vertex(p: sf::Vec3, base_width: f32) -> sf::LineVertex {
        sf::LineVertex {
            position: p,
            width: base_width / f32::max(0.5, (p.z + 1.) / 2.),
        }
    }

    /// Apply gravity, move the particle, update the trail
    pub fn tick(&mut self, dt: f32) {
        // apply gravity as per newton's law F = Gm_1m_2/r^2
        // (disregarding masses and going directly to acceleration;
        // GRAVITY_CONSTANT = G * m_1)
        let dist = self.position - TARGET_POS;
        let dist_sq = dist.mag_sq();
        if self.orbit_time == 0. && dist_sq > ORBIT_DISTANCE.powi(2) {
            // falling
            let grav_accel = GRAVITY_STRENGTH / dist_sq;
            self.velocity -= dt.powi(2) * grav_accel * dist.normalized();
        } else {
            // orbiting
            let orbit_normal = dist.normalized();
            // cancel velocity away from the target
            self.velocity -= orbit_normal * self.velocity.dot(orbit_normal);
            // fall towards the target
            self.velocity -= ORBIT_DROP_SPEED * orbit_normal;
            // also twist a little to the side and apply damping
            let side_dir = orbit_normal.cross(self.velocity).normalized();
            self.velocity += ORBIT_TWIST_SPEED * side_dir;
            self.velocity *= ORBIT_DAMPING;

            self.orbit_time += dt;
        }

        let speed = self.velocity.mag();
        if speed > MAX_SPEED {
            self.velocity *= MAX_SPEED / speed;
        }

        // move the particle and update the trail
        self.position += dt * self.velocity;

        // if we're past the time to destroy,
        // don't push a new position, pop a couple off the back to gradually remove the trail
        if self.orbit_time >= DESTROY_ORBIT_TIME {
            self.trail_positions.pop_back();
            self.trail_positions.pop_back();
        } else {
            // normal handling of the trail
            if self.trail_positions.len() >= self.trail_length {
                self.trail_positions.pop_back();
            }
            self.trail_positions.push_front(self.position);
        }

        if self.trail_positions.len() >= 2 {
            self.update_trail();
        }
    }

    /// Push trail vertices to the GPU.
    fn update_trail(&mut self) {
        let vertices: Vec<sf::LineVertex> = self
            .trail_positions
            .iter()
            .map(|p| Self::point_to_line_vertex(*p, self.trail_width))
            .collect();
        self.trail_strip.overwrite(&vertices);
    }

    /// Destroy particles that have reached the staff and had their trails fully consumed.
    pub fn remove_completed(particles: &mut Vec<Self>) {
        particles.retain(|p| !p.trail_positions.is_empty());
    }
}
