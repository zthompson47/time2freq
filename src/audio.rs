use std::{path::PathBuf, thread, time::Duration};

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    FromSample, SizedSample,
};
use crossbeam::channel;
use ebur128::{EbuR128, Mode};
use rubato::Resampler as _;

use crate::resources::{AudioFile, CopyMethod};

type ChannelBuf = Vec<Vec<f32>>;

struct Resampler {
    inner: rubato::SincFixedIn<f32>,
    buf_in: ChannelBuf,
    buf_out: ChannelBuf,
}

pub struct AudioPlayer {
    #[allow(unused)]
    stream: cpal::Stream,
    tx_play_song: channel::Sender<PathBuf>,
    lvl_cons: rtrb::Consumer<f32>,
    rms: [f32; 2],
    #[allow(dead_code)]
    rms_buf: Option<ChannelBuf>,
    sample_rate: u32,
    #[allow(dead_code)]
    channels: u32,
    ebur128: EbuR128,
}

impl AudioPlayer {
    pub fn new<T>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        latency_ms: usize,
        chunk_size: usize,
    ) -> anyhow::Result<Self>
    where
        T: SizedSample + FromSample<f32>,
    {
        let device_sample_rate = config.sample_rate.0;
        let device_channels = config.channels as u32;

        let latency_frames =
            (latency_ms as f32 * device_sample_rate as f32 / 1000.0).round() as u32;
        let latency_samples = (latency_frames * device_channels) as usize;

        log::info!("device sample rate: {device_sample_rate}");
        log::info!("device channels: {device_channels}");
        log::info!("latency frames: {latency_frames}");
        log::info!("latency samples: {latency_samples}");

        let (mut device_send, mut device_recv) = rtrb::RingBuffer::<f32>::new(latency_samples * 2);
        let (mut analysis_send, analysis_recv) = rtrb::RingBuffer::<f32>::new(latency_samples * 2);

        for _ in 0..latency_samples {
            device_send.push(0.0)?;
            //analysis_send.push(0.0)?;
        }

        let (tx_play_song, rx_play_song) = channel::unbounded::<PathBuf>();

        // Spawn a thread to process audio files.
        std::thread::spawn(move || {
            while let Ok(song) = rx_play_song.recv() {
                let mut audio = AudioFile::open(song).unwrap();
                let mut audio_buf = Vec::<f32>::with_capacity(4 * chunk_size);
                let mut resampler_final = Vec::new();

                log::info!("audio channels: {}", audio.channels());
                log::info!("audio sample rate: {}", audio.sample_rate());

                let mut resampler = {
                    if audio.sample_rate() != device_sample_rate {
                        let interpolation_params = rubato::InterpolationParameters {
                            sinc_len: 256,
                            f_cutoff: 0.95,
                            interpolation: rubato::InterpolationType::Linear,
                            oversampling_factor: 256,
                            window: rubato::WindowFunction::BlackmanHarris2,
                        };
                        let inner = rubato::SincFixedIn::<f32>::new(
                            device_sample_rate as f64 / audio.sample_rate() as f64,
                            2.0,
                            interpolation_params,
                            chunk_size,
                            audio.channels(),
                        )
                        .unwrap();

                        let buf_in = inner.input_buffer_allocate();
                        let buf_out = inner.output_buffer_allocate();
                        log::info!(
                            "buf_in: {} buf_out: {}",
                            buf_in[0].capacity(),
                            buf_out[0].capacity()
                        );

                        Some(Resampler {
                            inner,
                            buf_in,
                            buf_out,
                        })
                    } else {
                        log::info!("NO REsampler");
                        None
                    }
                };

                let chunk_size = audio.channels() * chunk_size;

                loop {
                    match audio.next_sample(CopyMethod::Interleaved) {
                        Ok(Some(signal)) => {
                            let output = {
                                if let Some(ref mut resampler) = resampler {
                                    audio_buf.extend(signal.samples());
                                    if audio_buf.len() >= chunk_size {
                                        // Clear resampler buffers.
                                        for buf in [&mut resampler.buf_in, &mut resampler.buf_out] {
                                            for channel in buf {
                                                channel.clear();
                                            }
                                        }

                                        // Drain and process incoming audio.
                                        let mut chunk = audio_buf.drain(0..chunk_size);
                                        for _ in 0..chunk_size / audio.channels() {
                                            for channel in 0..audio.channels() {
                                                resampler.buf_in[channel]
                                                    .push(chunk.next().unwrap());
                                            }
                                        }

                                        resampler
                                            .inner
                                            .process_into_buffer(
                                                &resampler.buf_in,
                                                &mut resampler.buf_out,
                                                None,
                                            )
                                            .unwrap();
                                    } else {
                                        // Buffer not full - get more data.
                                        continue;
                                    }

                                    resampler_final.clear();

                                    for i in 0..resampler.buf_out[0].len() {
                                        for channel in 0..audio.channels() {
                                            resampler_final.push(resampler.buf_out[channel][i]);
                                        }
                                    }

                                    resampler_final.as_ref()
                                } else {
                                    signal.samples()
                                }
                            };

                            // Send output to ring buffers.
                            for sample in output {
                                loop {
                                    if device_send.push(*sample).is_ok() {
                                        if analysis_send.push(*sample).is_err() {
                                            //log::info!("couldn't write to lvl ringbuffer");
                                        }
                                        break;
                                    }
                                    log::info!("sleep: {}", latency_ms);
                                    thread::sleep(Duration::from_millis(latency_ms as u64 / 2));
                                }
                            }
                        }

                        Ok(None) => {
                            break;
                        }

                        Err(e) => {
                            log::error!("{e:?}");
                            break;
                        }
                    }
                }

                log::info!("Song over");
            }
        });

        // Create audio output stream.
        let stream = device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut input_fell_behind = false;

                for sample in data.chunks_mut(device_channels as usize) {
                    if let Ok(chunk) = device_recv.read_chunk(2) {
                        let mut chunk = chunk.into_iter();
                        sample[0] = chunk.next().unwrap();
                        sample[1] = chunk.next().unwrap();
                    } else {
                        input_fell_behind = true;
                        sample[0] = 0.0;
                        sample[1] = 0.0;
                    }
                }

                if input_fell_behind {
                    log::warn!("input fell behind");
                }
            },
            move |err| {
                log::error!("{err}");
            },
            None,
        )?;

        stream.play()?;

        let ebur128 = EbuR128::new(device_channels, device_sample_rate, Mode::M).unwrap();

        Ok(Self {
            stream,
            tx_play_song,
            lvl_cons: analysis_recv,
            rms: [0., 0.],
            rms_buf: None,
            sample_rate: device_sample_rate,
            channels: device_channels,
            ebur128,
        })
    }

    pub fn rms(&mut self, dt: Duration) -> ([f32; 2], f32) {
        let buf_size = (dt.as_secs_f32() * self.sample_rate as f32).round() as usize;

        let (mut l, mut r) = (vec![], vec![]);

        while let Ok(chunk) = self.lvl_cons.read_chunk(2) {
            let mut chunk = chunk.into_iter();
            l.push(chunk.next().unwrap().powi(2));
            r.push(chunk.next().unwrap().powi(2));
            if l.len() >= buf_size {
                break;
            }
        }

        log::trace!(
            "rms.len {} {} -- {}",
            l.len(),
            r.len(),
            self.lvl_cons.slots()
        );

        if !l.is_empty() && !r.is_empty() {
            self.ebur128.add_frames_planar_f32(&[&l, &r]).unwrap();

            let lvl_l = l.iter().sum::<f32>() / l.len() as f32;
            let lvl_r = r.iter().sum::<f32>() / r.len() as f32;

            self.rms = [lvl_l, lvl_r];
            //self.rms = [lvl_l.sqrt(), lvl_r.sqrt()];
        }

        let loudness = if let Ok(loudness) = self.ebur128.loudness_momentary() {
            loudness as f32
        } else {
            0.0
        };

        (self.rms, loudness)
    }

    pub fn play(&self, song: PathBuf) {
        self.tx_play_song.send(song).unwrap();
    }
}
