use std::time::Instant;

use iced::Rectangle;
use iced::widget::shader;
use wgpu::util::DeviceExt;

use orki_gfx::{AURORA_WGSL, OrkiUniforms};

pub struct AuroraShader {
    start: Instant,
    progress: f32,
}

impl AuroraShader {
    pub fn new(progress: f32) -> Self {
        Self {
            start: Instant::now(),
            progress,
        }
    }

    pub fn set_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AuroraPrimitive {
    time: f32,
    progress: f32,
    width: f32,
    height: f32,
}

impl shader::Primitive for AuroraPrimitive {
    type Pipeline = AuroraPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &shader::Viewport,
    ) {
        let uniforms = OrkiUniforms {
            time: self.time,
            progress: self.progress,
            resolution: [self.width, self.height],
            ..OrkiUniforms::default()
        };
        queue.write_buffer(&pipeline.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
    }

    fn draw(&self, pipeline: &Self::Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        render_pass.set_pipeline(&pipeline.pipeline);
        render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
        true
    }
}

pub struct AuroraPipeline {
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}

impl shader::Pipeline for AuroraPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("orki-aurora"),
            source: wgpu::ShaderSource::Wgsl(AURORA_WGSL.into()),
        });

        let uniforms = OrkiUniforms::default();
        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("orki-uniforms"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("orki-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("orki-pl"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("orki-rp"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let bgl_ref = pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("orki-bg"),
            layout: &bgl_ref,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        Self {
            uniform_buf,
            bind_group,
            pipeline,
        }
    }
}

impl<Message> shader::Program<Message> for AuroraShader
where
    Message: 'static,
{
    type State = ();
    type Primitive = AuroraPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: iced::mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        AuroraPrimitive {
            time: self.start.elapsed().as_secs_f32(),
            progress: self.progress,
            width: bounds.width,
            height: bounds.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use super::{AuroraPipeline, AuroraPrimitive, AuroraShader};

    #[test]
    fn primitive_layout() {
        assert_eq!(size_of::<AuroraPrimitive>(), 16);
    }

    #[test]
    fn shader_progress_clamped() {
        let mut s = AuroraShader::new(0.5);
        s.set_progress(1.5);
        assert_eq!(s.progress, 1.0);
        s.set_progress(-1.0);
        assert_eq!(s.progress, 0.0);
    }

    #[test]
    fn pipeline_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AuroraPipeline>();
    }
}
