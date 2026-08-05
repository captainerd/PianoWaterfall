use std::time::Duration;

use wgpu_jumpstart::{Color, Gpu, TransformUniform, Uniform};

use super::{GlowInstance, GlowPipeline};

struct GlowState {
    time: f32,
    active_timer: f32,
    was_pressed_last_frame: bool,
    pushed_this_frame: bool,
}

impl GlowState {
    fn size(&self) -> f32 {
        150.0 + self.time.sin() * 10.0
    }

    fn update(&mut self, delta: Duration, is_pressed: bool, retrigger: bool) {
        let dt = delta.as_secs_f32();

        if is_pressed {
            if !self.was_pressed_last_frame || retrigger {
                // Brand new note press OR rapid re-strike: reset age timer
                self.active_timer = 0.0;
                self.time = 0.0;
            } else {
                // Note held down continuously: progress age
                self.active_timer += dt;
            }
            self.time += dt * 5.0;
            self.was_pressed_last_frame = true;
        } else {
            // Note released: clear state
            self.was_pressed_last_frame = false;
            self.active_timer = 0.0;
        }
    }

    fn calc_color(&self, color: Color) -> [f32; 4] {
        let mut color = color.into_linear_rgba();
        let v = 0.2 * self.time.cos().abs();
        let v = v.min(1.0);
        color[0] += v;
        color[1] += v;
        color[2] += v;
        color
    }
}

pub struct GlowRenderer {
    pipeline: GlowPipeline,
    states: Vec<GlowState>,
}

impl GlowRenderer {
    pub fn new(
        gpu: &Gpu,
        transform: &Uniform<TransformUniform>,
        layout: &crate::piano_layout::KeyboardLayout,
    ) -> Self {
        let pipeline = GlowPipeline::new(gpu, transform);

        let states: Vec<GlowState> = layout
            .keys
            .iter()
            .map(|_| GlowState {
                time: 0.0,
                active_timer: 0.0,
                was_pressed_last_frame: false,
                pushed_this_frame: false,
            })
            .collect();

        Self { pipeline, states }
    }

    pub fn prepare(&mut self) {
        // Process keys that were NOT pushed during update_glow() this frame
        for state in &mut self.states {
            if !state.pushed_this_frame {
                state.update(Duration::ZERO, false, false);
            }
            // Reset flag for the next frame iteration
            state.pushed_this_frame = false;
        }

        self.pipeline.prepare();
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.pipeline.render(render_pass);
    }

    pub fn clear(&mut self) {
        self.pipeline.clear();
    }

    pub fn push(
        &mut self,
        id: usize,
        color: Color,
        key_x: f32,
        key_y: f32,
        key_w: f32,
        delta: Duration,
        retrigger: bool,
    ) {
        if id >= self.states.len() {
            return;
        }

        let state = &mut self.states[id];

        state.pushed_this_frame = true;
        state.update(delta, true, retrigger);

        let color = state.calc_color(color);
        let glow_w = state.size();
        let glow_h = glow_w;

        self.pipeline.instances().push(GlowInstance {
            position: [key_x - glow_w / 2.0 + key_w / 2.0, key_y - glow_w / 2.0],
            size: [glow_w, glow_h],
            color,
            age: state.active_timer,
        });
    }
}