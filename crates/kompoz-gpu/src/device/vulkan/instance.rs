//! The instance (with validation's messenger, and the window's surface) and the GPU picked on it.

use super::VulkanSurface;
use crate::device::{env_is_one, log};
use ash::vk;
use std::ffi::{CStr, c_char, c_void};
use std::sync::atomic::{AtomicU32, Ordering};

const VALIDATION_LAYER: &CStr = c"VK_LAYER_KHRONOS_validation";

/// What lives as long as the device and is let go of after it: the loader, the instance, validation's messenger, and
/// the surface presented to (with what makes it again for another native window).
pub(super) struct Instance {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    debug_utils: Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
    /// Validation errors reported (the messenger's user data: boxed, where it stays).
    pub errors: Box<AtomicU32>,
    pub surface_fn: Option<ash::khr::surface::Instance>,
    pub surface: vk::SurfaceKHR,
    pub window: Option<VulkanSurface>,
    /// What `surface` was made for ([`VulkanSurface::native_window`]'s).
    pub surface_window: usize,
}

impl Instance {
    /// The instance (Vulkan 1.1, validation's layer and messenger with `KOMPOZ_GPU_VALIDATION=1` where the layer is
    /// there) and the window's surface, if there is a window.
    pub fn new(window: Option<VulkanSurface>) -> Result<Instance, String> {
        // SAFETY: loads the system's Vulkan loader, whose initialisation is the platform's own.
        let entry = unsafe { ash::Entry::load() }.map_err(|e| format!("no Vulkan loader ({e})"))?;
        let mut validation = false;
        if env_is_one("KOMPOZ_GPU_VALIDATION") {
            // SAFETY: a query of the loader's.
            let layers = unsafe { entry.enumerate_instance_layer_properties() }.unwrap_or_default();
            validation = layers.iter().any(|l| l.layer_name_as_c_str() == Ok(VALIDATION_LAYER));
            if !validation {
                log("kompoz: KOMPOZ_GPU_VALIDATION=1 but there is no VK_LAYER_KHRONOS_validation");
            }
        }
        let mut extensions: Vec<*const c_char> =
            window.iter().flat_map(|w| w.instance_extensions.iter().map(|e| e.as_ptr())).collect();
        if validation {
            extensions.push(ash::ext::debug_utils::NAME.as_ptr());
        }
        let layers = [VALIDATION_LAYER.as_ptr()];
        let app = vk::ApplicationInfo::default()
            .application_name(c"Kompoz")
            .engine_name(c"Kompoz")
            .api_version(vk::API_VERSION_1_1);
        let info = vk::InstanceCreateInfo::default()
            .application_info(&app)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(if validation { &layers } else { &[] });
        // SAFETY: the info and the names it points to live through the call.
        let instance = unsafe { entry.create_instance(&info, None) }
            .map_err(|e| format!("vkCreateInstance failed ({})", e.as_raw()))?;
        let mut made = Instance {
            entry,
            instance,
            debug_utils: None,
            errors: Box::new(AtomicU32::new(0)),
            surface_fn: None,
            surface: vk::SurfaceKHR::null(),
            window: None,
            surface_window: 0,
        };
        if validation {
            let debug_utils = ash::ext::debug_utils::Instance::new(&made.entry, &made.instance);
            let info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::WARNING | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(validation_message))
                .user_data(std::ptr::from_ref::<AtomicU32>(&made.errors).cast_mut().cast::<c_void>());
            // SAFETY: the instance has the extension; the user data (the boxed counter) outlives the messenger, which
            // is destroyed before it (in drop).
            if let Ok(messenger) = unsafe { debug_utils.create_debug_utils_messenger(&info, None) } {
                made.debug_utils = Some((debug_utils, messenger));
            }
        }
        if let Some(window) = window {
            made.surface_fn = Some(ash::khr::surface::Instance::new(&made.entry, &made.instance));
            made.surface_window = window.native_window.as_ref().map_or(0, |w| w());
            made.surface =
                (window.create_surface)(&made.entry, &made.instance).ok_or("no Vulkan surface for the window")?;
            made.window = Some(window);
        }
        Ok(made)
    }

    pub fn validation_errors(&self) -> u32 {
        self.errors.load(Ordering::Relaxed)
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: what is destroyed here was made on this instance, and everything made from it (the device) is gone.
        unsafe {
            if let Some(surface_fn) = &self.surface_fn {
                surface_fn.destroy_surface(self.surface, None);
            }
            if let Some((debug_utils, messenger)) = &self.debug_utils {
                debug_utils.destroy_debug_utils_messenger(*messenger, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}

/// Validation's messages logged, its errors counted.
unsafe extern "system" fn validation_message(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _types: vk::DebugUtilsMessageTypeFlagsEXT,
    data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    user: *mut c_void,
) -> vk::Bool32 {
    // SAFETY: the layer passes its callback data (with a message) for the call's length, and the user data given
    // with the messenger: the instance's boxed counter, alive while the messenger is.
    unsafe {
        let message = data.as_ref().and_then(|d| d.message_as_c_str()).map_or("".into(), CStr::to_string_lossy);
        log(&format!("kompoz: vulkan: {message}"));
        if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
            && let Some(errors) = user.cast::<AtomicU32>().as_ref()
        {
            errors.fetch_add(1, Ordering::Relaxed);
        }
    }
    vk::FALSE
}

/// The GPU picked and what about it the device keeps.
pub(super) struct Choice {
    pub physical: vk::PhysicalDevice,
    pub properties: vk::PhysicalDeviceProperties,
    pub queue_family: u32,
    /// Its queue writes timestamps that mean something.
    pub timestamps_valid: bool,
}

impl Choice {
    pub fn device_name(&self) -> String {
        self.properties.device_name_as_c_str().map_or_else(|_| String::new(), |n| n.to_string_lossy().into_owned())
    }
}

/// A GPU with Vulkan 1.1 and a queue that draws (and presents to the surface, if there is one): the one
/// `KOMPOZ_GPU_DEVICE` names part of, else an integrated one (it saves power), else a discrete one, else any, a
/// CPU's last.
pub(super) fn pick_physical_device(instance: &Instance) -> Option<Choice> {
    let wanted = kompoz_runtime::environment::var("KOMPOZ_GPU_DEVICE").filter(|w| !w.is_empty());
    let surface = instance.surface_fn.as_ref().map(|f| (f, instance.surface));
    // SAFETY: queries of the instance's physical devices.
    unsafe {
        let devices = instance.instance.enumerate_physical_devices().unwrap_or_default();
        let mut best: Option<(i32, Choice)> = None;
        for physical in devices {
            let properties = instance.instance.get_physical_device_properties(physical);
            if properties.api_version < vk::API_VERSION_1_1 {
                continue;
            }
            if surface.is_some() {
                let extensions = instance.instance.enumerate_device_extension_properties(physical).unwrap_or_default();
                if !extensions.iter().any(|e| e.extension_name_as_c_str() == Ok(ash::khr::swapchain::NAME)) {
                    continue;
                }
            }
            let families = instance.instance.get_physical_device_queue_family_properties(physical);
            for (q, family) in families.iter().enumerate() {
                let presents = match surface {
                    Some((surface_fn, surface)) => {
                        surface_fn.get_physical_device_surface_support(physical, q as u32, surface).unwrap_or(false)
                    }
                    None => true,
                };
                if !family.queue_flags.contains(vk::QueueFlags::GRAPHICS) || !presents {
                    continue;
                }
                let name = properties.device_name_as_c_str().map(CStr::to_string_lossy).unwrap_or_default();
                let rank = if wanted.as_deref().is_some_and(|w| name.contains(w)) {
                    10
                } else {
                    match properties.device_type {
                        vk::PhysicalDeviceType::INTEGRATED_GPU => 3,
                        vk::PhysicalDeviceType::DISCRETE_GPU => 2,
                        vk::PhysicalDeviceType::CPU => 0,
                        _ => 1,
                    }
                };
                if best.as_ref().is_none_or(|(r, _)| rank > *r) {
                    let timestamps_valid = family.timestamp_valid_bits > 0 && properties.limits.timestamp_period > 0.0;
                    best = Some((rank, Choice { physical, properties, queue_family: q as u32, timestamps_valid }));
                }
                break;
            }
        }
        best.map(|(_, choice)| choice)
    }
}
