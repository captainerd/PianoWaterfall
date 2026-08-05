use wgpu_jumpstart::wgpu;

use bytemuck::{Pod, Zeroable};
use wgpu::vertex_attr_array;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct NoteInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
    pub state: u32,
}

impl NoteInstance {
    pub fn attributes() -> [wgpu::VertexAttribute; 4] {
        // Attribute 1: position (Float32x2)
        // Attribute 2: size (Float32x2)
        // Attribute 3: color (Float32x4)
        // Attribute 4: state (Uint32)
        vertex_attr_array!(1 => Float32x2, 2 => Float32x2, 3 => Float32x4, 4 => Uint32)
    }

    pub fn layout(attributes: &[wgpu::VertexAttribute]) -> wgpu::VertexBufferLayout<'_> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<NoteInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes,
        }
    }
}
