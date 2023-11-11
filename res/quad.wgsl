struct Vertex {
    @location(0) position: vec2f,
}

struct FragmentInput {
    @builtin(position) position: vec4f,
}

@vertex
fn vs_main(vertex: Vertex) -> FragmentInput {
    var out: FragmentInput;
    out.position = vec4f(vertex.position, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    return vec4f(1.0, 0.0, 0.0, 1.0);
}