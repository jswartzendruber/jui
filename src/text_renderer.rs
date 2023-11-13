use crate::texture::Texture;
use etagere::*;
use fontdue::Font;
use image::{Rgba, RgbaImage};
use std::collections::HashMap;
use wgpu::{
    util::DeviceExt, BindGroup, Buffer, Device, Queue, RenderPass, RenderPipeline, TextureFormat,
};
use winit::dpi::PhysicalSize;

struct Atlas {
    atlas_size_x: f32,
    atlas_size_y: f32,
    atlas_image: RgbaImage,
    allocator_map: HashMap<char, Option<Allocation>>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub tex_coords: [f32; 2],
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

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Char {
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

impl Char {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Char>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/*
D--C
|\ |
| \|
A--B
*/
#[rustfmt::skip]
const VERTICES: &[Vertex] = &[
    Vertex { pos: [-1.0, -1.0], tex_coords: [0.0, 1.0] }, // A
    Vertex { pos: [ 1.0, -1.0], tex_coords: [1.0, 1.0] }, // B
    Vertex { pos: [ 1.0,  1.0], tex_coords: [1.0, 0.0] }, // C
    Vertex { pos: [-1.0,  1.0], tex_coords: [0.0, 0.0] }, // D
];

const INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

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
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    num_indices: u32,

    instances: Vec<Char>,
    instance_buffer: Buffer,

    uniforms_buffer: Buffer,
    uniforms_bind_group: BindGroup,

    texture_bind_group: BindGroup,

    atlas: Atlas,
    pub character: char,
}

impl TextRenderer {
    fn generate_img_atlas(font: &Font, font_size: f32) -> Atlas {
        let atlas_size_x = 1024.0;
        let atlas_size_y = 1024.0;
        let mut atlas = AtlasAllocator::new(size2(atlas_size_x as i32, atlas_size_y as i32));
        let mut img = RgbaImage::from_pixel(1024, 1024, Rgba([0, 0, 0, 255]));
        let mut chars_to_allocs = HashMap::new();

        for glyph in font.chars() {
            let (metrics, bitmap) = font.rasterize(*glyph.0, font_size);
            let slot = atlas.allocate(size2(metrics.width as i32 + 1, metrics.height as i32 + 1));
            chars_to_allocs.insert(*glyph.0, slot);

            if let Some(rect) = slot {
                let rect = rect.rectangle;

                let mut img_y = rect.min.y;
                for y in 0..metrics.height {
                    let mut img_x = rect.min.x;
                    for x in 0..metrics.width {
                        let r = bitmap[x + y * metrics.width];

                        img.put_pixel(img_x as u32, img_y as u32, Rgba([r, r, r, r]));
                        img_x += 1;
                    }
                    img_y += 1;
                }
            }
        }

        Atlas {
            atlas_image: img,
            allocator_map: chars_to_allocs,
            atlas_size_x,
            atlas_size_y,
        }
    }

    fn gen_atlas() -> Atlas {
        let font = include_bytes!("../res/Roboto-Regular.ttf") as &[u8];
        let settings = fontdue::FontSettings {
            scale: 72.0,
            ..fontdue::FontSettings::default()
        };
        let font = fontdue::Font::from_bytes(font, settings).unwrap();

        Self::generate_img_atlas(&font, 36.0)
    }

    pub fn new(
        device: &Device,
        queue: &Queue,
        format: &TextureFormat,
        size: PhysicalSize<u32>,
    ) -> Self {
        let atlas = Self::gen_atlas();
        let img = image::DynamicImage::ImageRgba8(atlas.atlas_image.clone());
        let image_texture =
            Texture::from_image(device, queue, &img, Some("Atlas image")).unwrap();

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
                    resource: wgpu::BindingResource::TextureView(&image_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&image_texture.sampler),
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
            label: Some("Quad Shader"),
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
                buffers: &[Vertex::desc(), Char::desc()],
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
                alpha_to_coverage_enabled: true,
            },
            multiview: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let num_indices = INDICES.len() as u32;

        let instances = vec![
            Char {
                origin: [0.5, -0.75],
                size: [0.024, 0.032],
                color: [1.0, 0.0, 0.0, 1.0],
            },
            Char {
                origin: [0.55, -0.752],
                size: [0.024, 0.032],
                color: [1.0, 0.0, 0.0, 1.0],
            },
        ];
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::VERTEX,
        });

        TextRenderer {
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,

            instances,
            instance_buffer,

            uniforms_buffer,
            uniforms_bind_group,

            texture_bind_group,

            atlas,
            character: 'J',
        }
    }

    pub fn update(&mut self, size: PhysicalSize<u32>, queue: &Queue) {
        let uniforms = Uniforms::new(size);
        queue.write_buffer(&self.uniforms_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        let character_coordinates = self.atlas.allocator_map.get(&self.character);
        if let Some(Some(char_coords)) = character_coordinates {
            let atlas_size = (self.atlas.atlas_size_x, self.atlas.atlas_size_y);
            let char_pos = (
                char_coords.rectangle.min.x as f32,
                char_coords.rectangle.min.y as f32,
            );
            let char_size = (
                char_coords.rectangle.width() as f32 - 1.0,
                char_coords.rectangle.height() as f32 - 1.0,
            );

            let x0 = char_pos.0 / atlas_size.0;
            let x1 = (char_pos.0 + char_size.0) / atlas_size.0;
            let y0 = (char_pos.1 + char_size.1) / atlas_size.1;
            let y1 = char_pos.1 / atlas_size.1;

            #[rustfmt::skip]
            let vertices: &[Vertex] = &[
                Vertex { pos: VERTICES[0].pos, tex_coords: [x0, y0], },
                Vertex { pos: VERTICES[1].pos, tex_coords: [x1, y0], },
                Vertex { pos: VERTICES[2].pos, tex_coords: [x1, y1], },
                Vertex { pos: VERTICES[3].pos, tex_coords: [x0, y1], },
            ];
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));
        }
    }

    pub fn prepare(&mut self, _device: &Device, _queue: &Queue) {}

    pub fn render<'rpass>(&'rpass self, rpass: &mut RenderPass<'rpass>) {
        rpass.set_pipeline(&self.render_pipeline);
        rpass.set_bind_group(0, &self.uniforms_bind_group, &[]);
        rpass.set_bind_group(1, &self.texture_bind_group, &[]);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        rpass.draw_indexed(0..self.num_indices, 0, 0..self.instances.len() as u32);
    }
}
