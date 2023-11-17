use crate::renderer::State;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

pub struct Bbox {
    min: (f32, f32),
    max: (f32, f32),
}

impl Bbox {
    fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self {
            min: (x0, y0),
            max: (x1, y1),
        }
    }

    fn width(&self) -> f32 {
        self.max.0 - self.min.0
    }

    fn height(&self) -> f32 {
        self.max.1 - self.min.1
    }

    fn center(&self) -> (f32, f32) {
        (
            (self.min.0 + self.max.0) / 2.0,
            (self.min.1 + self.max.1) / 2.0,
        )
    }
}

pub enum Thing {
    Text { text: String },
    Quad { color: [f32; 4] },

    Hbox(Hbox),
}

pub trait Container {
    fn render(&mut self, state: &mut State);
    fn add_element(&mut self, element: Thing);
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
    fn render(&mut self, state: &mut State) {
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
                Thing::Quad { .. } => todo!(),
                Thing::Hbox(_) => todo!(),
            }
        }
    }

    fn add_element(&mut self, element: Thing) {
        self.elements.push(element);
    }
}

pub struct SceneRoot {
    root: Box<dyn Container>,

    state: State,
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
        };

        scene_root.root.add_element(Thing::Text {
            text: "SIDE 1".to_string(),
        });
        scene_root.root.add_element(Thing::Text {
            text: "SIDE 2".to_string(),
        });
        scene_root.root.add_element(Thing::Text {
            text: "SIDE 3".to_string(),
        });
        scene_root.root.add_element(Thing::Text {
            text: "SIDE 4".to_string(),
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
                    scene_root.state.text_renderer.start_text_batch();
                    scene_root.state.update();
                    scene_root.root.render(&mut scene_root.state);
                    scene_root
                        .state
                        .text_renderer
                        .end_text_batch(&scene_root.state.queue);

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
                }
                Event::AboutToWait => {
                    scene_root.state.window.request_redraw();
                }
                _ => {}
            })
            .unwrap();
    }
}
