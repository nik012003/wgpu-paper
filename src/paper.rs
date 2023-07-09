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
    protocol::{wl_output, wl_pointer},
    Connection, Proxy, QueueHandle,
};
use wgpu::util::DeviceExt;

use std::{fs, path::PathBuf, time::Instant};

use crate::wgpu_layer::*;
pub struct Paper {
    pub registry_state: RegistryState,
    pub seat_state: SeatState,
    pub output_state: OutputState,

    pub globals: GlobalList,
    pub qh: QueueHandle<Self>,

    pub exit: bool,
    pub width: u32,
    pub height: u32,

    pub shader_path: PathBuf,
    pub output_name: Option<String>,

    pub pointer: Option<wl_pointer::WlPointer>,
    pub wgpu_layer: Option<WgpuLayer>,
}

impl Paper {
    pub fn run(shader_path: PathBuf, output_name: Option<String>) {
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
            width: 256,
            height: 256,
            shader_path,
            output_name,
            pointer: None,
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
        _output: wl_output::WlOutput,
    ) {
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
            None,
        );
        // Configure the layer surface, providing things like the anchor on screen, desired size and the keyboard
        // interactivity
        layer.set_anchor(Anchor::RIGHT);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        // TODO: get size of outputs
        layer.set_size(1920, 1080);

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
            //handle.surface = window.wl_surface().id().as_ptr() as *mut _;
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

        dbg!(&adapter.get_info());

        let elapsed_time_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[0.0f32]), // Start from 0
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create a bind group layout visible to the fragment shader
        let elapsed_time_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("elapsed_time_group_layout"),
            });

        // Create the bind group
        let elapsed_time_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &elapsed_time_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: elapsed_time_buffer.as_entire_binding(),
            }],
            label: Some("elapsed_time_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_data.into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&elapsed_time_group_layout],
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
    pub fn draw(&self, qh: &QueueHandle<Self>) {
        if self.wgpu_layer.is_none() {
            return;
        };
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
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.1,
                                g: 0.2,
                                b: 0.3,
                                a: 1.0,
                            }),
                            store: true,
                        },
                    }),
                ],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&wgpu_layer.render_pipeline);

            render_pass.set_bind_group(0, &wgpu_layer.elapsed_time_bind_group, &[]);

            render_pass.draw(0..3, 0..1); // 3.
        }

        // Submit the command in the queue to execute
        wgpu_layer.queue.write_buffer(
            &wgpu_layer.elapsed_time_buffer,
            0,
            bytemuck::cast_slice(&[wgpu_layer.start_time.elapsed().as_secs_f32()]),
        );

        wgpu_layer.queue.submit(Some(encoder.finish()));
        surface_texture.present();

        // Request new frame
        wgpu_layer
            .layer
            .wl_surface()
            .frame(qh, wgpu_layer.layer.wl_surface().clone());
    }
}
