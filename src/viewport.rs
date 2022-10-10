use std::time::Duration;

use winit::{
    dpi::PhysicalSize,
    //event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    //event_loop::{ControlFlow, EventLoop},
    window::Window,
};

use noise::{Ease, PNoise1};
use crate::Uniform;

pub struct Viewport {
    size: PhysicalSize<u32>,
    #[allow(unused)]
    scale_factor: f32,
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    uniform: Uniform,
    noise: (PNoise1, PNoise1),
}

impl Viewport {
    pub async fn new(window: &Window) -> Self {
        let size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let instance = wgpu::Instance::new(wgpu::Backends::all());

        // SAFETY: `Viewport` is created in the main thread and `window` remains valid
        // for the lifetime of `surface`.
        let surface = unsafe { instance.create_surface(window) };

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
            )
            .await
            .unwrap();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_supported_formats(&adapter)[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        let uniform = Uniform::new(&device);
        let noise = (
            PNoise1::new(47, 64, 1024, Ease::SmoothStep),
            PNoise1::new(42, 64, 1024, Ease::SmoothStep),
        );

        Self {
            size,
            scale_factor,
            surface,
            device,
            queue,
            config,
            uniform,
            noise,
        }
    }

    pub fn render(&self) -> Result<(), wgpu::SurfaceError> {
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

        let shader = self
            .device
            .create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Viewport::render() pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
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
                    module: &shader,
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
        }

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

    pub fn update(&mut self, dt: Duration) {
        let dt = dt.as_secs_f32();
        let d_left = dt * self.noise.0.next().unwrap();
        let d_right = dt * self.noise.1.next().unwrap();

        self.uniform
            .update_with_delta(&self.queue, [d_left, d_right]);
    }
}
