use crate::{layout::Bbox, texture::Texture};
use etagere::*;
use freetype::{face::LoadFlag, Face};
use image::{DynamicImage, Rgba, RgbaImage};
use lru::LruCache;
use wgpu::{
    util::DeviceExt, BindGroup, Buffer, BufferDescriptor, Device, Queue, RenderPass,
    RenderPipeline, TextureFormat,
};
use winit::dpi::PhysicalSize;

pub struct Atlas {
    size: f32,
    atlas_image: DynamicImage,
    allocations: LruCache<char, AtlasChar>,
    allocator: AtlasAllocator,
}

#[derive(Debug, Clone)]
struct AtlasChar {
    advance: (f32, f32),
    pos: (f32, f32),
    size: (f32, f32),
    alloc: Option<Allocation>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pos: [f32; 2],
    tex_coords: [f32; 2],
    text_color: [f32; 4],
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

    atlas: Atlas,
    atlas_texture: Texture,

    // Hold onto this in case we want to load any new font faces
    _freetype: freetype::Library,
    font_size: isize,
    face: Face,
}

impl TextRenderer {
    fn cache_char(&mut self, c: char, queue: &Queue) {
        if self.atlas.allocations.get(&c).is_some() {
            return;
        }

        self.face.load_char(c as usize, LoadFlag::RENDER).unwrap();
        let glyph = self.face.glyph();

        let width = glyph.bitmap().width() as u32;
        let height = glyph.bitmap().rows() as u32;

        if c.is_whitespace() {
            let atlas_char = AtlasChar {
                advance: (
                    glyph.advance().x as f32 / 64.0,
                    glyph.advance().y as f32 / 64.0,
                ),
                size: (width as f32, height as f32),
                pos: (0.0, 0.0),
                alloc: None,
            };
            self.atlas.allocations.put(c, atlas_char);
            return;
        }

        if width == 0 || height == 0 {
            return;
        }

        // Pad allocation 1 pixel on each side to avoid bleeding
        let mut img = RgbaImage::from_pixel(width + 2, height + 2, Rgba([0, 0, 0, 0]));

        for x in 0..width {
            for y in 0..height {
                img.put_pixel(
                    x + 1,
                    y + 1,
                    Rgba([
                        255,
                        255,
                        255,
                        glyph.bitmap().buffer()[(x + y * width) as usize],
                    ]),
                );
            }
        }

        // Evict characters until we can place the new one
        loop {
            if let Some(alloc) = self
                .atlas
                .allocator
                .allocate(size2(img.width() as i32, img.height() as i32))
            {
                let atlas_char = AtlasChar {
                    advance: (
                        glyph.advance().x as f32 / 64.0,
                        glyph.advance().y as f32 / 64.0,
                    ),
                    size: (width as f32, height as f32),
                    pos: (
                        glyph.bitmap_left() as f32,
                        glyph.bitmap_top() as f32 - height as f32,
                    ),
                    alloc: Some(alloc),
                };
                let xmin = alloc.rectangle.min.x;
                let ymin = alloc.rectangle.min.y;

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
                        bytes_per_row: Some(4 * img.width()),
                        rows_per_image: None,
                    },
                    wgpu::Extent3d {
                        width: img.width(),
                        height: img.height(),
                        depth_or_array_layers: 1,
                    },
                );

                self.atlas.allocations.put(c, atlas_char);
                return;
            } else {
                let lru = self.atlas.allocations.pop_lru().unwrap();
                if let Some(alloc) = lru.1.alloc {
                    self.atlas.allocator.deallocate(alloc.id);
                }
            }
        }
    }

    fn generate_img_atlas(size: f32) -> Atlas {
        let allocator = AtlasAllocator::new(size2(size as i32, size as i32));
        let img = RgbaImage::from_pixel(size as u32, size as u32, Rgba([0, 0, 0, 0]));

        Atlas {
            atlas_image: DynamicImage::ImageRgba8(img),
            allocations: LruCache::unbounded(),
            size,
            allocator,
        }
    }

    pub fn new(
        device: &Device,
        queue: &Queue,
        format: &TextureFormat,
        size: PhysicalSize<u32>,
    ) -> Self {
        let font_size = 18;
        let lib = freetype::Library::init().unwrap();
        let face = lib.new_face("res/iosevka-extended.ttf", 0).unwrap();
        face.set_char_size(font_size * 64, 0, 0, 0).unwrap();

        let atlas = Self::generate_img_atlas(256.0);
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

        let max_chars = 4096;
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

            _freetype: lib,
            font_size,
            face,
        }
    }

    pub fn clear(&mut self) {
        self.indices.clear();
        self.vertices.clear();
    }

    pub fn update(&mut self, size: PhysicalSize<u32>, queue: &Queue) {
        let uniforms = Uniforms::new(size);
        queue.write_buffer(&self.uniforms_buffer, 0, bytemuck::cast_slice(&[uniforms]));
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&self.indices));
    }

    /// Add a string of text for rendering.
    /// (x, y) is the vertical and horizontal center of where the text will be placed.
    pub fn add_string_to_batch_centered(
        &mut self,
        s: &str,
        queue: &Queue,
        x: f32,
        y: f32,
        text_color: [f32; 4],
    ) {
        // calculate bottom, fudge it a bit because off center things look more centered
        let mut y = (y - ((self.font_size as f32 * 0.8) / 2.0)).floor();

        // calculate left
        let mut text_len = 0.0;
        for c in s.chars() {
            self.cache_char(c, queue);
            if let Some(glyph) = self.atlas.allocations.get(&c) {
                text_len += glyph.advance.0;
            }
        }
        let mut x = (x - (text_len / 2.0)).floor();

        // text is placed using x,y, the bottom left corner of the start of the text.
        for c in s.chars() {
            self.cache_char(c, queue);
            self.add_char_to_batch(c, &mut x, &mut y, text_color);
        }
    }

    pub fn line_height(&self) -> f32 {
        self.font_size as f32
    }

    /// Add a string of text for rendering.
    /// (x, y) is the top left corner of where the text will be placed.
    /// If wrap_bbox exists, will wrap the line to stay inside the bbox. Must not fail.
    ///
    /// Returns the vertical space it used up.
    /// When wrapping, this could be multiple lines * line_height.
    pub fn add_string_to_batch(
        &mut self,
        s: &str,
        queue: &Queue,
        x: f32,
        y: f32,
        text_color: [f32; 4],
        wrap_bbox: Option<&Bbox>,
    ) -> f32 {
        // calculate bottom, fudge it a bit because off center things look more centered
        let mut y = y.floor() - self.line_height();
        let y_start = y;

        // calculate left
        let mut x = x.floor();

        // calculate length for wrapping
        if let Some(wrap_bbox) = wrap_bbox {
            let mut len = 0.0;
            for c in s.chars() {
                self.cache_char(c, queue);
                let atlas_char = self.atlas.allocations.get(&c).unwrap();
                len += atlas_char.advance.0;
            }

            let num_lines = (len / wrap_bbox.width()).ceil() as usize;

            for _ in 0..num_lines {
                // text is placed using x,y, the bottom left corner of the start of the text.
                x = wrap_bbox.min.0;
                for c in s.chars() {
                    if x > wrap_bbox.max.0 {
                        x = wrap_bbox.min.0;
                        y -= self.line_height();
                    } else if !wrap_bbox.inside((x, y)) {
                        // Exit early if we've run out of space. No point continuing.
                        return y_start - y;
                    }
                    self.cache_char(c, queue);
                    self.add_char_to_batch(c, &mut x, &mut y, text_color);
                }
                y -= self.line_height();
            }
            y_start - y
        } else {
            // text is placed using x,y, the bottom left corner of the start of the text.
            for c in s.chars() {
                self.cache_char(c, queue);
                self.add_char_to_batch(c, &mut x, &mut y, text_color);
            }
            y_start - y
        }
    }

    /// If wrap_bbox exists, will wrap the line to stay inside the bbox. Must not fail.
    pub fn add_multiline_string_to_batch(
        &mut self,
        text: &Vec<String>,
        queue: &Queue,
        x: f32,
        y: f32,
        text_color: [f32; 4],
        wrap_bbox: Option<&Bbox>,
    ) {
        let mut y = y;
        for line in text {
            let line = self.add_string_to_batch(line, queue, x, y, text_color, wrap_bbox);
            y -= line;
        }
    }

    /// Internal details, you should use add_string_to_batch
    fn add_char_to_batch(
        &mut self,
        c: char,
        x_start: &mut f32,
        y_start: &mut f32,
        text_color: [f32; 4],
    ) {
        if let Some(glyph) = self.atlas.allocations.get(&c) {
            let x = *x_start + glyph.pos.0;
            let y = *y_start + glyph.pos.1;
            let w = glyph.size.0;
            let h = glyph.size.1;

            *x_start += glyph.advance.0;
            *y_start += glyph.advance.1;

            if let Some(alloc_rect) = glyph.alloc {
                // Undo padding
                let glyph_pos_in_atlas = (
                    alloc_rect.rectangle.min.x as f32 + 1.0,
                    alloc_rect.rectangle.min.y as f32 + 1.0,
                );

                let x0 = glyph_pos_in_atlas.0 / self.atlas.size;
                let x1 = (glyph_pos_in_atlas.0 + glyph.size.0) / self.atlas.size;
                let y1 = (glyph_pos_in_atlas.1 + glyph.size.1) / self.atlas.size;
                let y0 = glyph_pos_in_atlas.1 / self.atlas.size;

                let start = (4 * (self.indices.len() / 6)) as u16;
                self.indices.push(start);
                self.indices.push(start + 1);
                self.indices.push(start + 2);
                self.indices.push(start);
                self.indices.push(start + 2);
                self.indices.push(start + 3);

                self.vertices.push(Vertex {
                    pos: [x, y], // 0
                    tex_coords: [x0, y1],
                    text_color,
                });
                self.vertices.push(Vertex {
                    pos: [x + w, y], // 1
                    tex_coords: [x1, y1],
                    text_color,
                });
                self.vertices.push(Vertex {
                    pos: [x + w, y + h], // 2
                    tex_coords: [x1, y0],
                    text_color,
                });
                self.vertices.push(Vertex {
                    pos: [x, y + h], // 3
                    tex_coords: [x0, y0],
                    text_color,
                });
            }
        }
    }

    pub fn render<'rpass>(&'rpass self, rpass: &mut RenderPass<'rpass>) {
        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, &self.uniforms_bind_group, &[]);
        rpass.set_bind_group(1, &self.texture_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..self.indices.len() as u32, 0, 0..1_u32);
    }
}
