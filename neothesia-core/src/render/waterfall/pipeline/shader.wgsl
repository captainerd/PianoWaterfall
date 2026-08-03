struct ViewUniform {
    transform: mat4x4<f32>,
    size: vec2<f32>,
    scale: f32,
}

struct TimeUniform {
    time: f32,
    speed: f32,
    real_time: f32, // Match the Rust struct layout
}
@group(0) @binding(0)
var<uniform> view_uniform: ViewUniform;

@group(1) @binding(0)
var<uniform> time_uniform: TimeUniform;

struct Vertex {
    @location(0) position: vec2<f32>,
}

struct NoteInstance {
    @location(1) n_position: vec2<f32>,
    @location(2) size: vec2<f32>,
    @location(3) color: vec3<f32>,
    @location(4) state: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,

    @location(0) src_position: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec3<f32>,
    @location(3) @interpolate(flat) state: u32,
    @location(4) note_pos: vec2<f32>,
}

fn dist(
    frag_coord: vec2<f32>,
    position: vec2<f32>,
    size: vec2<f32>,
) -> f32 {
    let radius = 0.0;
    let inner_size: vec2<f32> = size - vec2<f32>(radius, radius) * 2.0;
    let top_left: vec2<f32> = position + vec2<f32>(radius, radius);
    let bottom_right: vec2<f32> = top_left + inner_size;

    let top_left_distance: vec2<f32> = top_left - frag_coord;
    let bottom_right_distance: vec2<f32> = frag_coord - bottom_right;

    let dist_val: vec2<f32> = vec2<f32>(
        max(max(top_left_distance.x, bottom_right_distance.x), 0.0),
        max(max(top_left_distance.y, bottom_right_distance.y), 0.0),
    );

    return sqrt(dist_val.x * dist_val.x + dist_val.y * dist_val.y);
}

@vertex
fn vs_main(vertex: Vertex, note: NoteInstance) -> VertexOutput {
    let speed = time_uniform.speed;

    let size = vec2<f32>(note.size.x * view_uniform.scale, note.size.y * abs(speed));

    let keyboard_h = view_uniform.size.y / 5.0;
    let keyboard_y = view_uniform.size.y - keyboard_h;

    var pos = vec2<f32>(note.n_position.x * view_uniform.scale, keyboard_y);

    if speed > 0.0 {
        pos.y -= size.y;
    }

    // Offset position by playback time
    let time_offset = (note.n_position.y - time_uniform.time) * speed;
    pos.y -= time_offset;

    // --- State Calculation ---
    var computed_state: u32 = 0u; 
    let note_bottom = pos.y + size.y;
    
    if pos.y <= keyboard_y && note_bottom >= keyboard_y {
        computed_state = 1u; // Active touch zone
    } else if note_bottom < keyboard_y {
        computed_state = 2u; // Passed completely below
    }

    let transform = mat4x4<f32>(
        vec4<f32>(size.x, 0.0,    0.0, 0.0),
        vec4<f32>(0.0,    size.y, 0.0, 0.0),
        vec4<f32>(0.0,    0.0,    1.0, 0.0),
        vec4<f32>(pos.x,  pos.y,  0.0, 1.0)
    );

    var out: VertexOutput;
    out.position = view_uniform.transform * transform * vec4<f32>(vertex.position, 0.0, 1.0);
    out.note_pos = pos;

    out.src_position = vertex.position;
    out.size = size;
    out.color = note.color;
    out.state = computed_state;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let distance_val: f32 = dist(
        in.position.xy,
        in.note_pos,
        in.size,
    );

    let alpha: f32 = 1.0 - smoothstep(
        0.0,
        1.0,
        distance_val,
    );

    var final_color = in.color;

    if (in.state == 1u) {
        // Use real_time so it pulses and breathes even when paused/waiting!
        let blink = sin(time_uniform.real_time * 25.0) * 0.5 + 0.5;
        let complementary_color = vec3<f32>(1.0) - in.color;
        let pulse_color = mix(complementary_color * 1.5, vec3<f32>(2.0, 2.0, 0.5), blink);

        // Body gradient animated via real_time
        let body_gradient = sin(in.src_position.y * 12.0 - time_uniform.real_time * 6.0) * 0.2 + 0.8;

        if (in.size.y < 50.0) {
            final_color = mix(final_color, pulse_color * body_gradient, 0.85);
            final_color *= 1.8;
        } else {
            let tip = smoothstep(0.4, 1.0, in.src_position.y);
            final_color *= body_gradient;

            final_color = mix(
                final_color,
                pulse_color,
                tip,
            );
            final_color *= 1.6;
        }
    } else if (in.state == 2u) {
        final_color *= 0.8;
    }

    return vec4<f32>(final_color, alpha);
}