#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    unused_assignments,
    unreachable_code
)]
#![feature(path_file_prefix)]

use image::EncodableLayout;
use std::mem::size_of;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct UniformData {
    height_resolution: f32,
    texture_width: f32,
    texture_height: f32,
    window_width: f32,
    window_height: f32,
    utime: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Sprite {
    top_left_position_x: f32,
    top_left_position_y: f32,
    top_left_tex_coords_x: f32,
    top_left_tex_coords_y: f32,
    width: f32,
    height: f32,
    depth_base: f32,
    origin_offset_y: f32,
}

#[repr(C)]
struct TextureData {
    top_left_tex_pos: (u32, u32),
    size: (u32, u32),
    origin_offset: u32,
}

#[derive(Clone, Debug)]
struct Time {
    start_time: std::time::Instant,
    utime: f32,
    delta_time: f32,
}

#[derive(Clone, Debug)]
enum CloseStatus {
    Running,
    Closed,
}

struct PixelRenderer {
    surface: wgpu::Surface,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    shader: wgpu::ShaderModule,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,

    texture_atlas_array: wgpu::Texture,
    depth_stencil_texture: wgpu::Texture,

    uniform_data: UniformData,
    start_time: std::time::Instant,
    delta_time: f32,
    last_frame_time: f32,

    uniform_buffer: wgpu::Buffer,
    storage_buffer: wgpu::Buffer,

    sorted_sprites: Vec<Sprite>,
}

impl PixelRenderer {
    fn new(window: &winit::window::Window, height_resolution: u32) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let surface = unsafe { instance.create_surface(window).unwrap() };
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            }))
            .unwrap();
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: adapter.features(),
                limits: adapter.limits(),
            },
            None,
        ))
        .unwrap();
        let surface_texture_format = surface.get_capabilities(&adapter).formats[0];

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_texture_format,
            width: window.inner_size().width,
            height: window.inner_size().height,
            // this mode is always available, you can also get the first one in the present mode vec from capabilities
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode: surface.get_capabilities(&adapter).alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &surface_config);

        // TEXTURE LOADING
        let mut dir = std::env::current_dir().unwrap();
        dir.push("src/res/texture_pack.png");
        let texture_data = image::io::Reader::open(dir)
            .unwrap()
            .decode()
            .unwrap()
            .into_rgba8();

        let texture_atlas_array = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: texture_data.width(),
                height: texture_data.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTextureBase {
                texture: &texture_atlas_array,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            texture_data.as_bytes(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * texture_data.width()),
                rows_per_image: Some(texture_data.height()),
            },
            texture_atlas_array.size(),
        );

        // depth texture
        let depth_stencil_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: surface_config.width,
                height: surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32FloatStencil8,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: size_of::<UniformData>() as u64,
            usage: wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let start_time = std::time::Instant::now();

        let uniform_data = UniformData {
            height_resolution: (height_resolution / 2) as f32,
            texture_width: texture_data.width() as f32,
            texture_height: texture_data.height() as f32,
            window_width: window.inner_size().width as f32,
            window_height: window.inner_size().height as f32,
            utime: 0.0,
        };
        queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[uniform_data]));

        let storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 512,
            usage: wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // BIND GROUP
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                // our texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // some uniform data ig
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // vertex data
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_atlas_array.create_view(
                        &wgpu::TextureViewDescriptor {
                            label: None,
                            format: None,
                            dimension: None,
                            aspect: wgpu::TextureAspect::All,
                            base_mip_level: 0,
                            mip_level_count: None,
                            base_array_layer: 0,
                            array_layer_count: None,
                        },
                    )),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniform_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &storage_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        // SHADER
        let shader = device.create_shader_module(wgpu::include_wgsl!("./shader.wgsl"));

        // PIPELINE
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                }),
            ),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32FloatStencil8,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_texture_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        Self {
            surface,
            surface_config,
            device,
            queue,
            shader,
            pipeline,
            texture_atlas_array,
            bind_group,
            depth_stencil_texture,
            uniform_data,
            start_time,
            delta_time: 0.0,
            last_frame_time: 0.0,

            uniform_buffer,
            storage_buffer,
            sorted_sprites: vec![],
        }
    }

    fn update_window_size(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.height = new_size.height;
        self.surface_config.width = new_size.width;
        self.surface.configure(&self.device, &self.surface_config);
        self.depth_stencil_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: self.surface_config.width,
                height: self.surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32FloatStencil8,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.uniform_data.window_height = new_size.height as _;
        self.uniform_data.window_width = new_size.width as _;
    }

    fn update_time(&mut self) {
        self.uniform_data.utime = self.start_time.elapsed().as_secs_f32();
        self.delta_time = self.uniform_data.utime - self.last_frame_time;
        self.last_frame_time = self.uniform_data.utime;
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniform_data]),
        );
    }

    fn load_sprites(&mut self, sprites: &[Sprite]) {
        // copy, sort, then submit
        if self.sorted_sprites.len() < sprites.len() {
            self.sorted_sprites.resize(
                sprites.len(),
                Sprite {
                    top_left_position_x: 0.0,
                    top_left_position_y: 0.0,
                    top_left_tex_coords_x: 0.0,
                    top_left_tex_coords_y: 0.0,
                    width: 32.0,
                    height: 32.0,
                    depth_base: 0.0,
                    origin_offset_y: 32.0,
                },
            );
        }
        self.sorted_sprites[0..sprites.len()].clone_from_slice(sprites);
        self.sorted_sprites[0..sprites.len()].sort_unstable_by(|a, b| {
            let depth_a = 0.2
                * (((a.top_left_position_y - a.origin_offset_y)
                    / self.uniform_data.height_resolution)
                    + 1.0)
                / 2.0
                + a.depth_base;
            let depth_b = 0.2
                * (((b.top_left_position_y - b.origin_offset_y)
                    / self.uniform_data.height_resolution)
                    + 1.0)
                / 2.0
                + b.depth_base;
            depth_b.total_cmp(&depth_a)
        });
        self.queue.write_buffer(
            &self.storage_buffer,
            0,
            bytemuck::cast_slice(&self.sorted_sprites),
        );
    }

    fn render(&mut self) {
        let canvas = self.surface.get_current_texture().unwrap();
        let canvas_view = canvas
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = self
            .depth_stencil_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &canvas_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                    store: false,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: false,
                }),
                stencil_ops: None,
            }),
        });
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..6 * 128, 0..1);
        drop(render_pass);
        self.queue.submit(Some(encoder.finish()));
        canvas.present();
    }
}

