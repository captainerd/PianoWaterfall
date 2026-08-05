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
    out.uv_position = (vertex.position + vec2<f32>(1.0, 1.0)) / 2.0;
    return out;
}

fn rot_z(angle: f32) -> mat2x2<f32> {
    let ca = cos(angle);
    let sa = sin(angle);
    return mat2x2<f32>(
        vec2<f32>(ca, -sa),
        vec2<f32>(sa, ca)
    );
}

const speed: f32 = -0.35;
const live_time: f32 = 3.0;

fn note_render(uv: vec2<f32>, pos: f32, color: vec3<f32>) -> vec3<f32> {
    let mod_x: f32 = uv.x % (0.1 * 2.5 * 2.0);

    // Punchy neon crimson-red lane glow (zero green to eliminate yellow/brown)
    var col: vec3<f32> = vec3<f32>(0.45, 0.04, 0.08);

    if pos == 0.5 {
        col = vec3<f32>(0.22, 0.01, 0.03);
    }

    if uv.y > 0.0 && uv.y < 1.0 {
        let intensity = smoothstep(-0.003, 0.0, 127.0 / 5800.0 - abs(mod_x - pos));
        return mix(color, col, vec3<f32>(intensity));
    } else {
        return color;
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv: vec2<f32> = in.uv_position;

    // Deep pitch-black to dark ruby-crimson vertical gradient (no brown tints)
    var base_bg = mix(vec3<f32>(0.01, 0.005, 0.008), vec3<f32>(0.04, 0.008, 0.015), uv.y);
    var color: vec3<f32> = base_bg;

    // Slight angle for dynamic background streams
    uv = uv * rot_z(0.5);
    uv.x = uv.x + 1.0;
    uv.x = uv.x * 1.5;
    uv.x = uv.x % 0.5;

    {
        uv.y = uv.y - 1.5;

        var off: f32 = 0.0;
        var pos: vec2<f32> = uv;

        pos.y = pos.y - (((time_uniform.time * speed + off) / 5.0) % 1.0) * live_time;
        color = note_render(pos, 0.1, color);

        off = 1.0;
        pos = uv;
        pos.y = pos.y - (((time_uniform.time * speed + off) / 5.0) % 1.0) * live_time;
        color = note_render(pos, 0.1 * 2.0, color);

        off = 3.0;
        pos = uv;
        pos.y = pos.y - (((time_uniform.time * speed + off) / 5.0) % 1.0) * live_time;
        color = note_render(pos, 0.1 * 3.0, color);

        off = 2.0;
        pos = uv;
        pos.y = pos.y - (((time_uniform.time * speed + off) / 5.0) % 1.0) * live_time;
        color = note_render(pos, 0.1 * 4.0, color);

        off = 0.0;
        pos = uv;
        pos.y = pos.y - (((time_uniform.time * speed + off) / 5.0) % 1.0) * live_time;
        color = note_render(pos, 0.1 * 5.0, color);

        off = 4.0;
        pos = uv;
        pos.y = pos.y - (((time_uniform.time * speed + off) / 5.0) % 1.0) * live_time;
        color = note_render(pos, 0.1 * 5.0, color);
    }

    return vec4<f32>(color, 0.85);
}