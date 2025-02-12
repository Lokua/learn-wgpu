use env_logger::{Builder, Env};
use winit::window::Window;
use winit::{
    error::EventLoopError,
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

pub async fn run() -> Result<(), EventLoopError> {
    init_logger();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Learn WGPU")
        .build(&event_loop)
        .unwrap();

    let mut state = State::new(&window).await;

    // Calling helps us avoid manually tracking if the surface is
    // configured or not (it can become invalidated for example
    // when changing windows - me thinks)
    // Ummmmmm....or https://github.com/sotrh/learn-wgpu/issues/585
    state.resize(state.size);

    event_loop.run(move |event, control_flow| match event {
        Event::WindowEvent { event, window_id } => {
            if window_id != state.window().id() {
                return;
            }

            if state.input(&event) {
                return;
            }

            match event {
                WindowEvent::CloseRequested => control_flow.exit(),
                WindowEvent::KeyboardInput { event, .. } => {
                    on_keyboard_input(&mut state, &event, control_flow);
                }
                WindowEvent::Resized(physical_size) => {
                    state.resize(physical_size);
                }
                WindowEvent::RedrawRequested => {
                    on_redraw_requested(&mut state, control_flow);
                }
                _ => {}
            }
        }
        _ => {}
    })
}

fn on_keyboard_input<'a>(
    _state: &mut State<'a>,
    event: &KeyEvent,
    control_flow: &winit::event_loop::EventLoopWindowTarget<()>,
) {
    match event {
        KeyEvent {
            state: ElementState::Pressed,
            physical_key: PhysicalKey::Code(code),
            ..
        } => match code {
            KeyCode::Escape => control_flow.exit(),
            // Handle other keys...
            _ => {}
        },
        _ => {}
    }
}

fn on_redraw_requested<'a>(
    state: &mut State<'a>,
    control_flow: &winit::event_loop::EventLoopWindowTarget<()>,
) {
    // This tells winit that we want another frame after this one
    state.window().request_redraw();

    state.update();

    match state.render() {
        Ok(_) => {}
        // Reconfigure the surface if it's lost or outdated
        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
            state.resize(state.size)
        }

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

#[allow(dead_code)]
struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_configuration: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: &'a Window,
    clear_color: wgpu::Color,
    render_pipelines: Vec<wgpu::RenderPipeline>,
    active_render_pipeline_index: usize,
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

        // The surface is the part of the window that we draw to.
        // We need it to draw directly to the screen
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                // power_preference has two variants: LowPower and
                // HighPerformance. LowPower will pick an adapter that favors
                // battery life, such as an integrated GPU. HighPerformance will
                // pick an adapter for more power-hungry yet more performant
                // GPU's, such as a dedicated graphics card. WGPU will favor
                // LowPower if there is no adapter for the HighPerformance
                // option.
                power_preference: wgpu::PowerPreference::default(),

                // The compatible_surface field tells wgpu to find an adapter
                // that can present to the supplied surface.
                compatible_surface: Some(&surface),

                // The force_fallback_adapter forces wgpu to pick an adapter
                // that will work on all hardware. This usually means that the
                // rendering backend will use a "software" system instead of
                // hardware such as a GPU.
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

        let surface_configuration = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,

            // present_mode uses wgpu::PresentMode enum, which determines
            // how to sync the surface with the display.
            // For the sake of simplicity, we select the first available
            // option. If you do not want runtime selection,
            // `PresentMode::Fifo` will cap the display rate at the display's
            // framerate. This is essentially VSync.
            // This mode is guaranteed to be supported on all platforms.
            //
            // There are other options, and you can see all of them in the docs:
            // https://docs.rs/wgpu/latest/wgpu/enum.PresentMode.html
            //
            // `PresentMode::AutoVsync` and `PresentMode::AutoNoVsync` have
            // fallback support and therefore will work on all platforms.
            present_mode: surface_caps.present_modes[0],

            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shader.wgsl").into(),
                ),
            });

        let render_pipeline = Self::create_render_pipeline(
            &device,
            &surface_configuration,
            &shader,
        );

        let render_pipeline2 = Self::create_render_pipeline(
            &device,
            &surface_configuration,
            &device.create_shader_module(wgpu::include_wgsl!("shader2.wgsl")),
        );

        Self {
            surface,
            device,
            queue,
            surface_configuration,
            size,
            window,
            render_pipelines: vec![render_pipeline, render_pipeline2],
            active_render_pipeline_index: 0,
            clear_color: wgpu::Color {
                r: 0.03,
                g: 0.03,
                b: 0.03,
                a: 1.0,
            },
        }
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
        surface_configuration: &wgpu::SurfaceConfiguration,
        shader: &wgpu::ShaderModule,
    ) -> wgpu::RenderPipeline {
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options:
                        wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_configuration.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options:
                        wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    // Setting this to anything other than Fill requires
                    // `Features::NON_FILL_POLYGON_MODE`
                    polygon_mode: wgpu::PolygonMode::Fill,
                    // Requires `Features::DEPTH_CLIP_CONTROL`
                    unclipped_depth: false,
                    // Requires `Features::CONSERVATIVE_RASTERIZATION`
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            });

        render_pipeline
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.surface_configuration.width = new_size.width;
            self.surface_configuration.height = new_size.height;
            self.surface
                .configure(&self.device, &self.surface_configuration);
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x / self.size.width as f64;
                let y = position.y / self.size.height as f64;
                self.clear_color = wgpu::Color {
                    r: x,
                    g: y,
                    b: (x + y) / 2.0,
                    a: 1.0,
                };
                true
            }
            WindowEvent::KeyboardInput { event, .. } => match event {
                KeyEvent {
                    state: ElementState::Pressed,
                    physical_key: PhysicalKey::Code(code),
                    ..
                } => match code {
                    KeyCode::Space => {
                        self.active_render_pipeline_index =
                            (self.active_render_pipeline_index + 1) % 2;
                        true
                    }
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        }
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
            // Begin_render_pass() borrows encoder mutably (aka &mut self). We
            // can't call encoder.finish() until we release that mutable borrow.
            // The block tells Rust to drop any variables within it when the
            // code leaves that scope, thus releasing the mutable borrow on
            // encoder and allowing us to finish() it.
            //
            // If you don't like the {}, you can also use drop(render_pass) to
            // achieve the same effect.
            let mut render_pass =
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(
                        wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(self.clear_color),
                                store: wgpu::StoreOp::Store,
                            },
                        },
                    )],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

            let active_render_pipeline =
                &self.render_pipelines[self.active_render_pipeline_index];

            render_pass.set_pipeline(&active_render_pipeline);
            render_pass.draw(0..3, 0..1);
        }

        // Submit will accept anything that implements `IntoIter`
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub fn init_logger() {
    let filter = Env::default().default_filter_or("learn_wgpu=info");
    Builder::from_env(filter).init();
}
