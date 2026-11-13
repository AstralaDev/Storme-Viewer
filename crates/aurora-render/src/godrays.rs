//! Volumetric lighting (godrays), as Black Dragon's RenderVolumetricLighting
//! (LLPipeline::renderVolumetric, newview/pipeline.cpp, and
//! volumetricLightF.glsl, originally LGPL 2.1): each pixel's ray from the
//! camera is walked through the sun's shadow cascades and the light it
//! meets is added in the sun's colour. One pass over the resolved scene
//! (`shaders/godrays.wgsl`), reading the frame's shadow atlas as the
//! shading does. Its target exists only while the effect is on: off, it
//! costs nothing.

use crate::Godrays;
use crate::pipeline_cache::CachedPipelines;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GodrayU {
    inv_view_proj: [[f32; 4]; 4],
    cascade_vp: [[[f32; 4]; 4]; 3],
    splits: [f32; 4],
    camera_pos: [f32; 4],
    /// rgb: the sun's light, w: how visible the sun is.
    sun_color: [f32; 4],
    /// Strength, steps, falloff, haze weight.
    params: [f32; 4],
    /// x: noise phase of the frame.
    noise: [f32; 4],
}

/// What the pass reads from the frame's uniforms and shadow atlas.
pub(crate) struct Sun<'a> {
    pub inv_view_proj: glam::Mat4,
    pub cascade_vp: [[[f32; 4]; 4]; 3],
    pub splits: [f32; 4],
    pub camera_pos: [f32; 4],
    pub sunlight: [f32; 3],
    pub visible: f32,
    /// haze / (blue + haze) of the sky (volumetricLightF `haze_weight.x`).
    pub haze_weight: f32,
    pub shadow_map: &'a wgpu::TextureView,
    pub shadow_sampler: &'a wgpu::Sampler,
}

struct Targets {
    size: (u32, u32),
    out: wgpu::TextureView,
    /// The post pass's bind group reading `out`.
    post_bg: wgpu::BindGroup,
}

pub(crate) struct GodrayPass {
    layout: wgpu::BindGroupLayout,
    buffer: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    targets: Option<Targets>,
    frame: u32,
}

/// What the post pass reads, given to the target's bind group.
pub(crate) use crate::dof::PostInputs;

impl GodrayPass {
    pub(crate) fn new(device: &wgpu::Device, hdr: wgpu::TextureFormat) -> GodrayPass {
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
            label: Some("godrays"),
            entries: &[
                tex(0, float),
                tex(1, wgpu::TextureSampleType::Depth),
                tex(2, wgpu::TextureSampleType::Depth),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
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
            label: Some("godrays params"),
            size: std::mem::size_of::<GodrayU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("godrays"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/godrays.wgsl").into()),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("godrays"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline_cached(&wgpu::RenderPipelineDescriptor {
            label: Some("godrays"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_gr"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_gr"),
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
        GodrayPass {
            layout,
            buffer,
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
        let out = crate::renderer::make_view(device, "godrays out", size.0, size.1, hdr, 1, usage).1;
        let post_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post (godrays)"),
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

    /// The effect over `input` (the scene, after TAA) with the depth of the
    /// frame. Targets must exist (`ensure_targets`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        input: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        sun: &Sun,
        params: &Godrays,
    ) {
        let Some(t) = self.targets.as_ref() else { return };
        self.frame = self.frame.wrapping_add(1);
        let u = GodrayU {
            inv_view_proj: sun.inv_view_proj.to_cols_array_2d(),
            cascade_vp: sun.cascade_vp,
            splits: sun.splits,
            camera_pos: sun.camera_pos,
            sun_color: [sun.sunlight[0], sun.sunlight[1], sun.sunlight[2], sun.visible.clamp(0.0, 1.0)],
            params: [
                params.strength.clamp(0.0, 8.0),
                params.steps.clamp(8, 128) as f32,
                params.falloff.clamp(0.0, 1.0),
                sun.haze_weight.clamp(0.0, 1.0),
            ],
            noise: [(self.frame % 16) as f32, 0.0, 0.0, 0.0],
        };
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&u));
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("godrays"),
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
                    resource: wgpu::BindingResource::TextureView(sun.shadow_map),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(sun.shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.buffer.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("godrays"),
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
