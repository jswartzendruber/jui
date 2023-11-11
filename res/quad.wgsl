struct Vertex {
    @location(0) pos: vec2f,
}

struct Quad {
    @location(1) origin: vec2f,
    @location(2) size: vec2f,
    @location(3) color: vec4f,
    @location(4) radius: f32,
}

struct FragmentInput {
    @builtin(position) position: vec4f,
    @location(0) origin: vec2f,
    @location(1) size: vec2f,
    @location(2) color: vec4f,
    @location(3) radius: f32,
    @location(4) pos: vec2f,
}

struct Uniforms {
    camera: mat4x4f,
    window_size: vec4f, // padding
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(vertex: Vertex, quad: Quad) -> FragmentInput {
    let transformed_coords = vertex.pos * quad.size  + quad.origin;
    let scaled_coords = uniforms.window_size.xy * 0.5 * (transformed_coords + vec2f(1.0, 1.0));

    var out: FragmentInput;
    out.pos = scaled_coords;
    out.position = vec4f(transformed_coords, 0.0, 1.0);
    out.origin = quad.origin;
    out.size = quad.size;
    out.color = quad.color;
    out.radius = quad.radius;
    return out;
}

fn rounded_rect_sdf(frag_pos: vec2f, rect_center: vec2f, size: vec2f, radius: f32) -> f32{
    return length(max(abs(frag_pos - rect_center) - size + radius, vec2f(0.0, 0.0))) - radius;
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    let pos = (in.pos.xy / (uniforms.window_size.xy / 2.0)) - 1.0;
    var dist = rounded_rect_sdf(pos, in.origin, in.size, in.radius);

    var color = select(in.color.xyz, vec3(0.0, 0.0, 0.0), dist > 0.0);
    color = mix(color, vec3f(1.0), 1.0 - smoothstep(-1.0, 0.0, abs(dist)));
    return vec4f(color, 1.0);
}