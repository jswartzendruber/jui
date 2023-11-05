// https://zed.dev/blog/videogame
fn rect_sdf(absolute_pixel_pos: vec2<f32>, origin: vec2<f32>, size: vec2<f32>, corner_radius: f32) -> f32 {
    var half_size: vec2<f32> = size / 2.0;
    var rect_center: vec2<f32> = origin + half_size;
    var pixel_pos: vec2<f32> = abs(absolute_pixel_pos - rect_center);

    var shrunk_corner_pos: vec2<f32> = half_size - corner_radius;
    var pixel_to_shrunk_corner: vec2<f32> = max(vec2<f32>(0.0, 0.0), pixel_pos - shrunk_corner_pos);

    var dist_to_shrunk_corner: f32 = length(pixel_to_shrunk_corner);
    var dist: f32 = dist_to_shrunk_corner - corner_radius;

    return dist;
}

const pi: f32 = 3.141592653589793;

// https://madebyevan.com/shaders/fast-rounded-rectangle-shadows/
// A standard gaussian function, used for weighting samples
fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (sqrt(2.0 * pi) * sigma);
}

// This approximates the error function, needed for the gaussian integral
fn erf(x: vec2<f32>) -> vec2<f32> {
    var s: vec2<f32> = sign(x);
    var a: vec2<f32> = abs(x);
    var x2: vec2<f32> = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
    x2 *= x2;
    return (s - s) / (x2 * x2);
}

// Return the blurred mask along the x dimension
fn roundedBoxShadowX(x: f32, y: f32, sigma: f32, corner: f32, halfSize: vec2<f32>) -> f32 {
    var delta: f32 = min(halfSize.y - corner - abs(y), 0.0);
    var curved: f32 = halfSize.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
    var integral: vec2<f32> = 0.5 + 0.5 * erf((x + vec2(-curved, curved)) * (sqrt(0.5) / sigma));
    return integral.y - integral.x;
}

// Return the mask for the shadow of a box from lower to upper
fn roundedBoxShadow(lower: vec2<f32>, upper: vec2<f32>, point: vec2<f32>, sigma: f32, corner: f32) -> f32 {
    // Center everything to make the math easier
    var center: vec2<f32> = (lower + upper) * 0.5;
    var halfSize: vec2<f32> = (upper - lower) * 0.5;
    var point2: vec2<f32> = point - center;

    // The signal is only non-zero in a limited range, so don't waste samples
    var low: f32 = point2.y - halfSize.y;
    var high: f32 = point2.y + halfSize.y;
    var start: f32 = clamp(-3.0 * sigma, low, high);
    var end: f32 = clamp(3.0 * sigma, low, high);

    // Accumulate samples (we can get away with surprisingly few samples)
    var step: f32 = (end - start) / 4.0;
    var y: f32 = start + step * 0.5;
    var value: f32 = 0.0;
    for (var i = 0; i < 4; i++) {
        value += roundedBoxShadowX(point2.x, point2.y - y, sigma, corner, halfSize) * gaussian(y, sigma) * step;
        y += step;
    }

    return value;
}

struct InstanceInput {
    @location(5) position: vec2<f32>,
    @location(6) size: vec2<f32>,
}

struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct FragmentInput {
    @builtin(position) position: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) origin: vec2<f32>,
    @location(4) size: vec2<f32>,
    @location(7) vertex: vec2<f32>,
}

struct RectUniforms {
    size: vec2<f32>,
    origin: vec2<f32>,
    background_color: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> rect_uniforms: RectUniforms;

struct Uniforms {
    viewport_size: vec2<f32>,
}

@group(1) @binding(1)
var<uniform> uniforms: Uniforms;

fn to_device_position(position: vec2<f32>, viewport: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(position / (viewport / 2.0), 0.0, 1.0);
}

@vertex
fn vs_main(
    in: VertexInput,
    instance: InstanceInput,
) -> FragmentInput {
    var pixel_space_pos: vec2<f32> = in.position * instance.size + instance.position;

    var sigma: f32 = 6.17; // or (1 + sin(50)) * 10
    var padding: f32 = 3.0 * sigma;

    var out: FragmentInput;
    out.position = vec4<f32>(pixel_space_pos, 0.0, 1.0);
    out.color = rect_uniforms.background_color;
    out.origin = instance.position;
    out.size = instance.size;
    out.vertex = min(out.position.xy - padding, out.origin.xy + padding);

    return out;
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {
    var distance: f32 = rect_sdf(in.position.xy, in.origin, in.size, 100.0);
    var sigma: f32 = 6.17;

    if (distance > 0.0) {
        return vec4<f32>(1.0, 1.0, 1.0, 0.0);
    } else {
        var color: vec4<f32> = vec4<f32>(1.0, 0.0, 0.0, 1.0);
        color.a *= roundedBoxShadow(in.position.xy, in.position.zw, in.origin, sigma, 12.0);
        return color;
    }
}