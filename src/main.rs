//use rand::seq::SliceRandom;
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle,
};
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    registry::RegistryState,
    seat::SeatState,
    shell::{
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
        WaylandSurface,
    },
};
use wayland_client::{globals::registry_queue_init, Connection, Proxy, QueueHandle};

use crate::wgpu_layer::*;
mod wgpu_layer;
fn main() {
    // All Wayland apps start by connecting the compositor (server).
    let conn = Connection::connect_to_env().unwrap();

    // Enumerate the list of globals to get the protocols the server implements.
    let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    // The compositor (not to be confused with the server which is commonly called the compositor) allows
    // configuring surfaces to be presented.
    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor is not available");
    // This app uses the wlr layer shell, which may not be available with every compositor.
    let layer_shell = LayerShell::bind(&globals, &qh).expect("layer shell is not available");

    let surface = compositor.create_surface(&qh);
    // Create the window for adapter selection
    //let window = xdg_shell_state.create_window(surface, WindowDecorations::ServerDefault, &qh);
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface.clone(),
        Layer::Background,
        Some("simple_layer"),
        None,
    );
    // Configure the layer surface, providing things like the anchor on screen, desired size and the keyboard
    // interactivity
    layer.set_anchor(Anchor::RIGHT);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
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
        //handle.surface = window.wl_surface().id().as_ptr() as *mut _;
        let window_handle = RawWindowHandle::Wayland(handle);

        /// https://github.com/rust-windowing/raw-window-handle/issues/49
        struct YesRawWindowHandleImplementingHasRawWindowHandleIsUnsound(
            RawDisplayHandle,
            RawWindowHandle,
        );

        unsafe impl HasRawDisplayHandle for YesRawWindowHandleImplementingHasRawWindowHandleIsUnsound {
            fn raw_display_handle(&self) -> RawDisplayHandle {
                self.0
            }
        }

        unsafe impl HasRawWindowHandle for YesRawWindowHandleImplementingHasRawWindowHandleIsUnsound {
            fn raw_window_handle(&self) -> RawWindowHandle {
                self.1
            }
        }

        YesRawWindowHandleImplementingHasRawWindowHandleIsUnsound(display_handle, window_handle)
    };

    let surface = unsafe { instance.create_surface(&handle).unwrap() };

    // Pick a supported adapter
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        //power_preference: wgpu::PowerPreference::default(),
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
        ..Default::default()
    }))
    .expect("Failed to find suitable adapter");

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::empty(),
            // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
            limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
        },
        None,
    ))
    .expect("Failed to request device");

    dbg!(&adapter.get_info());

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
    });
    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main", // 1.
            buffers: &[],           // 2.
        },
        fragment: Some(wgpu::FragmentState {
            // 3.
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                // 4.
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

    let mut wgpu = WgpuLayer {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),

        exit: false,
        width: 256,
        height: 256,
        layer,
        device,
        surface,
        adapter,
        queue,
        render_pipeline,
    };

    // We don't draw immediately, the configure will notify us when to first draw.
    loop {
        event_queue.blocking_dispatch(&mut wgpu).unwrap();

        if wgpu.exit {
            println!("exiting example");
            break;
        }
    }

    // On exit we must destroy the surface before the window is destroyed.
    drop(wgpu.surface);
    //drop(wgpu.window);
}

impl WgpuLayer {
    fn draw(&self, qh: &QueueHandle<Self>) {
        let surface_texture = self
            .surface
            .get_current_texture()
            .expect("failed to acquire next swapchain texture");

        let texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());
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

            // NEW!
            render_pass.set_pipeline(&self.render_pipeline); // 2.
            render_pass.draw(0..3, 0..1); // 3.

            //let _renderpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            //    label: None,
            //    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            //        view: &texture_view,
            //        resolve_target: None,
            //        ops: wgpu::Operations {
            //            load: wgpu::LoadOp::Clear(
            //                vec![wgpu::Color::RED, wgpu::Color::BLUE]
            //                    .choose(&mut rand::thread_rng())
            //                    .unwrap()
            //                    .to_owned(),
            //            ),
            //            store: true,
            //        },
            //    })],
            //    depth_stencil_attachment: None,
            //});
        }

        // Submit the command in the queue to execute
        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();

        self.layer
            .wl_surface()
            .frame(qh, self.layer.wl_surface().clone());
    }
}
