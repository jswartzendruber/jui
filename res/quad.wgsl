struct Vertex {
    @location(0) pos: vec2f,
}

struct Quad {
    @location(1) size: vec2f,
    @location(2) pos: vec2f,
}

struct FragmentInput {
    @builtin(position) position: vec4f,
}

struct Uniforms {
    camera: mat4x4f,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(vertex: Vertex, quad: Quad) -> FragmentInput {
    var out: FragmentInput;

    var transform: mat4x4f = mat4x4f(
        vec4f(quad.size.x, 0.0, 0.0, 0.0),
        vec4f(0.0, quad.size.y, 0.0, 0.0),
        vec4f(0.0, 0.0, 1.0, 0.0),
        vec4f(quad.pos, 0.0, 1.0)
    );

    out.position = uniforms.camera * transform * vec4f(vertex.pos, 1.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    return vec4f(1.0, 0.0, 0.0, 1.0);
}