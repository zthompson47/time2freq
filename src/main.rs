use std::{path::PathBuf, time::Instant};

use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait};
use pollster::block_on;
use winit::{
    event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use egui_wgpu::wgpu;

use time2freq::{audio::AudioPlayer, gui::Gui, Viewport};

#[derive(Parser)]
struct Cli {
    #[arg(short, long, default_value_t = 100)]
    latency_ms: usize,
    #[arg(short, long, default_value_t = 4096)]
    chunk_size: usize,
    song: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    let _log = tailog::init();
    log::info!("Starting...");

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let mut last_render_time = Instant::now();
    let mut viewport = block_on(Viewport::new(&window));

    let mut gui = Gui::new(&viewport.device, &event_loop, viewport.config.format);

    let audio_device = cpal::default_host().default_output_device().unwrap();
    let audio_config = audio_device.default_output_config().unwrap();

    let mut audio = match audio_config.sample_format() {
        cpal::SampleFormat::I8 => AudioPlayer::new::<i8>(
            &audio_device,
            &audio_config.into(),
            cli.latency_ms,
            cli.chunk_size,
        ),
        cpal::SampleFormat::F32 => AudioPlayer::new::<f32>(
            &audio_device,
            &audio_config.into(),
            cli.latency_ms,
            cli.chunk_size,
        ),
        _ => panic!("unsupported format"),
    }
    .unwrap();
    //audio.play(&std::env::args().nth(1).expect("Expected song file"));
    audio.play(cli.song);

    event_loop.run(move |event, _, control_flow| match event {
        Event::DeviceEvent {
            event: DeviceEvent::MouseMotion { delta: _delta },
            ..
        } => (),

        Event::WindowEvent {
            window_id,
            ref event,
        } if window_id == window.id() => {
            if gui.process_event(event) {
                return;
            }

            match event {
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
            }
        }

        Event::RedrawRequested(window_id) if window_id == window.id() => {
            let now = Instant::now();
            let dt = now - last_render_time;
            last_render_time = now;

            // Try to scale and normalize the levels for max visual effect.
            let (mut rms, mut loudness) = audio.rms(dt);

            rms[0] = (1. - 20. * rms[0].log10() / -20.).clamp(-1., 1.);
            rms[1] = (1. - 20. * rms[1].log10() / -20.).clamp(-1., 1.);

            loudness = (10f32.powf(loudness / 20.) * 20.) * 2. - 1.;

            log::trace!("got RMS in redraw() {rms:?} {loudness}");

            //let egui_input = gui.window_state.take_egui_input(&window);

            viewport.update(dt, (rms, loudness));
            //viewport.render(egui_input).unwrap();
            viewport.render(&mut gui, &window).unwrap();
        }

        Event::MainEventsCleared => window.request_redraw(),

        _ => (),
    });
}
