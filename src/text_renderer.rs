use crate::texture::Texture;
use etagere::*;
use fontdue::Font;
use image::{DynamicImage, GenericImage, Rgba, RgbaImage};
use lru::LruCache;
use wgpu::{
    util::DeviceExt, BindGroup, Buffer, BufferDescriptor, Device, Queue, RenderPass,
    RenderPipeline, TextureFormat,
};
use winit::dpi::PhysicalSize;

// TODO: performs poorly when full, error or something
pub struct Atlas {
    size: f32,
    pub atlas_image: DynamicImage,
    allocations: LruCache<char, AtlasChar>,
    allocator: AtlasAllocator,
    pub is_dirty: bool,
}

#[derive(Debug, Clone)]
struct AtlasChar {
    x_advance: f32,
    y_advance: f32,
    alloc: Allocation,
    pos: (f32, f32),
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub tex_coords: [f32; 2],
    pub color: [f32; 4],
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
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    window_size: [f32; 4], // padding cuz wgsl dumb
}

impl Uniforms {
    fn new(size: PhysicalSize<u32>) -> Self {
        Self {
            window_size: [size.width as f32, size.height as f32, 0.0, 0.0],
        }
    }
}

pub struct TextRenderer {
    render_pipeline: RenderPipeline,
    vertices: Vec<Vertex>,
    vertex_buffer: Buffer,

    indices: Vec<u16>,
    index_buffer: Buffer,

    uniforms_buffer: Buffer,
    uniforms_bind_group: BindGroup,

    texture_bind_group: BindGroup,

    pub atlas: Atlas,
    pub atlas_texture: Texture,

    font: Font,
    font_size: f32,
}

