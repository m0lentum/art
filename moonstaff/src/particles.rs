use std::collections::VecDeque;

use rand::Rng;
use starframe as sf;

// particles gravitate towards the staff the character is holding,
// which is at roughly this position in the world
const TARGET_POS: sf::Vec3 = sf::Vec3::new(0.012096, 0.095921, -0.1);
const GRAVITY_STRENGTH: f32 = 10000.;
const MAX_SPEED: f32 = 10.;
const ORBIT_DISTANCE: f32 = 0.25;
const ORBIT_TIME: f32 = 0.5;
const ORBIT_PATH_SIZE: f32 = 0.05;

pub struct Particle {
    pub position: sf::Vec3,
    pub velocity: sf::Vec3,
    pub target: sf::Vec3,
    pub light_color: [f32; 3],
    pub trail_width: f32,
    pub trail_length: usize,
    pub trail_points: VecDeque<sf::LineVertex>,
    pub trail_strip: sf::LineStrip,
    pub end: Option<EndPath>,
}

// when the particles get close enough they change
// from gravity to a bezier curve ending in the staff
pub struct EndPath {
    start: sf::Vec3,
    control1: sf::Vec3,
    control2: sf::Vec3,
    t: f32,
}

impl Particle {
    pub fn new(position: sf::Vec3, material: sf::MaterialId) -> Self {
        let mut rng = rand::thread_rng();
        let trail_length = rng.gen_range(80..160);
        let trail_width = rng.gen_range(0.005..0.015);

        let mut trail_positions = VecDeque::with_capacity(trail_length);
        let first_point = Self::point_to_line_vertex(position, trail_width);
        trail_positions.push_front(first_point);

        // starting velocity that pushes away from the moon's center for a nice curving effect
        let dist = position - super::MOON_POS;
        let normal_vel = rng.gen_range(0.1..0.3);
        let velocity = normal_vel * dist + sf::Vec3::new(0., 0., -5.);

        // offset target from the staff position by a random amount
        let to_angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let to_dist = rng.gen_range(0.02..0.1);
        let target_offset = to_dist * sf::Vec3::new(to_angle.cos(), to_angle.sin(), 0.);
        let target = TARGET_POS + target_offset;

        let light_color = [0.4, 0.4, rng.gen_range(0.6..0.8)];
        Self {
            position,
            velocity,
            target,
            light_color,
            trail_width,
            trail_length,
            trail_points: trail_positions,
            // write some placeholder points because lines need to have at least two,
            // we'll overwrite this before we draw
            trail_strip: sf::LineStrip::new(&[first_point, first_point], Some(material)),
            end: None,
        }
    }

    /// Generate a line vertex with the given position
    /// and a width modulated by the z coordinate
    /// to create an illusion of depth.
    pub fn point_to_line_vertex(p: sf::Vec3, base_width: f32) -> sf::LineVertex {
        sf::LineVertex {
            position: p,
            width: base_width / f32::max(1., (p.z + 2.) / 2.),
        }
    }

    /// Apply gravity, move the particle, update the trail
    pub fn tick(&mut self, dt: f32) {
        if let Some(end) = &mut self.end {
            if end.t < 1. {
                end.t += dt / ORBIT_TIME;
                // bezier curve as repeated linear interpolation
                let lerp = |a: sf::Vec3, b: sf::Vec3| -> sf::Vec3 { a + end.t * (b - a) };
                self.position = lerp(
                    lerp(
                        lerp(end.start, end.control1),
                        lerp(end.control1, end.control2),
                    ),
                    lerp(
                        lerp(end.control1, end.control2),
                        lerp(end.control2, TARGET_POS),
                    ),
                );
                self.trail_points.pop_back();
                self.trail_points.push_front(sf::LineVertex {
                    position: self.position,
                    width: self.trail_width * (1. - end.t.powi(4)).max(0.05),
                });
            } else {
                // we've reached the end, remove particles (at an accelerated rate) until the trail is gone
                self.trail_points.pop_back();
                self.trail_points.pop_back();
            }
        } else {
            // apply gravity as per newton's law F = Gm_1m_2/r^2
            // (disregarding masses and going directly to acceleration;
            // GRAVITY_CONSTANT = G * m_1)
            let dist = self.position - self.target;
            let dist_sq = dist.mag_sq();
            if dist_sq > ORBIT_DISTANCE.powi(2) {
                // falling
                let grav_accel = GRAVITY_STRENGTH / dist_sq;
                self.velocity -= dt.powi(2) * grav_accel * dist.normalized();

                let speed = self.velocity.mag();
                if speed > MAX_SPEED {
                    self.velocity *= MAX_SPEED / speed;
                }

                self.position += dt * self.velocity;

                if self.trail_points.len() >= self.trail_length {
                    self.trail_points.pop_back();
                }
                self.trail_points
                    .push_front(Self::point_to_line_vertex(self.position, self.trail_width));
            } else {
                // reached the end zone, generate bezier path to the end
                let vel_scaled = ORBIT_PATH_SIZE * self.velocity;
                let control1 = self.position + vel_scaled;
                // always turn towards the target to avoid loops
                // (those don't look good with the current line rendering impl)
                let dir_to_target = TARGET_POS - control1;
                let vel_turned = sf::Vec3::new(-vel_scaled.y, vel_scaled.x, 0.);
                let c2_offset = if dir_to_target.dot(vel_turned) > 0. {
                    vel_turned
                } else {
                    -vel_turned
                };
                let control2 = control1 + c2_offset;
                self.end = Some(EndPath {
                    start: self.position,
                    control1,
                    control2,
                    t: 0.,
                });
            }
        }

        if self.trail_points.len() >= 2 {
            self.update_trail();
        }
    }

    /// Push trail vertices to the GPU.
    fn update_trail(&mut self) {
        let vertices: Vec<sf::LineVertex> = self.trail_points.iter().cloned().collect();
        self.trail_strip.overwrite(&vertices);
    }

    /// Destroy particles that have reached the staff and had their trails fully consumed.
    ///
    /// Returns the number of particles removed, used for driving the "charging" animation.
    pub fn remove_completed(particles: &mut Vec<Self>) -> usize {
        let len_before = particles.len();
        particles.retain(|p| !p.trail_points.is_empty());
        len_before - particles.len()
    }
}
