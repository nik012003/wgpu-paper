use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::StreamConfig;
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use std::sync::{Arc, Mutex};

const CHANNELS: u16 = 2;
pub struct AudioInput {
    pub modified: bool,
    pub audio_buffers: Vec<Vec<f32>>,
    pub fft_buffers: Vec<Vec<f32>>,
}

impl AudioInput {
    pub fn new() -> Self {
        AudioInput {
            modified: true,
            audio_buffers: vec![],
            fft_buffers: vec![],
        }
    }

    pub fn start_capture_loop(audio_in: Arc<Mutex<AudioInput>>) {
        // TODO: make everthing configurable by the user
        let host = cpal::default_host();
        let device = host.default_input_device().expect("No input device found");
        let config = StreamConfig {
            channels: CHANNELS,
            sample_rate: cpal::SampleRate(44100),
            buffer_size: cpal::BufferSize::Fixed(8096),
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
                    // [ [lr] [lr] [lr] ... ]
                    let channels_buff: Vec<&[f32]> = data.chunks(CHANNELS as usize).collect();

                    let fft = planner.plan_fft_forward(channels_buff.len());
                    ai.audio_buffers = vec![vec![]; CHANNELS as usize];
                    for sample in channels_buff.iter() {
                        for (channel, amplitude) in sample.iter().enumerate() {
                            ai.audio_buffers[channel].push(*amplitude);
                        }
                    }

                    let mut fft_cpmlx_buffers = vec![vec![]; CHANNELS as usize];
                    for (channel, wave) in ai.audio_buffers.iter().enumerate() {
                        fft_cpmlx_buffers[channel] = wave
                            .iter()
                            .map(|n| Complex { re: *n, im: 0.0f32 })
                            .collect();
                        fft.process(&mut fft_cpmlx_buffers[channel]);
                    }

                    ai.fft_buffers = vec![vec![]; CHANNELS as usize];
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
                    //dbg!(&ai.fft_buffers);
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
