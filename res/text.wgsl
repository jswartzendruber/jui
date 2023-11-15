struct Vertex {
    @location(0) pos: vec2f,
    @location(1) tex_coords: vec2f,
    @location(2) color: vec4f,
}

struct FragmentInput {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
    @location(1) tex_coords: vec2f,
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

    var out: FragmentInput;
    out.position = vec4f(transform.xy, 0.0, 1.0);
    out.tex_coords = vertex.tex_coords;
    out.color = vertex.color;
    return out;
}

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    return vec4f(in.color.rgb, textureSample(t_diffuse, s_diffuse, in.tex_coords).a);
}