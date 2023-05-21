use std::time::{Duration, Instant};

use winit::{dpi::PhysicalSize, window::Window};

use crate::{gui::Gui, Uniform, wgpu};
use noise::{Ease, PNoise1};

pub struct Viewport {
    size: PhysicalSize<u32>,
    #[allow(unused)]
    scale_factor: f32,
    surface: wgpu::Surface,
    pub device: wgpu::Device,
    queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    shader: wgpu::ShaderModule,
    pub uniform: Uniform,
    #[allow(unused)]
    noise: (PNoise1, PNoise1),
    start_time: Instant,
}

impl Viewport {
    pub async fn new(window: &Window) -> Self {
        let size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        //let instance = wgpu::Instance::new(wgpu::Backends::all());
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::default()
        });

        //let instance = wgpu::Instance::new(wgpu::Backends::GL);
        //let instance = wgpu::Instance::new(wgpu::Backends::VULKAN);

        // SAFETY: `Viewport` is created in the main thread and `window` remains valid
        // for the lifetime of `surface`.
        let surface = unsafe { instance.create_surface(window).unwrap() };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
                //Some(std::path::Path::new("/home/zach/projects/wgpu/time2freq/trace")),
            )
            .await
            .unwrap();

        let capabilities = surface.get_capabilities(&adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: capabilities.formats[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![capabilities.formats[0]],
        };


        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let uniform = Uniform::new(&device);
        let noise = (
            PNoise1::new(47, 16, 1024, Ease::SmoothStep),
            PNoise1::new(42, 16, 1024, Ease::SmoothStep),
        );

        Self {
            size,
            scale_factor,
            surface,
            device,
            queue,
            config,
            shader,
            uniform,
            noise,
            start_time: Instant::now(),
        }
    }

    pub fn render(
        &self,
        gui: &mut Gui,
        window: &winit::window::Window,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Viewport::render() encoder"),
            });

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[self.uniform.bind_group_layout()],
                push_constant_ranges: &[],
            });

        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Viewport::render() pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Viewport::render() render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::default()),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_bind_group(0, self.uniform.bind_group(), &[]);
            render_pass.set_pipeline(&pipeline);
            render_pass.draw(0..4, 0..1);
            render_pass.draw(4..8, 0..1);
            //render_pass.draw(0..8, 0..1);
        }

        gui.render(
            window,
            &self.device,
            &self.queue,
            &mut encoder,
            &self.config,
            &view,
        );

        self.queue.submit(Some(encoder.finish()));
        output.present();

        Ok(())
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn update(&mut self, _dt: Duration, level: ([f32; 2], f32)) {
        //let level_left = self.noise.0.next().unwrap();
        //let level_right = self.noise.1.next().unwrap();
        //self.uniform.raw.level = [level_left, level_right];
        self.uniform.raw.level = level.0;
        self.uniform.raw.loudness = level.1;
        self.uniform.raw.screen_size = [self.config.width as f32, self.config.height as f32];
        self.uniform.raw.time = (Instant::now() - self.start_time).as_secs_f32();

        self.uniform.write_buffer(&self.queue);
    }
}
