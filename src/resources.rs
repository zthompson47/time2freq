#![allow(unused)]
use std::{collections::HashMap, fs::File, path::Path};

use anyhow::{Error, Result};
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
