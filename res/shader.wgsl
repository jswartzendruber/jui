fn rect_sdf(absolute_pixel_pos: vec2<f32>, origin: vec2<f32>, size: vec2<f32>, corner_radius: f32) -> f32 {
    var half_size: vec2<f32> = size / 2.0;
    var rect_center: vec2<f32> = origin + half_size;

    var pixel_pos: vec2<f32> = abs(absolute_pixel_pos - rect_center);

    var shrunk_corner_pos: vec2<f32> = half_size - corner_radius;

    var pixel_to_shrunk_corner: vec2<f32> = max(vec2<f32>(0.0, 0.0), pixel_pos - shrunk_corner_pos);

    var dist_to_shrunk_corner: f32 = length(pixel_to_shrunk_corner);

    var dist: f32 = dist_to_shrunk_corner - corner_radius;

    return dist;
}

struct InstanceInput {
    @location(5) transform: vec2<f32>,
    @location(6) scale: vec2<f32>,
}

struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct FragmentInput {
    @builtin(position) position: vec4<f32>,
    @location(2) color: vec4<f32>,
}

struct RectUniforms {
    size: vec2<f32>,
    origin: vec2<f32>,
    background_color: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> rect_uniforms: RectUniforms;

struct Uniforms {
    viewport_size: vec2<f32>,
}

@group(1) @binding(1)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(
    in: VertexInput,
    instance: InstanceInput,
) -> FragmentInput {
    var pixel_space_pos: vec2<f32> = (in.position * instance.scale) + instance.transform;
    var viewport_size: vec2<f32> = uniforms.viewport_size;

    var out: FragmentInput;
    out.position = vec4<f32>(pixel_space_pos / (viewport_size / 2.0), 0.0, 1.0);
    out.color = rect_uniforms.background_color;

    return out;
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {
    return in.color;
}