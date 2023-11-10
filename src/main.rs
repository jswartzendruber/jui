mod quad;

use etagere::*;
use fontdue::Font;
use image::{Rgba, RgbaImage};
use quad::Quad;
use std::io::Write;
use std::iter;
use wgpu::{util::DeviceExt, BindGroup, Buffer};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    bbox: [f32; 4],
    color: [f32; 4],
    sigma: f32,
    corner_radius: f32,
}

impl Instance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 9]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RectUniforms {
    size: [f32; 2],
    origin: [f32; 2],
    background_color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    viewport_size: [f32; 2],
}

impl Uniforms {
    fn new(x: f32, y: f32) -> Self {
        Self {
            viewport_size: [x, y],
        }
    }

    fn update(&mut self, x: f32, y: f32) {
        self.viewport_size = [x, y];
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

fn init_env_logger() {
    env_logger::builder()
        .format(|buf, record| {
            let mut style = buf.style();

            match record.level() {
                log::Level::Error => style.set_color(env_logger::fmt::Color::Red),
                log::Level::Warn => style.set_color(env_logger::fmt::Color::Yellow),
                log::Level::Info => style.set_color(env_logger::fmt::Color::White),
                log::Level::Debug => style.set_color(env_logger::fmt::Color::Rgb(50, 50, 50)),
                log::Level::Trace => style.set_color(env_logger::fmt::Color::Cyan),
            };

            writeln!(
                buf,
                "{}",
                style.value(format!(
                    "[{}:{}] {}: {}",
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0),
                    record.level(),
                    record.args()
                ))
            )
        })
        .init();
}

fn generate_img_atlas(font: &Font, font_size: f32) -> RgbaImage {
    let mut atlas = AtlasAllocator::new(size2(1024, 1024));
    let mut img = RgbaImage::from_pixel(1024, 1024, Rgba([0, 0, 0, 255]));

    for glyph in font.chars() {
        let (metrics, bitmap) = font.rasterize_subpixel(*glyph.0, font_size);
        let slot = atlas.allocate(size2(metrics.width as i32, metrics.height as i32));

        if let Some(rect) = slot {
            let rect = rect.rectangle;

            let mut img_y = rect.min.y;
            for y in 0..metrics.height {
                let mut img_x = rect.min.x;
                for x in (0..metrics.width * 3).step_by(3) {
                    let r = bitmap[x + y * metrics.width * 3];
                    let g = bitmap[x + 1 + y * metrics.width * 3];
                    let b = bitmap[x + 2 + y * metrics.width * 3];

                    img.put_pixel(img_x as u32, img_y as u32, Rgba([r, g, b, 255]));
                    img_x += 1;
                }
                img_y += 1;
            }
        }
    }

    img
}

pub fn main() {
    let font = include_bytes!("../res/Roboto-Regular.ttf") as &[u8];
    let settings = fontdue::FontSettings {
        scale: 72.0,
        ..fontdue::FontSettings::default()
    };
    let font = fontdue::Font::from_bytes(font, settings).unwrap();

    let atlas_img = generate_img_atlas(&font, 36.0);
    // let mut atlas_png = File::create("font_atlas.png").unwrap();
    // atlas_img
    //     .write_to(&mut atlas_png, ImageOutputFormat::Png)
    //     .unwrap();

    pollster::block_on(app(atlas_img));
}

async fn app(atlas_img: RgbaImage) {
    init_env_logger();
    run(atlas_img).await;
}

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    quad: Quad,
    rectangle_uniforms: RectUniforms,
    rectangle_uniforms_buffer: Buffer,
    rectangle_uniforms_bind_group: BindGroup,
    uniforms: Uniforms,
    uniforms_buffer: Buffer,
    uniforms_bind_group: BindGroup,
    instances: Vec<Instance>,
    instance_buffer: Buffer,
    window: Window,
    atlas_bind_group: BindGroup,
}

