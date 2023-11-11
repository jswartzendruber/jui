mod quad;
mod renderer;

use etagere::*;
use fontdue::Font;
use image::{Rgba, RgbaImage};
use renderer::State;
use std::io::Write;
use std::{collections::HashMap, fs::File};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

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

struct Atlas {
    allocator: AtlasAllocator,
    atlas_size_x: f32,
    atlas_size_y: f32,
    atlas_image: RgbaImage,
    allocator_map: HashMap<char, Option<Allocation>>,
}

fn generate_img_atlas(font: &Font, font_size: f32) -> Atlas {
    let atlas_size_x = 1024.0;
    let atlas_size_y = 1024.0;
    let mut atlas = AtlasAllocator::new(size2(atlas_size_x as i32, atlas_size_y as i32));
    let mut img = RgbaImage::from_pixel(1024, 1024, Rgba([0, 0, 0, 255]));
    let mut chars_to_allocs = HashMap::new();

    for glyph in font.chars() {
        if glyph.0 != &'A' {
            continue;
        }
        let (metrics, bitmap) = font.rasterize_subpixel(*glyph.0, font_size);
        let slot = atlas.allocate(size2(metrics.width as i32, metrics.height as i32));
        chars_to_allocs.insert(*glyph.0, slot);

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

    Atlas {
        allocator: atlas,
        atlas_image: img,
        allocator_map: chars_to_allocs,
        atlas_size_x,
        atlas_size_y,
    }
}

pub fn main() {
    let font = include_bytes!("../res/Roboto-Regular.ttf") as &[u8];
    let settings = fontdue::FontSettings {
        scale: 72.0,
        ..fontdue::FontSettings::default()
    };
    let font = fontdue::Font::from_bytes(font, settings).unwrap();

    let atlas = generate_img_atlas(&font, 36.0);
    // let mut atlas_png = File::create("font_atlas.png").unwrap();
    // atlas
    //     .atlas_image
    //     .write_to(&mut atlas_png, image::ImageOutputFormat::Png)
    //     .unwrap();

    pollster::block_on(run_event_loop(&atlas));
}

async fn run_event_loop(atlas: &Atlas) {
    init_env_logger();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut state = State::new(window, atlas).await;

    event_loop
        .run(move |event, elwt| match event {
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
        })
        .unwrap();
}
