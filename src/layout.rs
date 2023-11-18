use crate::renderer::State;
use std::time::{Duration, Instant};
use winit::{
    dpi::PhysicalSize,
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

pub enum Thing {
    Text { text: String },
    Quad { color: [f32; 4] },
    TexturedQuad {},

    Hbox(Hbox),
}

pub trait Container {
    fn layout(&mut self, state: &mut State);
    fn add_element(&mut self, element: Thing);
    fn elements(&mut self) -> &mut Vec<Thing>;
    fn bbox(&mut self) -> &mut Bbox;
}

pub struct Hbox {
    elements: Vec<Thing>,
    bbox: Bbox,
}

impl Hbox {
    pub fn new(bbox: Bbox) -> Self {
        Self {
            elements: vec![],
            bbox,
        }
    }
}

impl Container for Hbox {
    fn layout(&mut self, state: &mut State) {
        let child_width = self.bbox.width() / self.elements.len() as f32;
        let child_height = self.bbox.height();

        for (i, elem) in self.elements.iter().enumerate() {
            let child_bbox = Bbox::new(
                child_width * i as f32,
                0.0,
                (child_width * i as f32) + child_width,
                child_height,
            );
            let child_bbox_center = child_bbox.center();

            match elem {
                Thing::Text { text } => state.text_renderer.add_string_to_batch(
                    text,
                    &state.queue,
                    child_bbox_center.0,
                    child_bbox_center.1,
                ),
                Thing::Quad { color } => state.quad_renderer.add_instance(*color, &child_bbox),
                Thing::TexturedQuad {} => state.textured_quad_renderer.add_instance(&child_bbox),
                Thing::Hbox(_) => todo!(),
            }
        }
    }

    fn add_element(&mut self, element: Thing) {
        self.elements.push(element);
    }

    fn elements(&mut self) -> &mut Vec<Thing> {
        &mut self.elements
    }

    fn bbox(&mut self) -> &mut Bbox {
        &mut self.bbox
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
            root: Box::new(Hbox::new(Bbox::new(
                0.0,
                0.0,
                window.inner_size().width as f32,
                window.inner_size().height as f32,
            ))),
            state: State::new(window).await,
            last_frame_time: Duration::from_nanos(0),
        };

        scene_root.root.add_element(Thing::Text {
            text: "SIDE 1".to_string(),
        });
        scene_root.root.add_element(Thing::TexturedQuad {});
        scene_root.root.add_element(Thing::Text {
            text: format!("Last frame time: {:?}", scene_root.last_frame_time),
        });
        scene_root.root.add_element(Thing::Quad {
            color: [0.0, 0.0, 1.0, 1.0],
        });

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
        self.root.elements()[2] = Thing::Text {
            text: format!("FT: {:.2}", self.last_frame_time.as_micros() as f32 / 1000.0),
        };
        self.update_window_size(self.state.window.inner_size());
        self.root.layout(&mut self.state);
        self.state.update();
    }

    fn update_window_size(&mut self, size: PhysicalSize<u32>) {
        *self.root.bbox() = Bbox::new(0.0, 0.0, size.width as f32, size.height as f32);
    }
}
