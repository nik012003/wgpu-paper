use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::StreamConfig;
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use std::sync::{Arc, Mutex};

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
        let host = cpal::default_host();
        let device = match audio_in.lock().unwrap().device_name.clone() {
            Some(name) => host
                .input_devices()
                .expect("Coudn't get input devices")
                .filter(|d| d.name().is_ok())
                .find(|d| d.name().unwrap() == name),
            None => host.default_input_device(),
        }
        .expect("No input device found");

        let n_channels: usize = audio_in.lock().unwrap().channels;

        let config = StreamConfig {
            channels: n_channels as u16,
            sample_rate: cpal::SampleRate(audio_in.lock().unwrap().sample_rate),
            buffer_size: cpal::BufferSize::Fixed(audio_in.lock().unwrap().buffer_size),
        };
        let err_fn = move |err| {
            eprintln!("an error occurred on stream: {}", err);
        };
        let mut planner = FftPlanner::<f32>::new();
        let stream = device
            .build_input_stream(
                &config,
                move |data: &[f32], _| {
                    let mut ai = audio_in.lock().unwrap();

                    // If the buffers haven't been used yet, don't waste cpu time
                    // This is suboptimal, since it adds latency, but it should be acceptable given high enough fps
                    if !ai.used {
                        return;
                    }
                    ai.used = false;
                    // [ [lr] [lr] [lr] ... ]
                    let channels_buff: Vec<&[f32]> = data.chunks(n_channels).collect();

                    let fft = planner.plan_fft_forward(channels_buff.len());
                    ai.audio_buffers = vec![vec![]; n_channels];
                    for sample in channels_buff.iter() {
                        for (channel, amplitude) in sample.iter().enumerate() {
                            ai.audio_buffers[channel].push(*amplitude);
                        }
                    }

                    let mut fft_cpmlx_buffers = vec![vec![]; n_channels];
                    for (channel, wave) in ai.audio_buffers.iter().enumerate() {
                        fft_cpmlx_buffers[channel] = wave
                            .iter()
                            .map(|n| Complex { re: *n, im: 0.0f32 })
                            .collect();
                        fft.process(&mut fft_cpmlx_buffers[channel]);
                    }

                    ai.fft_buffers = vec![vec![]; n_channels];
                    for (channel, cmplx_fft) in fft_cpmlx_buffers.iter().enumerate() {
                        ai.fft_buffers[channel] = cmplx_fft
                            .iter()
                            .map(|c| {
                                // Real part > 0
                                // Im part < 0
                                let re = if c.re > 0.0f32 { c.re } else { 0.0f32 };
                                let im = if c.im < 0.0f32 { -c.re } else { 0.0f32 };
                                re + im
                            })
                            .collect();
                    }
                },
                err_fn,
                None,
            )
            .expect("cound't create stream");
        stream.play().expect("nope");
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}
