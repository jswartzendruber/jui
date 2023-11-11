struct Vertex {
    @location(0) pos: vec2f,
}

struct Quad {
    @location(1) scale: vec2f,
    @location(2) pos: vec2f,
}

struct FragmentInput {
    @builtin(position) position: vec4f,
}

@vertex
fn vs_main(vertex: Vertex, quad: Quad) -> FragmentInput {
    var out: FragmentInput;

    var transform: mat4x4<f32> = mat4x4<f32>(
        vec4<f32>(quad.scale.x, 0.0, 0.0, 0.0),
        vec4<f32>(0.0, quad.scale.y, 0.0, 0.0),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(quad.pos, 0.0, 1.0)
    );

    out.position = transform * vec4f(vertex.pos, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    return vec4f(1.0, 0.0, 0.0, 1.0);
}