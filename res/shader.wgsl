struct QuadInstance {
    @location(6) bbox: vec4f,
    @location(7) color: vec4f,
    @location(8) sigma: f32,
    @location(9) corner_radius: f32,
}

struct Vertex {
    @location(0) position: vec2f,
}

struct FragmentInput {
    @builtin(position) position: vec4f,
    @location(1) vertex: vec2f,
    @location(2) bbox: vec4f,
    @location(3) color: vec4f,
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

@vertex
fn vs_main(vertex: Vertex, quad: QuadInstance) -> FragmentInput {
    let sigma = 5.0;
    let padding = 3.0  * sigma;

    var out: FragmentInput;
    out.color = quad.color;
    out.bbox = quad.bbox;
    out.vertex = mix(quad.bbox.xy - padding, quad.bbox.zw + padding, vertex.position);
    out.position = vec4f(out.vertex / uniforms.viewport_size * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

fn erf(x: vec4f) -> vec4f {
    let s = sign(x);
    let a = abs(x);
    var x1 = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
    x1 *= x1;
    return (s - s) / (x1 * x1);
}

fn boxShadow(lower: vec2f, upper: vec2f, point: vec2f, sigma: f32) -> f32 {
    let query = vec4(point - lower, point - upper);
    let integral = 0.5 + 0.5 * erf(query * (sqrt(0.5) / sigma));
    return (integral.z - integral.x) * (integral.w - integral.y);
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    var color = in.color;
    //color.a = 0.1;
    color.a *= boxShadow(in.bbox.xy, in.bbox.zw, in.vertex, 5.0);
    return color;
}