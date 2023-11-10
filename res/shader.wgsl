struct QuadInstance {
    @location(6) bbox: vec4f,
    @location(7) color: vec4f,
    @location(8) sigma: f32,
    @location(9) corner_radius: f32,
}

struct Vertex {
    @location(0) position: vec2f,
    @location(1) tex_coords: vec2f,
}

struct FragmentInput {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
    @location(1) tex_coords: vec2f,
}

struct QuadUniforms {
    size: vec2f,
    origin: vec2f,
    background_color: vec4f,
}

@group(0) @binding(0)
var<uniform> quad_uniforms: QuadUniforms;

struct Uniforms {
    viewport_size: vec2f,
}

@group(1) @binding(1)
var<uniform> uniforms: Uniforms;

@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;

@group(2) @binding(1)
var s_diffuse: sampler;

@vertex
fn vs_main(vertex: Vertex, quad: QuadInstance) -> FragmentInput {
    var out: FragmentInput;
    out.position = vec4f(vertex.position, 1.0, 1.0);
    out.color = vec4f(1.0, 0.0, 0.0, 1.0);
    out.tex_coords = vertex.tex_coords;
    return out;
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}