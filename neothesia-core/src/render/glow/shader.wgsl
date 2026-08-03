struct ViewUniform {
    transform: mat4x4<f32>,
    size: vec2<f32>,
    scale: f32,
    time: f32,
}
@group(0) @binding(0) var<uniform> view: ViewUniform;

struct Vertex { @location(0) position: vec2<f32> }
struct QuadInstance {
    @location(1) q_position: vec2<f32>,
    @location(2) size: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) age: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) quad_color: vec4<f32>,
    @location(2) age: f32,
}

@vertex
fn vs_main(v: Vertex, q: QuadInstance) -> VertexOutput {
    let world_pos = (q.q_position + v.position * q.size) * view.scale;
    return VertexOutput(
        view.transform * vec4<f32>(world_pos, 0.0, 1.0),
        v.position,
        q.color,
        q.age
    );
}

fn rand2(p: vec2<f32>) -> vec2<f32> {
    return fract(sin(vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)))) * 43758.5453);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let st = (in.uv - 0.5) * 2.0;
    let dist = length(st);
    
    // Outer boundary mask
    let mask = 1.0 - smoothstep(0.85, 1.0, dist);
    if (mask <= 0.0) { discard; }

    // --- Continuous Breathing Glow (Underneath) ---
    let pulse = sin(view.time * 3.5) * 0.5 + 0.5;
    let glow_radius = max(0.0, 1.0 - dist * 1.4);
    let breathing_glow = (glow_radius * glow_radius) * (0.4 + pulse * 0.6) * 1.8;

    // --- Explosive Core Flash (Temporary) ---
    let core_shape = max(0.0, 1.0 - dist * 1.8);
    let burst_core = (core_shape * core_shape) * max(0.0, 1.0 - in.age * 2.0) * 2.0;

    // --- Exploding Dots ---
    let scaled_st = st * 6.0;
    let i_st = floor(scaled_st);
    let f_st = fract(scaled_st);

    var particles = 0.0;
    let radial_dir = normalize(st + vec2<f32>(0.0001));
    let base_thrust = in.age * 2.2;

    for (var y = -1.0; y <= 1.0; y += 1.0) {
        for (var x = -1.0; x <= 1.0; x += 1.0) {
            let neighbor = vec2<f32>(x, y);
            let p_id = i_st + neighbor;
            let point = rand2(p_id);
            
            let p_speed = 0.4 + point.x * 1.2;
            let drift = radial_dir * base_thrust * p_speed;

            let p_pos = neighbor + point + drift - f_st;
            let p_dist = length(p_pos);
            
            particles += smoothstep(0.07, 0.0, p_dist) * (1.2 + point.y * 0.8);
        }
    }

    // Combine layers: Breathing glow backing, burst flash, and flying particle field
    let total = breathing_glow + burst_core + (particles * 2.0);
    let alpha = clamp(total * mask * in.quad_color.a, 0.0, 1.0);

    return vec4<f32>(in.quad_color.rgb * total, alpha);
}