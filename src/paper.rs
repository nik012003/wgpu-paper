//use rand::seq::SliceRandom;
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle,
};
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::{OutputHandler, OutputState},
    registry::RegistryState,
    seat::SeatState,
    shell::{
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
        WaylandSurface,
    },
};
use wayland_client::{
    globals::{registry_queue_init, GlobalList},
    protocol::{
        wl_output::{self},
        wl_pointer,
    },
    Connection, Proxy, QueueHandle,
};

use std::{
    fs,
    path::PathBuf,
    thread::sleep,
    time::{Duration, Instant},
};

pub struct PaperConfig {
    pub output_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub anchor: Anchor,
    pub margin: Margin,
    pub pointer_trail_frames: usize,
    pub fps: Option<u64>,
    pub shader_path: PathBuf,
}

use crate::wgpu_layer::*;
pub struct Paper {
    pub registry_state: RegistryState,
    pub seat_state: SeatState,
    pub output_state: OutputState,

    pub globals: GlobalList,
    pub qh: QueueHandle<Self>,

    pub exit: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub anchor: Anchor,
    pub margin: Margin,

    pub shader_path: PathBuf,
    pub output_name: Option<String>,
    pub fps: Option<u64>,
    pub last_frame: Instant,

    pub pointer: Option<wl_pointer::WlPointer>,
    pub pointer_positions: Vec<[f32; 4]>,
    pub current_pointer_pos: Option<[f32; 2]>,
    pub wgpu_layer: Option<WgpuLayer>,
}

pub struct Margin {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Paper {
    pub fn run(config: PaperConfig) {
        let conn = Connection::connect_to_env().unwrap();

        let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
        let qh: QueueHandle<Self> = event_queue.handle();

        let mut paper = Self {
            registry_state: RegistryState::new(&globals),
            seat_state: SeatState::new(&globals, &qh),
            output_state: OutputState::new(&globals, &qh),
            globals,
            qh,
            exit: false,
            width: config.width,
            height: config.height,
            anchor: config.anchor,
            margin: config.margin,
            shader_path: config.shader_path,
            output_name: config.output_name,
            fps: config.fps,
            last_frame: Instant::now(),
            pointer: None,
            /*
            Right now, we are using (-100, -100) to indicate that the pointer isn't getting captured
            This approach *sucks*, but I don't know how to make it better
            I really don't wanna send another buffer to the gpu just for that
            The last two elements are there to pad out the elements so that they fit in a vec4
            */
            pointer_positions: vec![[-100.0, -100.0, 0.0, 0.0]; config.pointer_trail_frames],
            current_pointer_pos: None,
            wgpu_layer: None,
        };

        loop {
            event_queue.blocking_dispatch(&mut paper).unwrap();

            if paper.exit {
                println!("exiting example");
                break;
            }
        }

        // On exit we must destroy the surface before the layer is destroyed.
        if let Some(wgpu_layer) = paper.wgpu_layer {
            drop(wgpu_layer.surface);
            drop(wgpu_layer.layer);
        }
    }
}

impl OutputHandler for Paper {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        // Check if wgpu_layer was already created
        if self.wgpu_layer.is_some() {
            return;
        }

        if self.output_name.is_some() {
            let info = self.output_state.info(&output);
            if info.is_none() || info.unwrap().name != self.output_name {
                return;
            }
        }

        // Load the shader
        let shader_data =
            fs::read_to_string(self.shader_path.clone()).expect("Unable to read file");

        let compositor =
            CompositorState::bind(&self.globals, &self.qh).expect("wl_compositor is not available");
        // This app uses the wlr layer shell, which may not be available with every compositor.
        let layer_shell =
            LayerShell::bind(&self.globals, qh).expect("layer shell is not available");

        let surface = compositor.create_surface(qh);
        // Create the window for adapter selection
        //let window = xdg_shell_state.create_window(surface, WindowDecorations::ServerDefault, &qh);
        let layer = layer_shell.create_layer_surface(
            qh,
            surface.clone(),
            Layer::Background,
            Some("wpgu_layer"),
            Some(&output),
        );
        // Configure the layer surface, providing things like the anchor on screen, desired size and the keyboard
        // interactivity
        layer.set_anchor(self.anchor);
        layer.set_margin(
            self.margin.top,
            self.margin.right,
            self.margin.bottom,
            self.margin.left,
        );
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);

        // Get size of output
        if self.width.is_none() || self.height.is_none() {
            let (width, height) = self
                .output_state
                .info(&output)
                .expect("Unable to retreive output information. -W and -H required")
                .logical_size
                .expect("Unable to retreive output logical_size . -W and -H required");
            if self.width.is_none() {
                self.width = Some(width.try_into().unwrap());
            }
            if self.height.is_none() {
                self.height = Some(height.try_into().unwrap());
            }
        }
        layer.set_size(self.width.unwrap(), self.height.unwrap());

