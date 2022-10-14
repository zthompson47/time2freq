use std::time::Instant;

use pollster::block_on;
use winit::{
    event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use time2freq::{audio::AudioPlayer, Viewport};

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let mut last_render_time = Instant::now();
    let mut viewport = block_on(Viewport::new(&window));

    //let audio = AudioPlayer::new(150, 44100, 2).unwrap();
    let audio = AudioPlayer::new(150, 48000, 2).unwrap();
    audio.play(&std::env::args().nth(1).expect("Expected song file"));

    event_loop.run(move |event, _, control_flow| match event {
        Event::DeviceEvent {
            event: DeviceEvent::MouseMotion { delta: _delta },
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

            WindowEvent::Resized(physical_size) => viewport.resize(*physical_size),

            WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                viewport.resize(**new_inner_size)
            }

            WindowEvent::CursorMoved {
                device_id: _,
                position,
                ..
            } => viewport.uniform.raw.mouse_pos = [position.x as f32, position.y as f32],

            _ => (),
        },

        Event::RedrawRequested(window_id) if window_id == window.id() => {
            let now = Instant::now();
            let dt = now - last_render_time;
            last_render_time = now;

            viewport.update(dt);
            viewport.render().unwrap();
        }

        Event::MainEventsCleared => window.request_redraw(),

        _ => (),
    });
}