// ------------------------------------------------------------------ //
struct Prefab {
    map: std::collections::HashMap<String, TextureData>,
}
impl Prefab {
    fn new() -> Self {
        let mut map: std::collections::HashMap<String, TextureData> =
            std::collections::HashMap::new();
        map.insert(
            "char_main".to_string(),
            TextureData {
                top_left_tex_pos: (0, 0),
                size: (32, 32),
                origin_offset: 32,
            },
        );
        map.insert(
            "char_alt".to_string(),
            TextureData {
                top_left_tex_pos: (0, 32),
                size: (32, 32),
                origin_offset: 32,
            },
        );
        map.insert(
            "bg_tile".to_string(),
            TextureData {
                top_left_tex_pos: (32, 0),
                size: (32, 32),
                origin_offset: 16,
            },
        );
        map.insert(
            "spot".to_string(),
            TextureData {
                top_left_tex_pos: (32, 32),
                size: (32, 32),
                origin_offset: 32,
            },
        );

        Self { map }
    }

    fn gen(&self, name: &str, position: (f32, f32), layer: usize) -> Sprite {
        let tex_data = self.map.get(name).unwrap();
        Sprite {
            top_left_position_x: position.0,
            top_left_position_y: position.1,
            top_left_tex_coords_x: tex_data.top_left_tex_pos.0 as _,
            top_left_tex_coords_y: tex_data.top_left_tex_pos.1 as _,
            width: tex_data.size.0 as _,
            height: tex_data.size.1 as _,
            depth_base: match layer {
                0 => 0.0,
                1 => 0.25,
                2 => 0.5,
                3 => 0.75,
                _ => panic!(),
            },
            origin_offset_y: tex_data.origin_offset as _,
        }
    }
}

#[derive(Clone, Debug)]
struct ArrowKeyState {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}
impl ArrowKeyState {
    fn new() -> Self {
        Self {
            up: false,
            down: false,
            left: false,
            right: false,
        }
    }

    fn to_vector(&self, speed: f32) -> (f32, f32) {
        let x = (self.right as u32 as f32 - self.left as u32 as f32) * speed;
        let y = (self.up as u32 as f32 - self.down as u32 as f32) * speed;
        (x, y)
    }
}

#[derive(Clone, Debug)]
struct PlayerIndex(usize);

