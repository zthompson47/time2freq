use anyhow::{Error, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam::channel;
use rubato::Resampler;

use crate::resources::{AudioFile, CopyMethod};

pub struct AudioPlayer {
    #[allow(unused)]
    stream: cpal::Stream,
    tx_play_song: channel::Sender<String>,
}

impl AudioPlayer {
    const CHUNK_SIZE: usize = 1024;

    pub fn new(latency_ms: u32, sample_rate: u32, channels: u32) -> Result<Self> {
        let latency_frames = (latency_ms as f32 * sample_rate as f32 / 1000.0).round() as u32;
        let latency_samples = (latency_frames * channels) as usize;
        let (mut ring_prod, mut ring_cons) = rtrb::RingBuffer::<f32>::new(latency_samples * 2);
        for _ in 0..latency_samples {
            ring_prod.push(0.0)?;
        }

        // Spawn thread to play songs.
        let (tx_play_song, rx_play_song) = channel::unbounded::<String>();
        std::thread::spawn(move || {
            let interpolation_params = rubato::InterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: rubato::InterpolationType::Linear,
                oversampling_factor: 256,
                window: rubato::WindowFunction::BlackmanHarris2,
            };

            let mut resampler = rubato::SincFixedIn::<f32>::new(
                48000.0 / 44100.0,
                2.0,
                interpolation_params,
                Self::CHUNK_SIZE,
                2,
            )
            .unwrap();

            let mut buf_resampler_in = resampler.input_buffer_allocate();
            let mut buf_resampler_out = resampler.output_buffer_allocate();

            while let Ok(song) = rx_play_song.recv() {
                let mut audio = AudioFile::open(song).unwrap();
                let mut audio_buf = Vec::<f32>::with_capacity(4 * Self::CHUNK_SIZE);
                let mut resampler_final = Vec::new();

                while let Ok(Some(buf)) = audio.next_sample(CopyMethod::Interleaved) {
                    let output = {
                        if audio.sample_rate() == 44100 {
                            audio_buf.extend(buf.samples());

                            if audio_buf.len() >= 2 * Self::CHUNK_SIZE {
                                buf_resampler_in[0].clear();
                                buf_resampler_in[1].clear();
                                buf_resampler_out[0].clear();
                                buf_resampler_out[1].clear();

                                let len = 2 * Self::CHUNK_SIZE;
                                let mut chunk = audio_buf.drain(0..len);
                                for _ in 0..len / 2 {
                                    buf_resampler_in[0].push(chunk.next().unwrap());
                                    buf_resampler_in[1].push(chunk.next().unwrap());
                                }

                                resampler
                                    .process_into_buffer(
                                        &buf_resampler_in,
                                        &mut buf_resampler_out,
                                        None,
                                    )
                                    .unwrap();
                            } else {
                                continue;
                            }

                            resampler_final.clear();

                            for i in 0..buf_resampler_out[0].len() {
                                resampler_final.push(buf_resampler_out[0][i]);
                                resampler_final.push(buf_resampler_out[1][i]);
                            }

                            resampler_final.as_ref()
                        } else {
                            buf.samples()
                        }
                    };

                    for sample in output {
                        loop {
                            if ring_prod.push(*sample).is_ok() {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(latency_ms as u64));
                        }
                    }
                }
            }
        });

        // Open audio output device.
        let host = cpal::default_host();
        let cpal_device = host
            .default_output_device()
            .ok_or_else(|| Error::msg("No audio output device."))?;
        let mut cpal_config = None;
        for c in cpal_device.supported_output_configs()? {
            if c.channels() == 2 {
                cpal_config = Some(c.with_sample_rate(cpal::SampleRate(sample_rate)));
            }
        }

        if cpal_config.is_none() || cpal_config.as_ref().unwrap().channels() != 2 {
            return Err(Error::msg(
                "Could not get config for 2 channels with sr {sample_rate}",
            ));
        }

        // Create cpal stream to play audio.
        let stream = cpal_device.build_output_stream(
            &cpal_config.unwrap().into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut input_fell_behind = false;

                for sample in data.chunks_mut(channels as usize) {
                    if let Ok(chunk) = ring_cons.read_chunk(2) {
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
                    eprintln!("input fell behind");
                }
            },
            move |err| {
                eprintln!("{err:?}");
            },
        )?;

        stream.play()?;

        Ok(Self {
            stream,
            tx_play_song,
        })
    }

    pub fn play(&self, song: &str) {
        self.tx_play_song.send(song.into()).unwrap();
    }
}
