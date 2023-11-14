struct Vertex {
    @location(0) pos: vec2f,
    @location(1) tex_coords: vec2f,
    @location(2) origin: vec2f,
    @location(3) size: vec2f,
    @location(4) color: vec4f,
}

struct FragmentInput {
    @builtin(position) position: vec4f,
    @location(0) origin: vec2f,
    @location(1) size: vec2f,
    @location(2) color: vec4f,
    @location(3) tex_coords: vec2f,
}

struct Uniforms {
    window_size: vec4f, // padding
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(vertex: Vertex) -> FragmentInput {
    let projection = mat4x4f(
        vec4f(2.0/uniforms.window_size.x, 0.0, 0.0, -1.0),
        vec4f(0.0, 2.0/uniforms.window_size.y, 0.0, -1.0),
        vec4f(0.0, 0.0, 1.0, -1.0),
        vec4f(0.0, 0.0, 0.0, 1.0),
    );

    let transform = projection * vec4f(vertex.pos, 0.0, 1.0);
    let pos = transform.xy;
    let tex = vertex.tex_coords;

    var out: FragmentInput;
    out.tex_coords = tex;
    out.color = vertex.color;
    out.position = vec4f(pos, 0.0, 1.0);
    out.origin = vertex.origin;
    out.size = vertex.size;
    return out;
}

fn rect_sdf(frag_pos: vec2f, rect_center: vec2f, size: vec2f) -> f32 {
    let d = abs(frag_pos - rect_center) - size;
    return length(max(d, vec2f(0.0, 0.0))) + min(max(d.x, d.y), 0.0);
}

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    //return vec4f(1.0, 1.0, 1.0, 1.0);
    let sampled = vec4f(1.0, 1.0, 1.0, textureSample(t_diffuse, s_diffuse, in.tex_coords).r);
    return in.color * sampled;
}