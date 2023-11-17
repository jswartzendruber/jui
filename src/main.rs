mod layout;
mod quad;
mod renderer;
mod text_renderer;
mod texture;
mod textured_quad;

use layout::SceneRoot;

pub fn main() {
    pollster::block_on(SceneRoot::run());
}
