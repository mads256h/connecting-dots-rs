use winit::event_loop::EventLoop;

mod app;
mod state;

mod volume_providers;

use app::App;

pub fn run(background_image: Option<String>, #[cfg(not(target_arch = "wasm32"))] class: String) -> anyhow::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    #[cfg(target_arch = "wasm32")]
    console_log::init_with_level(log::Level::Info).unwrap_throw();

    let event_loop = EventLoop::with_user_event().build()?;
    let mut app = App::new(
        #[cfg(target_arch = "wasm32")]
        &event_loop,
        background_image,
        #[cfg(not(target_arch = "wasm32"))]
        class,
    );
  
    event_loop.run_app(&mut app)?;

    Ok(())
}
