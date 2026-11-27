//! A window's Vulkan surface (C++'s openWindow's Vulkan part): `VK_KHR_win32_surface` on the program's module and
//! the window's handle (the driver's WSI presents to it: through DXGI or GDI, as the driver does). An FFI module: its
//! one `unsafe` block says why it holds.

#![allow(unsafe_code)]

use super::sys::Hwnd;
use kompoz_gpu::device::vulkan::VulkanSurface;
use kompoz_gpu::device::vulkan::ash::{self, vk};

/// What the GPU device presents to `hwnd` with. The window must outlive the device (the app core lets the device go
/// before it closes the window).
pub(super) fn surface(hwnd: Hwnd) -> VulkanSurface {
    let (instance, window) = (hwnd.module_handle(), hwnd.value());
    VulkanSurface {
        instance_extensions: vec![ash::khr::surface::NAME, ash::khr::win32_surface::NAME],
        create_surface: Box::new(move |entry, vk_instance| {
            let loader = ash::khr::win32_surface::Instance::new(entry, vk_instance);
            let info = vk::Win32SurfaceCreateInfoKHR::default().hinstance(instance).hwnd(window);
            // SAFETY: the instance was made with VK_KHR_win32_surface (this surface's instance_extensions); the
            // module is the program's, the window alive until the device is gone.
            unsafe { loader.create_win32_surface(&info, None) }.ok()
        }),
        native_window: None,
        size_is_drawable: false,
    }
}
