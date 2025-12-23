use bytemuck::{Pod, Zeroable};
use log::info;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::vec;
use wgpu::util::DeviceExt;
use winit::window::Window;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::volume_providers::volume_provider::{VolumeProvider, get_volume_provider};

const SAMPLE_COUNT: u32 = 4;

pub struct State {
    pub window: Arc<Window>,

    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,
    msaa_texture_view: wgpu::TextureView,

    window_size_buffer: wgpu::Buffer,
    window_pos_buffer: wgpu::Buffer,
    delta_time_buffer: wgpu::Buffer,
    intensity_buffer: wgpu::Buffer,
    last_intensity: f32,
    intensity_multiplier: f32,
    points_buffer: wgpu::Buffer,

    compute_new_positions_pipeline: wgpu::ComputePipeline,
    compute_new_positions_bind_group: wgpu::BindGroup,

    render_pipeline: wgpu::RenderPipeline,
    render_bind_group: wgpu::BindGroup,

    background_image_state: Option<(wgpu::RenderPipeline, wgpu::BindGroup)>,

    points_count: usize,

    volume_provider: Rc<dyn VolumeProvider>,
}

impl State {
    pub async fn new(
        window: Arc<Window>,
        background_image: Option<String>,
    ) -> anyhow::Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let msaa_texture_view = Self::create_msaa_texture(&device, &config);

        let window_size = WindowSize {
            size: [size.width as f32, size.height as f32],
        };

        let window_size_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Window Size Buffer"),
            contents: bytemuck::bytes_of(&window_size),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let window_pos = window
            .inner_position()
            .unwrap_or(winit::dpi::PhysicalPosition { x: 0, y: 0 });
        let window_pos = WindowSize {
            size: [window_pos.x as f32, window_pos.y as f32],
        };

        let window_pos_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Window Position Buffer"),
            contents: bytemuck::bytes_of(&window_pos),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let delta_time = DeltaTime { dt: 0.016 };

        let delta_time_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Delta Time Buffer"),
            contents: bytemuck::bytes_of(&delta_time),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let point_size = 5f32;

        let point_size_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Point Size Buffer"),
            contents: bytemuck::bytes_of(&point_size),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let intensity = 0.8f32;

        let intensity_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Intensity Buffer"),
            contents: bytemuck::bytes_of(&intensity),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let points_count = 1000;

        let points = Self::create_points(points_count, window_size);

        let points_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Points Buffer"),
            contents: bytemuck::cast_slice(&points),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let (compute_new_positions_pipeline, compute_new_positions_bind_group) =
            Self::create_compute_new_positions_pipeline(
                &device,
                &points_buffer,
                &window_size_buffer,
                &delta_time_buffer,
            );

