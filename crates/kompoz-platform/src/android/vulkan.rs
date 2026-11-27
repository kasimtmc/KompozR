//! The view's Vulkan surface (C++'s openWindow's Vulkan part): `VK_KHR_android_surface` on the `ANativeWindow` the
//! view's surface has now. Android gives the view another window each time the app comes back from the background
//! (none while it is away): the device asks [`VulkanSurface::native_window`] which one it is before each frame and
//! makes its surface again for a new one. An FFI module: its one `unsafe` block says why it holds.

#![allow(unsafe_code)]

use kompoz_gpu::device::vulkan::VulkanSurface;
use kompoz_gpu::device::vulkan::ash::{self, vk};
use std::cell::Cell;
use std::rc::Rc;

/// What the GPU device presents with: a surface of the native window `window` holds (its address; 0 while the view
/// has none). The platform keeps each window alive until the device has let go of its surface
/// (`AppCore::surface_changed` before the window is released).
pub(super) fn surface(window: Rc<Cell<usize>>) -> VulkanSurface {
    let made_for = window.clone();
    VulkanSurface {
        instance_extensions: vec![ash::khr::surface::NAME, ash::khr::android_surface::NAME],
        create_surface: Box::new(move |entry, instance| {
            let native = made_for.get();
            if native == 0 {
                return None;
            }
            let loader = ash::khr::android_surface::Instance::new(entry, instance);
            let info = vk::AndroidSurfaceCreateInfoKHR::default().window(native as *mut vk::ANativeWindow);
            // SAFETY: the instance was made with VK_KHR_android_surface (this surface's instance_extensions); the
            // window is alive (the platform holds a reference on it until the device let go of this surface).
            unsafe { loader.create_android_surface(&info, None) }.ok()
        }),
        native_window: Some(Box::new(move || window.get())),
    }
}