impl State {
    async fn new(window: Window, atlas_img: RgbaImage) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
            flags: wgpu::InstanceFlags::default(),
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
        });

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let atlas_dimensions = atlas_img.dimensions();
        let atlas_size = wgpu::Extent3d {
            width: atlas_dimensions.0,
            height: atlas_dimensions.1,
            depth_or_array_layers: 1,
        };
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: atlas_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("diffuse_texture"),
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_img,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * atlas_dimensions.0),
                rows_per_image: Some(atlas_dimensions.1),
            },
            atlas_size,
        );
        let atlas_texture_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                ],
                label: Some("texture_bind_group_layout"),
            });
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
            label: Some("atlas_bind_group"),
        });

        let rectangle_uniforms = RectUniforms {
            size: [1.0, 1.0],
            origin: [0.0, 0.0],
            background_color: [1.0, 0.0, 0.0, 1.0],
        };

        let rectangle_uniforms_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rectangle_uniforms buffer"),
                contents: bytemuck::cast_slice(&[rectangle_uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let rectangle_uniforms_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("rectangle_uniforms_bind_group_layout"),
            });

        let rectangle_uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &rectangle_uniforms_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: rectangle_uniforms_buffer.as_entire_binding(),
            }],
            label: Some("uniforms_bind_group"),
        });

        let instances = vec![Instance {
            bbox: [300.0, 250.0, 350.0, 300.0],
            color: [0.0, 0.0, 1.0, 1.0],
            sigma: 10.0,
            corner_radius: 20.0,
        }];
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let size = window.inner_size();
        let uniforms = Uniforms::new(size.width as f32, size.height as f32);

        let uniforms_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniforms buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniforms_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("uniforms_bind_group_layout"),
            });

        let uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniforms_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 1,
                resource: uniforms_buffer.as_entire_binding(),
            }],
            label: Some("uniforms_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../res/shader.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &rectangle_uniforms_bind_group_layout,
                    &uniforms_bind_group_layout,
                    &atlas_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc(), Instance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::POLYGON_MODE_LINE
                // or Features::POLYGON_MODE_POINT
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: true,
            },
            multiview: None,
        });

        let quad = quad::Quad::new(&device);

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            quad,
            uniforms,
            uniforms_buffer,
            uniforms_bind_group,
            rectangle_uniforms,
            rectangle_uniforms_buffer,
            rectangle_uniforms_bind_group,
            instances,
            instance_buffer,
            window,
            atlas_bind_group,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.uniforms
                .update(new_size.width as f32, new_size.height as f32);
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn update(&mut self) {
        self.queue.write_buffer(
            &self.rectangle_uniforms_buffer,
            0,
            bytemuck::cast_slice(&[self.rectangle_uniforms]),
        );
        self.queue.write_buffer(
            &self.uniforms_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
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
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
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
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.rectangle_uniforms_bind_group, &[]);
            render_pass.set_bind_group(1, &self.uniforms_bind_group, &[]);
            render_pass.set_bind_group(2, &self.atlas_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.quad.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass
                .set_index_buffer(self.quad.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.quad.num_indices, 0, 0..self.instances.len() as u32);
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub async fn run(atlas_img: RgbaImage) {
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut state = State::new(window, atlas_img).await;

    event_loop
        .run(move |event, elwt| {
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    window_id: _,
                } => {
                    log::info!("Closing window.");
                    elwt.exit();
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    window_id: _,
                } => {
                    log::info!("Resizing window.");
                    state.resize(size);
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    window_id: _,
                } => {
                    state.update();
                    match state.render() {
                        Ok(_) => {}
                        // Reconfigure the surface if it's lost or outdated
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            state.resize(state.size)
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            log::debug!("Out of memory, closing.");
                            elwt.exit();
                        }
                        Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                    }
                }
                Event::AboutToWait => {
                    state.window.request_redraw();
                }
                _ => {}
            }
        })
        .unwrap();
}
