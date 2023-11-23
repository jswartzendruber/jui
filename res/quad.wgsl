struct Vertex {
    @location(0) pos: vec2f,
}

struct Quad {
    @location(1) origin: vec2f,
    @location(2) size: vec2f,
    @location(3) color: vec4f,
    @location(4) border_color: vec4f,
    @location(5) border: f32,
    @location(6) radius: f32,
}

struct FragmentInput {
    @builtin(position) position: vec4f,
    @location(0) origin: vec2f,
    @location(1) size: vec2f,
    @location(2) color: vec4f,
    @location(3) radius: f32,
    @location(4) border: f32,
    @location(5) border_color: vec4f,
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
fn vs_main(vertex: Vertex, quad: Quad) -> FragmentInput {
    let ndc_origin = to_ndc(quad.origin);
    let transformed_coords = vertex.pos * quad.size + ndc_origin;

    var out: FragmentInput;
    out.position = vec4f(transformed_coords, 0.0, 1.0);
    out.border_color = quad.border_color;
    out.radius = quad.radius;
    out.border = quad.border;
    out.origin = ndc_origin;
    out.color = quad.color;
    out.size = quad.size;
    return out;
}

fn rounded_rect_sdf(frag_pos: vec2f, rect_center: vec2f, size: vec2f, radius: f32) -> f32 {
    let q = vec2f(frag_pos.x - rect_center.x, frag_pos.y + rect_center.y);
    let d = abs(q) - size + radius;
    return min(max(d.x, d.y), 0.0) - radius + length(max(d, vec2f(0.0, 0.0)));
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    let pos = to_ndc(in.position.xy);
    let dist = rounded_rect_sdf(pos, in.origin, in.size, in.radius);

    var color: vec3f;
    if dist < in.border {
        color = in.color.xyz;
    } else if dist < 0.0 {
        color = in.border_color.xyz;
    } else {
        color = vec3f(0.0, 0.0, 0.0);
    }

    return vec4f(color, 1.0);
}