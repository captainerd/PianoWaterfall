struct TimeUniform {
    time: f32,
}

@group(0) @binding(0)
var<uniform> time_uniform: TimeUniform;

struct Vertex {
    @location(0) position: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv_position: vec2<f32>,
}

@vertex
fn vs_main(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(vertex.position, 0.0, 1.0);
    out.uv_position = (vertex.position + vec2<f32>(1.0, 1.0)) * 0.5;
    return out;
}

fn rot_z(angle: f32) -> mat2x2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return mat2x2<f32>(
        vec2<f32>(c, -s),
        vec2<f32>(s,  c)
    );
}

// Slower falling speed
const speed: f32 = 0.25;

// Rounded rectangle SDF
fn rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(radius);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

// Piano-key shaped falling note with an even darker aesthetic
fn note_render(
    uv: vec2<f32>,
    center: vec2<f32>,
    size: vec2<f32>,
    color: vec3<f32>,
) -> vec3<f32> {

    let dist = rounded_rect(uv - center, size, 0.015);

    let body = 1.0 - smoothstep(0.0, 0.010, dist);
    let glow = 1.0 - smoothstep(0.0, 0.060, dist);

    // Darker, more subdued crimson/magenta tone
    let noteColor = vec3<f32>(0.38, 0.07, 0.12);

    return mix(
        color,
        noteColor + glow * vec3<f32>(0.09, 0.01, 0.02),
        max(body, glow * 0.25)
    );
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {

    let uv = in.uv_position;

    //------------------------------------------------------
    // Background (Dark)
    //------------------------------------------------------

    var color = mix(
        vec3<f32>(0.002, 0.001, 0.002),
        vec3<f32>(0.012, 0.002, 0.004),
        uv.y
    );

    //------------------------------------------------------
    // Optional diagonal background streaks
    //------------------------------------------------------

    {
        var bg = uv;
        bg = bg * rot_z(0.45);

        let stripe = abs(fract(bg.x * 14.0) - 0.5);

        let s = smoothstep(
            0.06,
            0.0,
            stripe
        );

        color += s * vec3<f32>(0.01, 0.001, 0.002);
    }

    //------------------------------------------------------
    // Falling direction (Straight down)
    //------------------------------------------------------

    let angle = radians(-5.0);

    let dir = normalize(vec2<f32>(
        sin(angle),
        -cos(angle)
    ));

    //------------------------------------------------------
    // Lanes
    //------------------------------------------------------

    let laneCount = 12u;

    for (var i = 0u; i < laneCount; i++) {

        let laneX = -0.05 + f32(i) * 0.095;

        let off = f32(i) * 1.37 + sin(f32(i) * 4.5) * 0.8;
        let heightVariation = 0.07 + abs(sin(f32(i) * 2.1)) * 0.12;

        let t = fract(time_uniform.time * (speed + (sin(f32(i)) * 0.05)) + off);

        let start = vec2<f32>(laneX, 1.35);
        let center = start + dir * (t * 2.0);

        color = note_render(
            uv,
            center,
            vec2<f32>(
                0.025,
                heightVariation
            ),
            color
        );
    }

    //------------------------------------------------------
    // Keyboard line
    //------------------------------------------------------

    let line =
        smoothstep(
            0.004,
            0.0,
            abs(uv.y - 0.08)
        );

    color += line * vec3<f32>(0.20, 0.02, 0.04);

    return vec4<f32>(color, 0.85);
}