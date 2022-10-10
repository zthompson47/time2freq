use std::{collections::HashMap, fs::File, path::Path};

use anyhow::{Error, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam::channel;
use symphonia::{
    core::{
        audio::SampleBuffer,
        codecs::{Decoder, DecoderOptions},
        errors::Error::DecodeError,
        formats::{FormatOptions, FormatReader},
        io::MediaSourceStream,
        meta::MetadataOptions,
        probe::Hint,
    },
    default::{get_codecs, get_probe},
};

pub struct AudioPlayer {
    #[allow(unused)]
    stream: cpal::Stream,
    tx_play_song: channel::Sender<String>,
}

impl AudioPlayer {
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
            while let Ok(song) = rx_play_song.recv() {
                let mut audio = AudioFile::open(song).unwrap();

                while let Ok(Some(buf)) = audio.next_sample(CopyMethod::Interleaved) {
                    for i in 0..buf.len() {
                        loop {
                            if ring_prod.push(buf.samples()[i]).is_ok() {
                                // Fill buffer for signal analysis.
                                //if rb_prod_lvl.push(buf.samples()[i]).is_err() {
                                //panic!()
                                //}
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
                    //eprintln!("input fell behind");
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

pub struct AudioFile {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    default_track_id: u32,
}

impl AudioFile {
    pub fn info(&self) -> HashMap<String, String> {
        [
            ("sample_rate".to_string(), self.sample_rate().to_string()),
            ("channels".to_string(), self.channels().to_string()),
        ]
        .into()
    }

    pub fn sample_rate(&self) -> u32 {
        self.decoder.codec_params().sample_rate.unwrap()
    }

    pub fn channels(&self) -> usize {
        self.decoder.codec_params().channels.unwrap().count()
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let src = Box::new(File::open(path)?);
        let mss = MediaSourceStream::new(src, Default::default());
        let hint = Hint::new();
        let format_opts: FormatOptions = Default::default();
        let metadata_opts: MetadataOptions = Default::default();
        let decoder_opts: DecoderOptions = Default::default();
        let format = get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .unwrap()
            .format;
        let track = format
            .default_track()
            .ok_or_else(|| Error::msg("No default track."))?;
        let decoder = get_codecs().make(&track.codec_params, &decoder_opts)?;
        let default_track_id = track.id;

        Ok(AudioFile {
            format,
            decoder,
            default_track_id,
        })
    }

    pub fn next_sample(&mut self, meth: CopyMethod) -> Result<Option<SampleBuffer<f32>>> {
        let packet = self.format.next_packet()?;
        if packet.track_id() != self.default_track_id {
            return Ok(None);
        }
        match self.decoder.decode(&packet) {
            Ok(audio_buf_ref) => {
                let spec = *audio_buf_ref.spec();
                let duration = audio_buf_ref.capacity() as u64;
                let mut buf = SampleBuffer::new(duration, spec);
                if let CopyMethod::Interleaved = meth {
                    buf.copy_interleaved_ref(audio_buf_ref);
                } else if let CopyMethod::Planar = meth {
                    buf.copy_planar_ref(audio_buf_ref);
                }
                Ok(Some(buf))
            }
            Err(DecodeError(_)) => Ok(None),
            Err(_) => Err(Error::msg("Decode error.")),
        }
    }

    pub fn dump(&mut self) -> (Vec<f32>, Vec<f32>) {
        let mut left = Vec::new();
        let mut right = Vec::new();
        while let Ok(buf) = self.next_sample(CopyMethod::Planar) {
            if let Some(buf) = buf {
                let s = buf.samples();
                left.append(&mut Vec::from(&s[..s.len() / 2]));
                right.append(&mut Vec::from(&s[s.len() / 2..]));
            }
        }
        (left, right)
    }
}

pub enum CopyMethod {
    Interleaved,
    Planar,
}