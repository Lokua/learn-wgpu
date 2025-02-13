use env_logger::{Builder, Env};
use wgpu::util::DeviceExt;
use winit::window::Window;
use winit::{
    error::EventLoopError,
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

mod texture;

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

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
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
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>()
                        as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.0868241, 0.49240386, 0.0],
        tex_coords: [0.4131759, 0.99240386],
    }, // A
    Vertex {
        position: [-0.49513406, 0.06958647, 0.0],
        tex_coords: [0.0048659444, 0.56958647],
    }, // B
    Vertex {
        position: [-0.21918549, -0.44939706, 0.0],
        tex_coords: [0.28081453, 0.05060294],
    }, // C
    Vertex {
        position: [0.35966998, -0.3473291, 0.0],
        tex_coords: [0.85967, 0.1526709],
    }, // D
    Vertex {
        position: [0.44147372, 0.2347359, 0.0],
        tex_coords: [0.9414737, 0.7347359],
    }, // E
];

const INDICES: &[u16] = &[
    0, 1, 4, //
    1, 2, 4, //
    2, 3, 4, /* padding */ 0,
];

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
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    n_indices: u32,
    diffuse_bind_group: wgpu::BindGroup,
}

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

        surface.configure(&device, &surface_configuration);

        let diffuse_texture = texture::Texture::from_bytes(
            &device,
            &queue,
            include_bytes!("g25.png"),
            Some("Diffuse Texture"),
        )
        .unwrap();

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float {
                                filterable: true,
                            },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(
                            wgpu::SamplerBindingType::Filtering,
                        ),
                        count: None,
                    },
                ],
                label: Some("Texture Bind Group Layour"),
            });

        let diffuse_bind_group =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: &[
                    // @group(0) @binding(0)
                    // var t_diffuse: texture_2d<f32>;
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &diffuse_texture.view,
                        ),
                    },
                    // @group(0) @binding(1)
                    // var s_diffuse: sampler;
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(
                            &diffuse_texture.sampler,
                        ),
                    },
                ],
                label: Some("diffuse_bind_group"),
            });

        // To access the create_buffer_init method on wgpu::Device, we'll have
        // to import the DeviceExt
        // (https://docs.rs/wgpu/latest/wgpu/util/trait.DeviceExt.html#tymethod.create_buffer_init)
        // extension trait. For more information on extension traits, check out
        // this article: http://xion.io/post/code/rust-extension-traits.html.
        let pentagon_vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Pentagon Vertex Buffer"),
                contents: bytemuck::cast_slice(VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

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
            &texture_bind_group_layout,
        );

        let render_pipeline2 = Self::create_render_pipeline(
            &device,
            &surface_configuration,
            &device.create_shader_module(wgpu::include_wgsl!("shader2.wgsl")),
            &texture_bind_group_layout,
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
            vertex_buffer: pentagon_vertex_buffer,
            index_buffer,
            n_indices: INDICES.len() as u32,
            diffuse_bind_group,
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
        texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::desc()],
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

    fn window(&self) -> &Window {
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
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                self.index_buffer.slice(..),
                wgpu::IndexFormat::Uint16,
            );
            render_pass.draw_indexed(0..self.n_indices, 0, 0..1);
        }

        // Submit will accept anything that implements `IntoIter`
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

fn init_logger() {
    let filter = Env::default().default_filter_or("learn_wgpu=info");
    Builder::from_env(filter).init();
}
