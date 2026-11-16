//! The window's Vulkan surface (C++'s openWindow's Vulkan part): `VK_KHR_wayland_surface` on libwayland's display
//! and the window's `wl_surface` (the driver's WSI attaches its buffers to it and commits them). An FFI module: its
//! one `unsafe` block says why it holds.

#![allow(unsafe_code)]

use super::ffi::{Connection, Object};
use super::generated::WlSurface;
use kompoz_gpu::device::vulkan::VulkanSurface;
use kompoz_gpu::device::vulkan::ash::{self, vk};

/// What the GPU device presents to the window with. The display and the surface must outlive the device (the app
/// core lets the device go before it closes the window).
pub(super) fn surface(connection: &Connection, surface: &WlSurface) -> VulkanSurface {
    let display = connection.as_ptr();
    let surface = surface.proxy().as_ptr();
    VulkanSurface {
        instance_extensions: vec![ash::khr::surface::NAME, ash::khr::wayland_surface::NAME],
        create_surface: Box::new(move |entry, instance| {
            let loader = ash::khr::wayland_surface::Instance::new(entry, instance);
            let info = vk::WaylandSurfaceCreateInfoKHR::default().display(display.cast()).surface(surface.cast());
            // SAFETY: the instance was made with VK_KHR_wayland_surface (this surface's instance_extensions); the
            // display and the surface are libwayland's live objects, kept until the device is gone.
            unsafe { loader.create_wayland_surface(&info, None) }.ok()
        }),
        native_window: None,
    }
}
