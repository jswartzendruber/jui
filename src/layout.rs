use crate::renderer::State;
use std::time::{Duration, Instant};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[derive(Debug)]
pub struct Bbox {
    pub min: (f32, f32),
    pub max: (f32, f32),
}

impl Bbox {
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self {
            min: (x0, y0),
            max: (x1, y1),
        }
    }

    pub fn width(&self) -> f32 {
        self.max.0 - self.min.0
    }

    pub fn height(&self) -> f32 {
        self.max.1 - self.min.1
    }

    pub fn center(&self) -> (f32, f32) {
        (
            (self.min.0 + self.max.0) / 2.0,
            (self.min.1 + self.max.1) / 2.0,
        )
    }
}

#[derive(Debug)]
pub enum Thing {
    Text {
        text: String,
        text_color: [f32; 4],
        background_color: [f32; 4],
    },
    Quad {
        color: [f32; 4],
    },
    TexturedQuad {},

    Hbox(Hbox),
    Vbox(Vbox),
}

#[derive(Debug)]
pub struct Hbox {
    elements: Vec<Thing>,
}

impl Hbox {
    pub fn new(elements: Vec<Thing>) -> Self {
        Self { elements }
    }
}

#[derive(Debug)]
pub struct Vbox {
    elements: Vec<Thing>,
}

impl Vbox {
    pub fn new(elements: Vec<Thing>) -> Self {
        Self { elements }
    }
}
trait Container {
    fn layout(&self, state: &mut State, parent_size: Bbox) {
        for (i, elem) in self.elements().iter().enumerate() {
            let child_bbox = if self.is_hbox() {
                let child_index = i;
                let child_width = parent_size.width() / self.elements().len() as f32;
                let x0 = parent_size.min.0 + child_width * child_index as f32;

                Bbox::new(x0, parent_size.min.1, x0 + child_width, parent_size.max.1)
            } else {
                let child_index = self.elements().len() - i - 1;
                let child_height = parent_size.height() / self.elements().len() as f32;
                let y0 = parent_size.min.1 + child_height * child_index as f32;

                Bbox::new(parent_size.min.0, y0, parent_size.max.0, y0 + child_height)
            };

            let child_bbox_center = child_bbox.center();

            match elem {
                Thing::Text {
                    text,
                    text_color,
                    background_color,
                } => {
                    state
                        .quad_renderer
                        .add_instance(*background_color, &child_bbox);
                    state.text_renderer.add_string_to_batch(
                        text,
                        &state.queue,
                        child_bbox_center.0,
                        child_bbox_center.1,
                        *text_color,
                    );
                }
                Thing::Quad { color } => state.quad_renderer.add_instance(*color, &child_bbox),
                Thing::TexturedQuad {} => state.textured_quad_renderer.add_instance(&child_bbox),
                Thing::Hbox(hbox) => hbox.layout(state, child_bbox),
                Thing::Vbox(vbox) => vbox.layout(state, child_bbox),
            }
        }
    }

    fn elements(&self) -> &Vec<Thing>;

    fn is_hbox(&self) -> bool;
}

impl Container for Hbox {
    fn elements(&self) -> &Vec<Thing> {
        &self.elements
    }

    fn is_hbox(&self) -> bool {
        true
    }
}

impl Container for Vbox {
    fn elements(&self) -> &Vec<Thing> {
        &self.elements
    }

    fn is_hbox(&self) -> bool {
        false
    }
}

pub struct SceneRoot {
    root: Box<dyn Container>,

    state: State,
    last_frame_time: Duration,
}

impl SceneRoot {
    pub async fn run() {
        let event_loop = EventLoop::new().unwrap();
        let window = WindowBuilder::new().build(&event_loop).unwrap();
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut scene_root = SceneRoot {
            root: Box::new(Hbox::new(vec![
                Thing::Vbox(Vbox::new(vec![
                    Thing::Text {
                        text: "FT: 0.0".to_string(),
                        text_color: [1.0, 0.0, 0.0, 1.0],
                        background_color: [1.0, 1.0, 1.0, 1.0],
                    },
                    Thing::Hbox(Hbox::new(vec![
                        Thing::Quad {
                            color: [0.0, 0.5, 0.5, 1.0],
                        },
                        Thing::Quad {
                            color: [0.5, 0.5, 0.0, 1.0],
                        },
                    ])),
                ])),
                Thing::Quad {
                    color: [0.0, 0.0, 1.0, 1.0],
                },
                Thing::Quad {
                    color: [1.0, 0.0, 0.0, 1.0],
                },
                Thing::Vbox(Vbox::new(vec![
                    Thing::Quad {
                        color: [0.0, 1.0, 0.0, 1.0],
                    },
                    Thing::TexturedQuad {},
                    Thing::Quad {
                        color: [0.0, 1.0, 1.0, 1.0],
                    },
                    Thing::Text {
                        text: "asdfa".to_string(),
                        text_color: [0.0, 0.0, 0.0, 1.0],
                        background_color: [1.0, 1.0, 1.0, 1.0],
                    },
                ])),
            ])),
            state: State::new(window).await,
            last_frame_time: Duration::from_nanos(0),
        };

        event_loop
            .run(move |event, elwt| match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    elwt.exit();
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    scene_root.state.resize(size);
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    let frame_start = Instant::now();

                    scene_root.update();

                    match scene_root.state.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            scene_root.state.resize(scene_root.state.size)
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            elwt.exit();
                        }
                        Err(wgpu::SurfaceError::Timeout) => {}
                    }

                    let frame_time = Instant::now().duration_since(frame_start);
                    scene_root.last_frame_time = frame_time;
                }
                Event::AboutToWait => {
                    scene_root.state.window.request_redraw();
                }
                _ => {}
            })
            .unwrap();
    }

    pub fn update(&mut self) {
        self.state.clear();
        let window_size = self.state.window.inner_size();
        self.root.layout(
            &mut self.state,
            Bbox::new(
                0.0,
                0.0,
                window_size.width as f32,
                window_size.height as f32,
            ),
        );
        self.state.update();
    }
}
