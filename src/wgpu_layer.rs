use std::time::Instant;

use crate::paper::Paper;
use smithay_client_toolkit::{
    compositor::CompositorHandler,
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_xdg_shell,
    output::OutputState,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
        WaylandSurface,
    },
};
use wayland_client::{
    protocol::{wl_pointer, wl_seat, wl_surface},
    Connection, QueueHandle,
};

pub struct WgpuLayer {
    pub start_time: Instant,

    pub layer: LayerSurface,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub render_pipeline: wgpu::RenderPipeline,

    pub elapsed_time_bind_group: wgpu::BindGroup,
    pub elapsed_time_buffer: wgpu::Buffer,
}

// Boilerplate Papaer implements

delegate_compositor!(Paper);
delegate_output!(Paper);

delegate_seat!(Paper);
delegate_layer!(Paper);
delegate_pointer!(Paper);
delegate_xdg_shell!(Paper);

delegate_registry!(Paper);

impl CompositorHandler for Paper {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Not needed for this example.
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.draw(qh);
    }
}

impl SeatHandler for Paper {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            println!("Set pointer capability");
            let pointer = self
                .seat_state
                .get_pointer(qh, &seat)
                .expect("Failed to create pointer");
            self.pointer = Some(pointer);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_some() {
            println!("Set pointer capability");
            self.pointer = None;
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for Paper {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        use PointerEventKind::*;
        for event in events {
            // Ignore events for other surfaces
            if self.wgpu_layer.is_none()
                || (&event.surface != self.wgpu_layer.as_ref().unwrap().layer.wl_surface())
            {
                continue;
            }
            match event.kind {
                Enter { .. } => {
                    println!("Pointer entered @{:?}", event.position);
                }
                Leave { .. } => {
                    println!("Pointer left");
                }
                Motion { .. } => {}
                Press { button, .. } => {
                    println!("Press {:x} @ {:?}", button, event.position);
                    //self.shift = self.shift.xor(Some(0));
                }
                Release { button, .. } => {
                    println!("Release {:x} @ {:?}", button, event.position);
                }
                Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    println!("Scroll H:{horizontal:?}, V:{vertical:?}");
                }
            }
        }
    }
}

impl ProvidesRegistryState for Paper {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

impl LayerShellHandler for Paper {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if configure.new_size.0 == 0 || configure.new_size.1 == 0 {
            self.width = 256;
            self.height = 256;
        } else {
            self.width = configure.new_size.0;
            self.height = configure.new_size.1;
        }
        if let Some(wgpu_layer) = &self.wgpu_layer {
            let cap = wgpu_layer.surface.get_capabilities(&wgpu_layer.adapter);
            let surface_config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: cap.formats[0],
                view_formats: vec![cap.formats[0]],
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                width: self.width,
                height: self.height,
                // Wayland is inherently a mailbox system.
                // But we are using Fifo (traditional vsync), since all the gpus support that
                present_mode: wgpu::PresentMode::Fifo,
            };

            wgpu_layer
                .surface
                .configure(&wgpu_layer.device, &surface_config);

            self.draw(qh);
        }
    }
}
