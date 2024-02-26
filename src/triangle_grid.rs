use enterpolation::{linear::Linear, Generator};
use itertools::chain;
use palette::{IntoColor, LinSrgba, Srgb};
use rand::Rng;
use std::{f32::consts::PI, ops::Range};

use super::pipelines::ColoredVertex;

pub struct TriangleGrid {
    points: Vec<Point>,
    pub vertex_buf: wgpu::Buffer,
    pub vertex_count: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Point {
    root_pos: [f32; 2],
    color: [f32; 4],
    // randomized parameters for a sine curve
    x_phase: f32,
    x_amplitude: f32,
    x_velocity: f32,
    y_phase: f32,
    y_amplitude: f32,
    y_velocity: f32,
}

// const parameters for easy tweaking since this doesn't need to be reusable
const POINTS_PER_ROW: usize = 16;
const ROWS: usize = 14;
const X_VARIANCE_COEF: f32 = 0.2;
const Y_VARIANCE_COEF: f32 = 0.1;
const X_AMPLITUDE_RANGE: Range<f32> = 0.015..0.025;
const X_VELOCITY_RANGE: Range<f32> = 0.1 * PI..0.5 * PI;
const Y_AMPLITUDE_RANGE: Range<f32> = 0.01..0.015;
const Y_VELOCITY_RANGE: Range<f32> = 0.05 * PI..0.3 * PI;

impl TriangleGrid {
    pub fn generate(device: &wgpu::Device) -> Self {
        let mut rng = rand::thread_rng();

        // first generate a series of rows of points;
        // we'll then turn them into triangles
        let mut pts: Vec<Vec<Point>> = Vec::new();

        let x_step = 2. / POINTS_PER_ROW as f32;
        let y_step = 2. / ROWS as f32;
        // random offsets in the vertex locations for visual interest
        let x_variance = x_step * X_VARIANCE_COEF;
        let y_variance = y_step * Y_VARIANCE_COEF;
        for row in 0..ROWS + 1 {
            let row_y = -1. + row as f32 * y_step;
            // every other row has an extra point and an offset in the x direction,
            // for a nice staggered pattern
            let left_edge = if row % 2 == 0 { -1. } else { -1. - x_step / 2. };

            let row_pts = chain!(
                // end points without any random variation or movement
                std::iter::once(Point {
                    root_pos: [-1., row_y],
                    ..Default::default()
                }),
                (1..POINTS_PER_ROW + row % 2).map(|col| {
                    let x =
                        left_edge + col as f32 * x_step + rng.gen_range(-x_variance..x_variance);
                    if row == 0 || row == ROWS {
                        // no movement or random y variation on the borders
                        let y = row_y;
                        Point {
                            root_pos: [x, y],
                            ..Default::default()
                        }
                    } else {
                        let y = row_y + rng.gen_range(-y_variance..y_variance);
                        Point {
                            root_pos: [x, y],
                            // color will be filled in later
                            color: [0.; 4],
                            // random movement parameters
                            x_phase: rng.gen_range(0.0..2. * PI),
                            x_velocity: rng.gen_range(X_VELOCITY_RANGE),
                            x_amplitude: rng.gen_range(X_AMPLITUDE_RANGE),
                            y_phase: rng.gen_range(0.0..2. * PI),
                            y_velocity: rng.gen_range(Y_VELOCITY_RANGE),
                            y_amplitude: rng.gen_range(Y_AMPLITUDE_RANGE),
                        }
                    }
                }),
                std::iter::once(Point {
                    root_pos: [1., row_y],
                    ..Default::default()
                }),
            )
            .collect();
            pts.push(row_pts);
        }

        // gradient for coloring the triangles

        let color_curve = Linear::builder()
            .elements([
                Srgb::new(0.0637, 0.0143, 0.110).into_linear(),
                Srgb::new(0.140, 0.073, 0.200).into_linear(),
                Srgb::new(0.290, 0.0580, 0.155).into_linear(),
                Srgb::new(0.163, 0.0756, 0.210).into_linear(),
                Srgb::new(0.0637, 0.0143, 0.110).into_linear(),
            ])
            .knots([-1., -0.8, -0.3, 0.5, 1.])
            .build()
            .unwrap();

        // generate triangles from the rows of vertices

        let mut points = Vec::new();
        for (pair_idx, row_pair) in pts.windows(2).enumerate() {
            let (shorter_row, longer_row) = if pair_idx % 2 == 0 {
                (&row_pair[0], &row_pair[1])
            } else {
                (&row_pair[1], &row_pair[0])
            };

            // generate a triangle strip between the two rows
            let mut gen_triangle = |pts: [Point; 3]| {
                let centroid_y =
                    (pts[0].root_pos[1] + pts[1].root_pos[1] + pts[2].root_pos[1]) / 3.;
                let c_lin: LinSrgba = color_curve.gen(centroid_y).into_color();
                let color = [c_lin.red, c_lin.green, c_lin.blue, c_lin.alpha];
                points.extend(pts.into_iter().map(|p| Point { color, ..p }));
            };

            for i in 0..shorter_row.len() - 1 {
                let first_tri_points = [shorter_row[i], longer_row[i], longer_row[i + 1]];
                let second_tri_points = [longer_row[i + 1], shorter_row[i + 1], shorter_row[i]];
                for tri_pts in [first_tri_points, second_tri_points] {
                    gen_triangle(tri_pts);
                }
            }
            let end = longer_row.len() - 1;
            gen_triangle([longer_row[end - 1], longer_row[end], shorter_row[end - 1]]);
        }

        // initialize a GPU buffer for these points

        let vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("triangle grid"),
            size: (points.len() * std::mem::size_of::<ColoredVertex>()) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let vertex_count = points.len() as u32;

        Self {
            points,
            vertex_buf,
            vertex_count,
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, t: f32) {
        let vertices: Vec<ColoredVertex> = self
            .points
            .iter()
            .map(|p| ColoredVertex {
                pos: [
                    p.root_pos[0] + p.x_amplitude * f32::sin(p.x_phase + p.x_velocity * t),
                    p.root_pos[1] + p.y_amplitude * f32::sin(p.y_phase + p.y_velocity * t),
                ],
                col: p.color,
            })
            .collect();

        queue.write_buffer(&self.vertex_buf, 0, bytemuck::cast_slice(&vertices));
    }
}
