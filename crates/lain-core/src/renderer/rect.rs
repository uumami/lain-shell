use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BlendState, Buffer, BufferDescriptor, BufferUsages, ColorTargetState,
    ColorWrites, Device, FragmentState, MultisampleState, PipelineLayoutDescriptor, Queue,
    RenderPass, RenderPipeline, ShaderStages, TextureFormat, VertexAttribute, VertexBufferLayout,
    VertexFormat, VertexState, VertexStepMode,
};

const WGSL_SOURCE: &str = r#"
struct Uniforms {
    resolution: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, instance: VertexInput) -> VertexOutput {
    // Quad vertices: 0=TL, 1=TR, 2=BL, 3=BR (triangle strip)
    let x = select(0.0, 1.0, vertex_index == 1u || vertex_index == 3u);
    let y = select(0.0, 1.0, vertex_index == 2u || vertex_index == 3u);

    let pixel_pos = instance.pos + vec2<f32>(x, y) * instance.size;
    let ndc = vec2<f32>(
        pixel_pos.x / uniforms.resolution.x * 2.0 - 1.0,
        1.0 - pixel_pos.y / uniforms.resolution.y * 2.0,
    );

    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RectInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    _pad: [f32; 2],
}

pub struct RectRenderer {
    pipeline: RenderPipeline,
    instance_buffer: Buffer,
    instance_capacity: usize,
    uniform_buffer: Buffer,
    bind_group: BindGroup,
    instance_count: u32,
}

impl RectRenderer {
    pub fn new(device: &Device, format: TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect shader"),
            source: wgpu::ShaderSource::Wgsl(WGSL_SOURCE.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("rect bind group layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("rect pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rect pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<RectInstance>() as u64,
                    step_mode: VertexStepMode::Instance,
                    attributes: &[
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        VertexAttribute {
                            format: VertexFormat::Float32x4,
                            offset: 16,
                            shader_location: 2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let instance_capacity = 4096;
        let instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("rect instances"),
            size: (instance_capacity * std::mem::size_of::<RectInstance>()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("rect uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("rect bind group"),
            layout: &bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            instance_buffer,
            instance_capacity,
            uniform_buffer,
            bind_group,
            instance_count: 0,
        }
    }

    pub fn prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        width: u32,
        height: u32,
        rects: &[RectInstance],
    ) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&Uniforms {
                resolution: [width as f32, height as f32],
                _pad: [0.0; 2],
            }),
        );

        if rects.len() > self.instance_capacity {
            self.instance_capacity = rects.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("rect instances"),
                size: (self.instance_capacity * std::mem::size_of::<RectInstance>()) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(rects));
        self.instance_count = rects.len() as u32;
    }

    pub fn render<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        if self.instance_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        pass.draw(0..4, 0..self.instance_count);
    }
}