        let background_image_state = if let Some(background_image) = background_image {
            let monitor_size = window
                .available_monitors()
                .into_iter()
                .find(|_| true)
                .unwrap()
                .size();
            info!("Monitor size: {monitor_size:?}");
            let background_image_rgba = image::ImageReader::open(background_image)?
                .decode()?
                .resize_to_fill(
                    monitor_size.width,
                    monitor_size.height,
                    image::imageops::FilterType::Lanczos3,
                )
                .to_rgba8();
            let dimensions = background_image_rgba.dimensions();
            let background_image_texture_size = wgpu::Extent3d {
                width: dimensions.0,
                height: dimensions.1,
                depth_or_array_layers: 1,
            };

            let background_image_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Background Image Texture"),
                size: background_image_texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &background_image_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &background_image_rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * dimensions.0),
                    rows_per_image: Some(dimensions.1),
                },
                background_image_texture_size,
            );

            let background_image_texture_view =
                background_image_texture.create_view(&wgpu::TextureViewDescriptor::default());

            let background_image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });

            let background_image_bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Background Image Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

            let background_image_bind_group =
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Background Image Bind Group"),
                    layout: &background_image_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &background_image_texture_view,
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&background_image_sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: window_size_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: window_pos_buffer.as_entire_binding(),
                        },
                    ],
                });

            let background_image_shader =
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Background Image Shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("shaders/background_image_shader.wgsl").into(),
                    ),
                });

            let background_image_render_pipeline_layout =
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Background Image Render Pipeline Layout"),
                    bind_group_layouts: &[&background_image_bind_group_layout],
                    push_constant_ranges: &[],
                });

            let background_image_render_pipeline =
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Background Image Render Pipeline"),
                    layout: Some(&background_image_render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &background_image_shader,
                        entry_point: Some("vs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &background_image_shader,
                        entry_point: Some("fs_main"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: config.format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: SAMPLE_COUNT,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                    cache: None,
                });

            Some((
                background_image_render_pipeline,
                background_image_bind_group,
            ))
        } else {
            None
        };

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });

        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Render Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
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

        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render Bind Group"),
            layout: &render_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: points_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: window_size_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: point_size_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: intensity_buffer.as_entire_binding(),
                },
            ],
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&render_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: SAMPLE_COUNT,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let volume_provider = get_volume_provider();

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
            msaa_texture_view,
            window_size_buffer,
            window_pos_buffer,
            delta_time_buffer,
            intensity_buffer,
            last_intensity: intensity,
            intensity_multiplier: 1.0,
            points_buffer,
            compute_new_positions_pipeline,
            compute_new_positions_bind_group,
            render_pipeline,
            render_bind_group,
            background_image_state,
            points_count,
            volume_provider,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.is_surface_configured = true;

            self.msaa_texture_view = Self::create_msaa_texture(&self.device, &self.config);

            let window_size = WindowSize {
                size: [width as f32, height as f32],
            };
            self.queue.write_buffer(
                &self.window_size_buffer,
                0,
                bytemuck::bytes_of(&window_size),
            );

            let window_pos = self
                .get_window_pos()
                .unwrap_or(WindowSize { size: [0.0, 0.0] });
            self.queue
                .write_buffer(&self.window_pos_buffer, 0, bytemuck::bytes_of(&window_pos));

            let points = Self::create_points(self.points_count, window_size);
            self.queue
                .write_buffer(&self.points_buffer, 0, bytemuck::cast_slice(&points));
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        self.window.request_redraw();

        if !self.is_surface_configured {
            return Ok(());
        }

        let output = self.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute New Positions Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_new_positions_pipeline);
            compute_pass.set_bind_group(0, &self.compute_new_positions_bind_group, &[]);

            let num_dispatches = (self.points_count as u32 + 63) / 64;
            compute_pass.dispatch_workgroups(num_dispatches, 1, 1);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_texture_view,
                    resolve_target: Some(&view),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            if let Some((background_image_render_pipeline, background_image_bind_group)) =
                &self.background_image_state
            {
                render_pass.set_pipeline(background_image_render_pipeline);
                render_pass.set_bind_group(0, background_image_bind_group, &[]);
                render_pass.draw(0..4, 0..1);
            }

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.render_bind_group, &[]);
            render_pass.draw(0..4, 0..self.points_count as u32);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    pub fn update(&mut self, delta_time: Duration) {
        let delta_time = delta_time.as_secs_f32();
        self.queue
            .write_buffer(&self.delta_time_buffer, 0, bytemuck::bytes_of(&delta_time));

        let mut intensity = if let Some(intensity) = self.volume_provider.poll_volume().unwrap() {
            intensity * self.intensity_multiplier
        } else {
            f32::max(self.last_intensity - delta_time / 20.0, 0.0)
        };

        if intensity > 1.0 {
            let intensity_multiplier = self.intensity_multiplier;
            info!("Intensity: {intensity} Multiplier: {intensity_multiplier}");
            self.intensity_multiplier /= intensity;
            intensity = 1.0;
        } else if intensity != 0.0 {
            self.intensity_multiplier =
                f32::min(self.intensity_multiplier + delta_time / 20.0, 100.0);
        }

        self.queue
            .write_buffer(&self.intensity_buffer, 0, bytemuck::bytes_of(&intensity));
        self.last_intensity = intensity;
    }

    fn create_compute_new_positions_pipeline(
        device: &wgpu::Device,
        points_buffer: &wgpu::Buffer,
        window_size_buffer: &wgpu::Buffer,
        delta_time_buffer: &wgpu::Buffer,
    ) -> (wgpu::ComputePipeline, wgpu::BindGroup) {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compute New Positions Bind Group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute New Positions Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: points_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: window_size_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: delta_time_buffer.as_entire_binding(),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute New Positions Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/compute_new_positions.wgsl").into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compute New Positions Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Compute New Positions Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        (compute_pipeline, bind_group)
    }

    fn create_points(points_count: usize, window_size: WindowSize) -> Vec<Point> {
        let width = window_size.size[0] as u32;
        let height = window_size.size[1] as u32;

        let mut rng = rand::rng();

        let mut points = Vec::<Point>::with_capacity(points_count);
        for _ in 0..points_count {
            let x = rng.random_range(0..width) as f32;
            let y = rng.random_range(0..height) as f32;

            let invertx = rng.random_bool(0.5);
            let inverty = rng.random_bool(0.5);
            let vx = rng.random_range(1.0..3.0) * if invertx { -1.0 } else { 1.0 };
            let vy = rng.random_range(1.0..3.0) * if inverty { -1.0 } else { 1.0 };
            points.push(Point {
                position: [x, y],
                velocity: [vx, vy],
            });
        }

        points
    }

    fn create_msaa_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> wgpu::TextureView {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MSAA Color Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn get_window_pos(&self) -> anyhow::Result<WindowSize> {
        #[cfg(not(target_arch = "wasm32"))]
        #[cfg(target_os = "linux")]
        {
            use anyhow::anyhow;
            use std::process::{self, Command};

            let pid = process::id();

            let json = Command::new("hyprctl")
                .args(["-j", "clients"])
                .output()?
                .stdout;

            let clients: Vec<HyprClient> = serde_json::from_slice(&json)?;

            info!("My pid {pid}");
            info!("Found clients {clients:?}");

            let client = clients
                .iter()
                .find(|c| c.pid == pid)
                .ok_or_else(|| anyhow!("No client found"))?;

            let x = client.at[0];
            let monitor_height = self
                .window
                .current_monitor()
                .ok_or_else(|| anyhow!("No current monitor found"))?
                .size()
                .height as f32;
            let y = monitor_height - (self.window.inner_size().height as f32 + client.at[1]);

            Ok(WindowSize { size: [x, y] })
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct HyprClient {
    pid: u32,
    at: Vec<f32>,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Point {
    position: [f32; 2],
    velocity: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct WindowSize {
    size: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct DeltaTime {
    dt: f32,
}
