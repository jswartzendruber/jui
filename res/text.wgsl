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

// Coordinates are converted from screen space [0,0], [800,600] where 0,0 is the
// bottom left corner of the screen, to opengl space [-1,1].
fn to_ndc(p: vec2f) -> vec2f {
    return vec2f(
        ((2.0 * p.x) / uniforms.window_size.x) - 1.0,
        ((2.0 * p.y) / uniforms.window_size.y) - 1.0,
    );
}

@vertex
fn vs_main(vertex: Vertex) -> FragmentInput {
    let transform = vec4f(to_ndc(vertex.pos), 0.0, 1.0);

    var out: FragmentInput;
    out.position = transform;
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