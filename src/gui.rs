use crate::wgpu;

#[derive(Default)]
struct GuiState {
    repaint: bool,
}

pub struct Gui {
    context: egui::Context,
    renderer: egui_wgpu::Renderer,
    pub window_state: egui_winit::State,
    state: GuiState,
}

impl Gui {
    pub fn new(
        device: &wgpu::Device,
        event_loop: &winit::event_loop::EventLoop<()>,
        output_color_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            context: egui::Context::default(),
            renderer: egui_wgpu::Renderer::new(device, output_color_format, None, 1),
            window_state: egui_winit::State::new(event_loop),
            state: GuiState::default(),
        }
    }

    pub fn process_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        let response = self.window_state.on_event(&self.context, event);
        self.state.repaint = response.repaint;

        response.consumed
    }

    pub fn render(
        &mut self,
        window: &winit::window::Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        config: &wgpu::SurfaceConfiguration,
        view: &wgpu::TextureView,
    ) {
        let input = self.window_state.take_egui_input(window);
        let output = self.context.run(input, |ctx| {
            egui::Area::new("testitout").show(ctx, |ui| {
                ui.label("Hup Hup Hup");
            });
        });

        let clipped_primitives: Vec<egui::epaint::ClippedPrimitive> =
            self.context.tessellate(output.shapes);

        for (id, image_delta) in &output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }

        let screen_descriptor = egui_wgpu::renderer::ScreenDescriptor {
            size_in_pixels: [config.width, config.height],
            pixels_per_point: 2.0, //self.scale_factor,
        };

        self.renderer.update_buffers(
            device,
            queue,
            encoder,
            clipped_primitives.as_slice(),
            &screen_descriptor,
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            self.renderer.render(
                &mut render_pass,
                clipped_primitives.as_slice(),
                &screen_descriptor,
            );
        }
    }
}
