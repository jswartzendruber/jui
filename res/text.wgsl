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
    @location(2) pos: vec2f,
    @location(3) color: vec4f,
    @location(4) tex_coords: vec2f,
}

struct Uniforms {
    window_size: vec4f, // padding
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(vertex: Vertex) -> FragmentInput {
    let transformed_coords = vertex.pos * vertex.size  + vertex.origin;
    let scaled_coords = uniforms.window_size.xy * 0.5 * (transformed_coords + vec2f(1.0, 1.0));

    var out: FragmentInput;
    out.tex_coords = vertex.tex_coords;
    out.color = vertex.color;
    out.pos = scaled_coords;
    out.position = vec4f(transformed_coords, 0.0, 1.0);
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
    let pos = (in.pos.xy / (uniforms.window_size.xy / 2.0)) - 1.0;
    let dist = rect_sdf(pos, in.origin, in.size);

    var color: vec3f;
    if dist < 0.0 {
        color = textureSample(t_diffuse, s_diffuse, in.tex_coords).xyz;
    } else {
        color = vec3f(0.0, 0.0, 0.0);
    }

    color = mix(color, vec3f(1.0), 1.0 - smoothstep(-1.0, 0.0, abs(dist)));
    return vec4f(color, 1.0) * vec4f(in.color.xyz, 1.0);
}