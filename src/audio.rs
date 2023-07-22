use psimple::Simple;
use pulse::def::BufferAttr;
use pulse::sample::{Format, Spec};
use pulse::stream::Direction;
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct AudioInput {
    pub device_name: Option<String>,
    pub channels: usize,
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub used: bool,
    pub audio_buffers: Vec<Vec<f32>>,
    pub fft_buffers: Vec<Vec<f32>>,
}

impl AudioInput {
    pub fn new(
        device_name: Option<String>,
        channels: usize,
        sample_rate: u32,
        buffer_size: u32,
    ) -> Self {
        AudioInput {
            device_name,
            channels,
            sample_rate,
            buffer_size,
            used: true,
            audio_buffers: vec![],
            fft_buffers: vec![],
        }
    }

    pub fn start_capture_loop(audio_in: Arc<Mutex<AudioInput>>) {
        let ai = audio_in.lock().unwrap();
        let n_channels = ai.channels;
        let buffer_size = ai.buffer_size;
        let sample_rate = ai.sample_rate;
        let spec = Spec {
            format: Format::FLOAT32NE,
            channels: n_channels as u8,
            rate: sample_rate,
        };

        let attr = BufferAttr {
            maxlength: std::u32::MAX,
            fragsize: buffer_size,
            ..Default::default()
        };

        assert!(spec.is_valid());
        let s = Simple::new(
            None,                      // Use the default server
            "wgpu-paper",              // Our application’s name
            Direction::Record,         // We want a playback stream
            ai.device_name.as_deref(), // Use the default device
            "Music",                   // Description of our stream
            &spec,                     // Our sample format
            None,                      // Use default channel map
            Some(&attr),               // Use default buffering attributes
        )
        .expect("Error opening audio stream");

        drop(ai);

        let mut planner = FftPlanner::<f32>::new();
        loop {
            // If the buffers haven't been used yet, don't waste cpu time
            // This is suboptimal, since it adds latency, but it should be acceptable given high enough fps

            while !audio_in.lock().unwrap().used {
                std::thread::sleep(Duration::from_millis(
                    sample_rate as u64 / buffer_size as u64,
                ));
            }

            // Read data from pulseaudio
            let mut d_binding =
                vec![0; buffer_size as usize * std::mem::size_of::<f32>() * n_channels];
            let d = d_binding.as_mut_slice();

            s.read(d).unwrap();

            // Convert the byte array into f32
            let data: Vec<f32> = d
                .chunks(4)
                .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            // The data is stored this way:
            // sample = [ channel-0 , channel-1, ... channel-n]
            let samples: Vec<&[f32]> = data.chunks(n_channels).collect();
            dbg!(samples.len());
            let fft = planner.plan_fft_forward(samples.len());
            let mut audio_buffers = vec![vec![]; n_channels];
            for sample in samples.iter() {
                for (channel, amplitude) in sample.iter().enumerate() {
                    audio_buffers[channel].push(*amplitude);
                }
            }

            // f32 -> cmplx numbers
            let mut fft_cpmlx_buffers = vec![vec![]; n_channels];
            for (channel, wave) in audio_buffers.iter().enumerate() {
                fft_cpmlx_buffers[channel] = wave
                    .iter()
                    .map(|n| Complex { re: *n, im: 0.0f32 })
                    .collect();
                fft.process(&mut fft_cpmlx_buffers[channel]);
            }

            // Calculate fft
            let mut fft_buffers = vec![vec![]; n_channels];
            for (channel, cmplx_fft) in fft_cpmlx_buffers.iter().enumerate() {
                fft_buffers[channel] = cmplx_fft
                    .iter()
                    .take(samples.len() / 2)
                    .map(|c| c.norm())
                    .collect();
            }

            // Swap buffers

            let mut ai = audio_in.lock().unwrap();
            ai.audio_buffers = audio_buffers;
            ai.fft_buffers = fft_buffers;
            ai.used = false;
            //dbg!(ai.fft_buffers[0]
            //    .iter()
            //    .enumerate()
            //    .max_by(|a, b| a.1.total_cmp(b.1)));
        }
    }
}