impl TextRenderer {
    pub fn cache_char(&mut self, c: char, queue: &Queue) {
        if self.atlas.allocations.get(&c).is_some() {
            return;
        }

        if c.is_whitespace() {
            return;
        }

        self.atlas.is_dirty = true;
        let (metrics, bitmap) = self.font.rasterize(c, self.font_size);

        let new_glyph_size = size2(metrics.width as i32 + 1, metrics.height as i32 + 1);

        // Evict characters until we can place the new one
        loop {
            if let Some(alloc) = self.atlas.allocator.allocate(new_glyph_size) {
                println!("caching '{}'", c);
                let atlas_char = AtlasChar {
                    x_advance: metrics.advance_width,
                    y_advance: metrics.advance_height,
                    alloc,
                    pos: (metrics.xmin as f32, metrics.ymin as f32),
                };
                let xmin = atlas_char.alloc.rectangle.min.x;
                let ymin = atlas_char.alloc.rectangle.min.y;

                println!("{:?} {:?}", bitmap.len() / metrics.width, metrics.width);

                let mut img = RgbaImage::new(metrics.width as u32, metrics.height as u32);
                for x in 0..metrics.width {
                    for y in 0..metrics.height {
                        img.put_pixel(
                            x as u32,
                            y as u32,
                            Rgba([
                                bitmap[x + y * metrics.width],
                                bitmap[x + y * metrics.width],
                                bitmap[x + y * metrics.width],
                                bitmap[x + y * metrics.width],
                            ]),
                        );
                    }
                }

                // why no worky
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        aspect: wgpu::TextureAspect::All,
                        texture: &self.atlas_texture.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: xmin as u32,
                            y: ymin as u32,
                            z: 0,
                        },
                    },
                    &img,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * metrics.width as u32),
                        rows_per_image: None,
                    },
                    wgpu::Extent3d {
                        width: img.width(),
                        height: img.height(),
                        depth_or_array_layers: 1,
                    },
                );

                self.atlas.allocations.put(c, atlas_char);

                let mut img_y = alloc.rectangle.min.y;
                for y in 0..metrics.height {
                    let mut img_x = alloc.rectangle.min.x;
                    for x in 0..metrics.width {
                        let r = bitmap[x + y * metrics.width];

                        self.atlas.atlas_image.put_pixel(
                            img_x as u32,
                            img_y as u32,
                            Rgba([r, r, r, r]),
                        );
                        img_x += 1;
                    }
                    img_y += 1;
                }

                return;
            } else {
                let lru = self.atlas.allocations.pop_lru().unwrap();
                self.atlas.allocator.deallocate(lru.1.alloc.id);
                println!("evicting '{}'", lru.0);
            }
        }
    }

    fn generate_img_atlas(size: f32) -> Atlas {
        let allocator = AtlasAllocator::new(size2(size as i32, size as i32));
        let img = RgbaImage::from_pixel(size as u32, size as u32, Rgba([0, 0, 0, 0]));

        Atlas {
            atlas_image: DynamicImage::ImageRgba8(img),
            // TODO: probaby shouldn't be unbounded
            allocations: LruCache::unbounded(),
            size,
            allocator,
            is_dirty: false,
        }
    }

    pub fn new(
        device: &Device,
        queue: &Queue,
        format: &TextureFormat,
        size: PhysicalSize<u32>,
    ) -> Self {
        let font = include_bytes!("../res/JetBrainsMono-Regular.ttf") as &[u8];
        let font_size = 48.0;
        let settings = fontdue::FontSettings {
            scale: font_size,
            ..fontdue::FontSettings::default()
        };
        let font = fontdue::Font::from_bytes(font, settings).unwrap();

        let atlas = Self::generate_img_atlas(1024.0);
        let atlas_texture =
            Texture::from_image(device, queue, &atlas.atlas_image, Some("Atlas image"));

        let texture_bind_group_layout =
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
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_texture.sampler),
                },
            ],
            label: Some("texture_bind_group"),
        });

        let uniforms = Uniforms::new(size);

        let uniforms_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniforms buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniforms_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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
                binding: 0,
                resource: uniforms_buffer.as_entire_binding(),
            }],
            label: Some("uniforms_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../res/text.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&uniforms_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: *format,
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
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let max_chars = 1024;
        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: max_chars * 4 * std::mem::size_of::<Vertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Index Buffer"),
            size: max_chars * 6 * std::mem::size_of::<u16>() as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        TextRenderer {
            render_pipeline,
            vertices: vec![],
            vertex_buffer,

            indices: vec![],
            index_buffer,

            uniforms_buffer,
            uniforms_bind_group,

            texture_bind_group,

            atlas,
            atlas_texture,

            font,
            font_size,
        }
    }

    pub fn update(&mut self, size: PhysicalSize<u32>, queue: &Queue) {
        let uniforms = Uniforms::new(size);
        queue.write_buffer(&self.uniforms_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        let text = "abcdefghijklmnopqrs";

        self.indices = vec![];
        self.vertices = vec![];

        let mut x_start = -200.0;
        let mut y_start = 200.0;

        let mut glyphs_added = 0;
        for c in text.chars() {
            self.cache_char(c, queue);
            let char_coords = self.atlas.allocations.get(&c);

            if let Some(char_coords) = char_coords {
                let alloc_rect = char_coords.alloc.rectangle;
                let char_pos = (alloc_rect.min.x as f32, alloc_rect.min.y as f32);
                // Each atlas character is padded 1 pixel in width and height.
                let char_size = (
                    alloc_rect.width() as f32 - 1.0,
                    alloc_rect.height() as f32 - 1.0,
                );

                let xpos = x_start + char_coords.pos.0;
                let ypos = y_start + char_coords.pos.1;
                let w = char_size.0;
                let h = char_size.1;

                x_start += char_coords.x_advance;
                y_start += char_coords.y_advance;

                let x0 = char_pos.0 / self.atlas.size;
                let x1 = (char_pos.0 + char_size.0) / self.atlas.size;
                let y1 = (char_pos.1 + char_size.1) / self.atlas.size;
                let y0 = char_pos.1 / self.atlas.size;

                let start = 4 * glyphs_added;
                self.indices.push(start);
                self.indices.push(start + 1);
                self.indices.push(start + 2);
                self.indices.push(start);
                self.indices.push(start + 2);
                self.indices.push(start + 3);

                self.vertices.push(Vertex {
                    pos: [xpos, ypos + h], // 0
                    tex_coords: [x0, y0],
                    color: [1.0, 1.0, 1.0, 1.0],
                });
                self.vertices.push(Vertex {
                    pos: [xpos, ypos], // 1
                    tex_coords: [x0, y1],
                    color: [1.0, 1.0, 1.0, 1.0],
                });
                self.vertices.push(Vertex {
                    pos: [xpos + w, ypos], // 2
                    tex_coords: [x1, y1],
                    color: [1.0, 1.0, 1.0, 1.0],
                });
                self.vertices.push(Vertex {
                    pos: [xpos + w, ypos + h], // 3
                    tex_coords: [x1, y0],
                    color: [1.0, 1.0, 1.0, 1.0],
                });

                glyphs_added += 1;
            }
        }

        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&self.indices));
    }

    pub fn prepare(&mut self, _device: &Device, _queue: &Queue) {}

    pub fn render<'rpass>(&'rpass self, rpass: &mut RenderPass<'rpass>) {
        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, &self.uniforms_bind_group, &[]);
        rpass.set_bind_group(1, &self.texture_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..self.indices.len() as u32, 0, 0..1_u32);
    }
}
