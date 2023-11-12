struct Vertex {
    @location(0) pos: vec2f,
    @location(1) tex_coords: vec2f,
}

struct Quad {
    @location(2) origin: vec2f,
    @location(3) size: vec2f,
    @location(4) border_color: vec4f,
    @location(5) border: f32,
    @location(6) radius: f32,
}

struct FragmentInput {
    @builtin(position) position: vec4f,
    @location(0) origin: vec2f,
    @location(1) size: vec2f,
    @location(2) radius: f32,
    @location(3) pos: vec2f,
    @location(4) border: f32,
    @location(5) border_color: vec4f,
    @location(6) tex_coords: vec2f,
}

struct Uniforms {
    window_size: vec4f, // padding
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(vertex: Vertex, quad: Quad) -> FragmentInput {
    let transformed_coords = vertex.pos * quad.size  + quad.origin;
    let scaled_coords = uniforms.window_size.xy * 0.5 * (transformed_coords + vec2f(1.0, 1.0));

    var out: FragmentInput;
    out.tex_coords = vertex.tex_coords;
    out.border = quad.border;
    out.border_color = quad.border_color;
    out.pos = scaled_coords;
    out.position = vec4f(transformed_coords, 0.0, 1.0);
    out.origin = quad.origin;
    out.size = quad.size;
    out.radius = quad.radius;
    return out;
}

fn rounded_rect_sdf(frag_pos: vec2f, rect_center: vec2f, size: vec2f, radius: f32) -> f32 {
    let d = abs(frag_pos - rect_center) - size + radius;
    return length(max(d, vec2f(0.0, 0.0))) - radius + min(max(d.x, d.y), 0.0);
}

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    let pos = (in.pos.xy / (uniforms.window_size.xy / 2.0)) - 1.0;
    let dist = rounded_rect_sdf(pos, in.origin, in.size, in.radius);

    var color: vec3f;
    if dist < in.border {
        color = textureSample(t_diffuse, s_diffuse, in.tex_coords).xyz;
    } else if dist < 0.0 {
        color = in.border_color.xyz;
    } else {
        color = vec3f(0.0, 0.0, 0.0);
    }

    color = mix(color, vec3f(1.0), 1.0 - smoothstep(-1.0, 0.0, abs(dist)));
    return vec4f(color, 1.0);
}