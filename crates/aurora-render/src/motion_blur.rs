//! Motion blur, as Black Dragon's RenderMotionBlur (newview/pipeline.cpp and
//! motionBlurF.glsl, originally LGPL 2.1): the image smeared along each
//! pixel's screen motion since the last frame, so a turning camera or a
//! running avatar leaves a streak. The motion is the one TAA reprojects
//! with (the depth prepass's, motion_history.rs; the camera's alone for the
//! sky). One pass over the resolved scene (`shaders/motion_blur.wgsl`).
//! Its target exists only while the effect is on: off, it costs nothing.

use crate::MotionBlur;
use crate::pipeline_cache::CachedPipelines;
use bytemuck::{Pod, Zeroable};
use glam::Mat4;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MotionBlurU {
    inv_view_proj: [[f32; 4]; 4],
    prev_view_proj: [[f32; 4]; 4],
    /// Strength, samples, longest smear (uv), noise phase.
    params: [f32; 4],
    /// x: 1 when the prepass's motion is read.
    motion: [f32; 4],
}

struct Targets {
    size: (u32, u32),
    out: wgpu::TextureView,
    /// The post pass's bind group reading `out`.
    post_bg: wgpu::BindGroup,
}

pub(crate) struct MotionBlurPass {
    layout: wgpu::BindGroupLayout,
    buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
    targets: Option<Targets>,
    frame: u32,
}

/// What the post pass reads, given to the target's bind group.
pub(crate) use crate::dof::PostInputs;

impl MotionBlurPass {
    pub(crate) fn new(device: &wgpu::Device, hdr: wgpu::TextureFormat) -> MotionBlurPass {
        let tex = |binding: u32, sample_type: wgpu::TextureSampleType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let float = wgpu::TextureSampleType::Float { filterable: true };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("motion blur"),
            entries: &[
                tex(0, float),
                tex(1, wgpu::TextureSampleType::Depth),
                tex(2, float),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("motion blur params"),
            size: std::mem::size_of::<MotionBlurU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("motion blur"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("motion blur"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/motion_blur.wgsl").into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("motion blur"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline_cached(&wgpu::RenderPipelineDescriptor {
            label: Some("motion blur"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_mb"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_mb"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: hdr,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        MotionBlurPass {
            layout,
            buffer,
            sampler,
            pipeline,
            targets: None,
            frame: 0,
        }
    }

    /// The target is dropped with the effect.
    pub(crate) fn release(&mut self) {
        self.targets = None;
    }

    /// Whether the target for `size` is made.
    pub(crate) fn has_targets(&self, size: (u32, u32)) -> bool {
        self.targets.as_ref().is_some_and(|t| t.size == size)
    }

    pub(crate) fn ensure_targets(&mut self, device: &wgpu::Device, size: (u32, u32), hdr: wgpu::TextureFormat, post: &PostInputs) {
        if self.has_targets(size) {
            return;
        }
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let out = crate::renderer::make_view(device, "motion blur out", size.0, size.1, hdr, 1, usage).1;
        let post_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post (motion blur)"),
            layout: post.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&out),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(post.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: post.buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(post.glow),
                },
            ],
        });
        self.targets = Some(Targets { size, out, post_bg });
    }

    /// The image the effect drew, for the passes after it.
    pub(crate) fn output(&self) -> Option<&wgpu::TextureView> {
        self.targets.as_ref().map(|t| &t.out)
    }

    /// The post pass's bind group reading the result.
    pub(crate) fn post_bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.targets.as_ref().map(|t| &t.post_bg)
    }

    /// The effect over `input` (the scene, after TAA), from the frame's
    /// depth and motion. `view_proj` / `prev_view_proj` are the frame's and
    /// the last one's, without jitter. Targets must exist (`ensure_targets`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        input: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        motion: &wgpu::TextureView,
        view_proj: Mat4,
        prev_view_proj: Mat4,
        params: &MotionBlur,
        motion_on: bool,
    ) {
        let Some(t) = self.targets.as_ref() else { return };
        self.frame = self.frame.wrapping_add(1);
        let u = MotionBlurU {
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            prev_view_proj: prev_view_proj.to_cols_array_2d(),
            params: [
                params.strength.clamp(0.0, 2.0),
                params.samples.clamp(4, 32) as f32,
                params.max_smear.clamp(0.005, 0.25),
                (self.frame % 16) as f32,
            ],
            motion: [if motion_on { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        };
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&u));
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("motion blur"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(motion),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.buffer.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("motion blur"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &t.out,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}