        // In order for the layer surface to be mapped, we need to perform an initial commit with no attached\
        // buffer. For more info, see WaylandSurface::commit
        //
        // The compositor will respond with an initial configure that we can then use to present to the layer
        // surface with the correct options.
        layer.commit();

        // Initialize wgpu
        let instance = wgpu::Instance::default();

        // Create the raw window handle for the surface.
        let handle = {
            let mut handle = WaylandDisplayHandle::empty();
            handle.display = conn.backend().display_ptr() as *mut _;
            let display_handle = RawDisplayHandle::Wayland(handle);

            let mut handle = WaylandWindowHandle::empty();
            handle.surface = surface.id().as_ptr() as *mut _;
            // TODO : implement support for regualar wayland windows, instead of just layer shell
            // handle.surface = window.wl_surface().id().as_ptr() as *mut _;
            let window_handle = RawWindowHandle::Wayland(handle);

            struct RawWindowHandleHasRawWindowHandle(RawDisplayHandle, RawWindowHandle);

            unsafe impl HasRawDisplayHandle for RawWindowHandleHasRawWindowHandle {
                fn raw_display_handle(&self) -> RawDisplayHandle {
                    self.0
                }
            }

            unsafe impl HasRawWindowHandle for RawWindowHandleHasRawWindowHandle {
                fn raw_window_handle(&self) -> RawWindowHandle {
                    self.1
                }
            }

            RawWindowHandleHasRawWindowHandle(display_handle, window_handle)
        };

        let surface = unsafe { instance.create_surface(&handle).unwrap() };

        // Pick a supported adapter
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("Failed to find suitable adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                limits:
                    wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            },
            None,
        ))
        .expect("Failed to request device");

        //dbg!(&adapter.get_info());

        /* -- Elpsed time buffer, binding: 0 -- */
        let (elapsed_time_buffer, elapsed_time_group_layout, elapsed_time_bind_group) =
            create_gpu_buffer(
                &device,
                "elapsed_time",
                0,
                bytemuck::cast_slice(&[0.0f32]),
                false,
            );
        /* -- Pointer pos buffer, binding: 0 -- */
        let (pointer_buffer, pointer_group_layout, pointer_bind_group) = create_gpu_buffer(
            &device,
            "pointer",
            1,
            bytemuck::cast_slice(self.pointer_positions.as_slice()),
            false,
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_data.into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&elapsed_time_group_layout, &pointer_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                // Add the surface target
                targets: &[Some(wgpu::ColorTargetState {
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                    format: surface.get_capabilities(&adapter).formats[0],
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Default::default(),
            multisample: Default::default(),
            multiview: Default::default(),
        });

        self.wgpu_layer = Some(WgpuLayer {
            start_time: Instant::now(),
            layer,
            adapter,
            device,
            queue,
            surface,
            render_pipeline,
            elapsed_time_bind_group,
            elapsed_time_buffer,
            pointer_bind_group,
            pointer_buffer,
        })
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl Paper {
    pub fn draw(&mut self, qh: &QueueHandle<Self>) {
        if self.wgpu_layer.is_none() {
            return;
        };

        if let Some(fps) = self.fps {
            let time = Duration::from_secs_f64(1.0 / fps as f64)
                .checked_sub(Instant::now() - self.last_frame);
            if let Some(wait) = time {
                sleep(wait);
            }
        }
        let wgpu_layer = self.wgpu_layer.as_ref().unwrap();
        let surface_texture = wgpu_layer
            .surface
            .get_current_texture()
            .expect("failed to acquire next swapchain texture");

        let texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = wgpu_layer
            .device
            .create_command_encoder(&Default::default());
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[
                    // This is what @location(0) in the fragment shader targets
                    Some(wgpu::RenderPassColorAttachment {
                        view: &texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::default()),
                            store: true,
                        },
                    }),
                ],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&wgpu_layer.render_pipeline);

            render_pass.set_bind_group(0, &wgpu_layer.elapsed_time_bind_group, &[]);
            render_pass.set_bind_group(1, &wgpu_layer.pointer_bind_group, &[]);

            render_pass.draw(0..3, 0..1);
        }

        // Submit the command in the queue to execute
        wgpu_layer.queue.write_buffer(
            &wgpu_layer.elapsed_time_buffer,
            0,
            bytemuck::cast_slice(&[wgpu_layer.start_time.elapsed().as_secs_f32()]),
        );
        self.pointer_positions.pop();
        // Again, really bad
        let pos = self.current_pointer_pos.unwrap_or([-100.0f32, -100.0f32]);
        self.pointer_positions.insert(0, [pos[0], pos[1], 0.0, 0.0]);

        wgpu_layer.queue.write_buffer(
            &wgpu_layer.pointer_buffer,
            0,
            bytemuck::cast_slice(self.pointer_positions.as_slice()),
        );

        wgpu_layer.queue.submit(Some(encoder.finish()));
        surface_texture.present();
        self.last_frame = Instant::now();
        // Request new frame
        wgpu_layer
            .layer
            .wl_surface()
            .frame(qh, wgpu_layer.layer.wl_surface().clone());
    }
}
