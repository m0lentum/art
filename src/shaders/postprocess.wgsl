@group(0) @binding(0)
var gbuf_tex: texture_2d<f32>;
@group(0) @binding(1)
var gbuf_samp: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// vertex shader draws a single full-screen triangle using just vertex indices
// source: https://www.saschawillems.de/blog/2016/08/13/vulkan-tutorial-on-rendering-a-fullscreen-quad-without-buffers/
// (y flipped for wgpu)
@vertex
fn vs_main(
    @builtin(vertex_index) vert_idx: u32,
) -> VertexOutput {
    var out: VertexOutput;

    out.uv = vec2<f32>(f32((vert_idx << 1u) & 2u), f32(vert_idx & 2u));
    out.clip_position = vec4<f32>(out.uv.x * 2.0 - 1.0, out.uv.y * -2.0 + 1.0, 0., 1.);

    return out;
}

// CRT postprocessing effect based on
// https://babylonjs.medium.com/retro-crt-shader-a-post-processing-effect-study-1cb3f783afbc
// plus chromatic aberration and some other personal touches

fn distort_uv(in_uv: vec2<f32>) -> vec2<f32> {
    let curvature = 3.;

    var uv = in_uv;
    uv = uv * 2.0 - 1.0;
    let offset: vec2<f32> = abs(uv.yx) / curvature;
    uv = uv + uv * offset * offset;
    uv = uv * 0.5 + 0.5;
    return uv;
}

fn scanline_coef(coord: f32, resolution: f32) -> f32 {
    let opacity = 0.5;
    return pow(
	(0.5 * sin(coord * resolution * 3.14159 * 2.) + 0.5) * 0.9 + 0.1,
	opacity
    );
}

fn vignette_coef(uv: vec2<f32>, screen_size: vec2<f32>) -> f32 {
    let intensity = (screen_size.x / 16.0) * uv.x * uv.y * (1. - uv.x) * (1. - uv.y);
    return clamp(intensity, 0., 1.);
}

@fragment
fn fs_main(
    in: VertexOutput
) -> @location(0) vec4<f32> {
    let screen_size = vec2<f32>(textureDimensions(gbuf_tex));
    let distorted_uv = distort_uv(in.uv);

    // different channels offset slightly for chromatic aberration
    let aberration_intensity = 0.001;
    let red_uv = distorted_uv + aberration_intensity * vec2<f32>(1., 0.);
    let green_uv = distorted_uv + aberration_intensity * vec2<f32>(-0.8, 0.6);
    let blue_uv = distorted_uv + aberration_intensity * vec2<f32>(-0.8, -0.6);

    let screen_color = vec4<f32>(
    	textureSample(gbuf_tex, gbuf_samp, red_uv).r,
    	textureSample(gbuf_tex, gbuf_samp, green_uv).g,
    	textureSample(gbuf_tex, gbuf_samp, blue_uv).b,
	1.,
    );

    if distorted_uv.x < 0. || distorted_uv.x > 1. || distorted_uv.y < 0. || distorted_uv.y > 1. {
	return vec4<f32>(0., 0., 0., 1.);
    }

    let scanline = scanline_coef(distorted_uv.y, screen_size.x / 8.);
    let vignette = vignette_coef(in.uv, screen_size);
    let brightness_boost = 1.5;

    let dimmed_color = vec4<f32>(brightness_boost * scanline * vignette * screen_color.rgb, 1.);
    return dimmed_color;
}
