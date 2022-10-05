#![allow(unused)]
use std::time::Instant;
use winit::{
    event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let mut last_render_time = Instant::now();

    event_loop.run(move |event, _, control_flow| match event {
        Event::DeviceEvent {
            event: DeviceEvent::MouseMotion { delta },
            ..
        } => (),

        Event::WindowEvent {
            window_id,
            ref event,
        } if window_id == window.id() => match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,

            WindowEvent::Resized(physical_size) => (),

            WindowEvent::ScaleFactorChanged { new_inner_size, .. } => (),

            _ => (),
        },

        Event::RedrawRequested(window_id) if window_id == window.id() => {
            let now = Instant::now();
            let dt = now - last_render_time;
            last_render_time = now;
        },

        Event::MainEventsCleared => window.request_redraw(),

        _ => (),
    });
}