fn entry(table: &mut ecs::Table) {
    let vector = table
        .read_resource::<ArrowKeyState>()
        .unwrap()
        .to_vector(30f32);
    let player_index = table.read_resource::<PlayerIndex>().unwrap();
    let player_sprite = table.read::<Sprite>(player_index.0).unwrap();
    let time = table.read_resource::<Time>().unwrap();

    for each in table.read_event::<winit::event::KeyboardInput>().unwrap() {
        if let Some(code) = each.virtual_keycode {
            match code {
                winit::event::VirtualKeyCode::Space => match each.state {
                    winit::event::ElementState::Pressed => {
                        println!("omg uwu");
                    }
                    winit::event::ElementState::Released => {}
                },
                winit::event::VirtualKeyCode::Q => {
                    *table.read_resource::<CloseStatus>().unwrap() = CloseStatus::Closed;
                }
                winit::event::VirtualKeyCode::Right => match each.state {
                    winit::event::ElementState::Pressed => {
                        table.read_resource::<ArrowKeyState>().unwrap().right = true
                    }
                    winit::event::ElementState::Released => {
                        table.read_resource::<ArrowKeyState>().unwrap().right = false
                    }
                },
                winit::event::VirtualKeyCode::Left => match each.state {
                    winit::event::ElementState::Pressed => {
                        table.read_resource::<ArrowKeyState>().unwrap().left = true
                    }
                    winit::event::ElementState::Released => {
                        table.read_resource::<ArrowKeyState>().unwrap().left = false
                    }
                },
                winit::event::VirtualKeyCode::Up => match each.state {
                    winit::event::ElementState::Pressed => {
                        table.read_resource::<ArrowKeyState>().unwrap().up = true
                    }
                    winit::event::ElementState::Released => {
                        table.read_resource::<ArrowKeyState>().unwrap().up = false
                    }
                },
                winit::event::VirtualKeyCode::Down => match each.state {
                    winit::event::ElementState::Pressed => {
                        table.read_resource::<ArrowKeyState>().unwrap().down = true
                    }
                    winit::event::ElementState::Released => {
                        table.read_resource::<ArrowKeyState>().unwrap().down = false
                    }
                },
                _ => (),
            }
        }
    }

    player_sprite.top_left_position_x += vector.0 as f32 * time.delta_time;
    player_sprite.top_left_position_y += vector.1 as f32 * time.delta_time;
}

fn main() {
    // init
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    let mut renderer = PixelRenderer::new(&window, 144);
    let mut ecs = ecs::ECS::new(entry);
    // prep
    ecs.table.register_event::<winit::event::KeyboardInput>();
    ecs.table
        .add_resource::<Time>(Time {
            start_time: std::time::Instant::now(),
            utime: 0.0,
            delta_time: 0.0,
        })
        .unwrap();
    ecs.table.add_resource(CloseStatus::Running).unwrap();
    let prefab = Prefab::new();
    ecs.table.add_resource(ArrowKeyState::new()).unwrap();
    let player_index = ecs.table.insert_new(prefab.gen("char_main", (0.0, 0.0), 1));
    ecs.table.add_resource(PlayerIndex(player_index)).unwrap();
    ecs.table.insert_new(prefab.gen("char_alt", (0.0, 0.0), 1));
    ecs.table.insert_new(prefab.gen("bg_tile", (0.0, 0.0), 2));
    // loop
    event_loop.run(move |event, _, control_flow| {
        match ecs.table.read_resource::<CloseStatus>().unwrap() {
            CloseStatus::Running => control_flow.set_poll(),
            CloseStatus::Closed => control_flow.set_exit(),
        }
        renderer.update_time();
        *ecs.table.read_resource::<Time>().unwrap() = Time {
            start_time: renderer.start_time,
            utime: renderer.uniform_data.utime,
            delta_time: renderer.delta_time,
        };
        match event {
            winit::event::Event::WindowEvent { event, .. } => match event {
                winit::event::WindowEvent::Resized(size) => {
                    renderer.update_window_size(size);
                    window.request_redraw();
                }
                winit::event::WindowEvent::CloseRequested => control_flow.set_exit(),
                winit::event::WindowEvent::KeyboardInput { input, .. } => {
                    if ecs
                        .table
                        .read_event::<winit::event::KeyboardInput>()
                        .unwrap()
                        .last()
                        != Some(&input)
                    {
                        ecs.table.add_event(input.clone());
                    }
                }
                _ => (),
            },
            winit::event::Event::MainEventsCleared => window.request_redraw(),
            winit::event::Event::RedrawRequested(_) => {
                renderer.render();
            }
            _ => (),
        }
        ecs.tick();
        // all hinged on this special column
        renderer.load_sprites(&ecs.table.query_raw::<Sprite>().unwrap());
    });
}
