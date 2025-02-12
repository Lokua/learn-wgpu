use winit::window::Window;
use winit::{
    error::EventLoopError,
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

pub async fn run() -> Result<(), EventLoopError> {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Learn WGPU")
        .build(&event_loop)
        .unwrap();

    let mut state = State::new(&window).await;

    // Calling helps us avoid manually tracking if the surface is 
    // configured or not (it can become invalidated for example 
    // when chaning windows - me thinks)
    state.resize(state.size);

    event_loop.run(move |event, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == state.window().id() => {
            if !state.input(event) {
                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                state: ElementState::Pressed,
                                physical_key: PhysicalKey::Code(KeyCode::Escape),
                                ..
                            },
                        ..
                    } => control_flow.exit(),
                    WindowEvent::Resized(physical_size) => {
                        state.resize(*physical_size);
                    }
                    WindowEvent::RedrawRequested => {
                        // This tells winit that we want another frame after this one
                        state.window().request_redraw();
            
                        state.update();
                        match state.render() {
                            Ok(_) => {}
                            // Reconfigure the surface if it's lost or outdated
                            Err(
                                wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated,
                            ) => state.resize(state.size),

                            // The system is out of memory, we should probably quit
                            Err(wgpu::SurfaceError::OutOfMemory | wgpu::SurfaceError::Other) => {
                                log::error!("OutOfMemory");
                                control_flow.exit();
                            }
            
                            // This happens when the a frame takes too long to present
                            Err(wgpu::SurfaceError::Timeout) => {
                                log::warn!("Surface timeout")
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    })
}

#[allow(dead_code)]
struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: &'a Window,
}

#[allow(dead_code)]
impl<'a> State<'a> {
    // Creating some of the wgpu types requires async code
    async fn new(window: &'a Window) -> State<'a> {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        // he surface is the part of the window that we draw to.
        // We need it to draw directly to the screen
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                // power_preference has two variants: LowPower and HighPerformance.
                // LowPower will pick an adapter that favors battery life, such as an
                // integrated GPU. HighPerformance will pick an adapter for more
                // power-hungry yet more performant GPU's, such as a dedicated graphics
                // card. WGPU will favor LowPower if there is no adapter for the
                // HighPerformance option.
                power_preference: wgpu::PowerPreference::default(),

                // The compatible_surface field tells wgpu to find an adapter that can
                // present to the supplied surface.
                compatible_surface: Some(&surface),

                // The force_fallback_adapter forces wgpu to pick an adapter that will
                // work on all hardware. This usually means that the rendering backend
                // will use a "software" system instead of hardware such as a GPU.
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    // You can get a list of features supported by your device
                    // using adapter.features() or device.features().
                    // https://docs.rs/wgpu/latest/wgpu/struct.Features.html
                    required_features: wgpu::Features::empty(),

                    // https://docs.rs/wgpu/latest/wgpu/struct.Limits.html
                    required_limits: wgpu::Limits::default(),

                    label: None,

                    // https://wgpu.rs/doc/wgpu/enum.MemoryHints.html
                    memory_hints: Default::default(),
                },
                // Trace path
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,

            // present_mode uses wgpu::PresentMode enum, which determines
            // how to sync the surface with the display.
            // For the sake of simplicity, we select the first available option.
            // If you do not want runtime selection, `PresentMode::Fifo` will
            // cap the display rate at the display's framerate.
            // This is essentially VSync. This mode is guaranteed to be supported on all platforms.
            // There are other options, and you can see all of them in the docs
            // https://docs.rs/wgpu/latest/wgpu/enum.PresentMode.html
            // `PresentMode::AutoVsync` and `PresentMode::AutoNoVsync` have fallback support and therefore will work on all platforms.
            present_mode: surface_caps.present_modes[0],

            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        Self {
            surface,
            device,
            queue,
            config,
            size,
            window,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        } 
    }

    #[allow(unused_variables)]
    fn input(&mut self, event: &WindowEvent) -> bool {
        false
    }

    fn update(&mut self) {}

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            },
        );

        {
            // begin_render_pass() borrows encoder mutably (aka &mut self).
            // We can't call encoder.finish() until we release that mutable borrow. The block tells Rust to drop any variables within it when the code leaves that scope, thus releasing the mutable borrow on encoder and allowing us to finish() it. If you don't like the {}, you can also use drop(render_pass) to achieve the same effect.
            let _render_pass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(
                        wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.03,
                                    g: 0.03,
                                    b: 0.03,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        },
                    )],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
        }

        // submit will accept anything that implements IntoIter
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
