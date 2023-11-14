mod quad;
mod renderer;
mod text_renderer;
mod texture;
mod textured_quad;

use renderer::State;
use std::io::Write;
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

pub fn main() {
    pollster::block_on(run_event_loop());
}

async fn run_event_loop() {
    init_env_logger();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut state = State::new(window).await;

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
