//! The swapchain: made at the drawable's size (UNORM, sRGB non-linear, FIFO, turned as the display is), its image
//! acquired lazily (waiting a while at most: a window the compositor holds every image of misses a frame instead of
//! hanging the app), presented at the frame's end with how old what it holds is, made again when the drawable
//! changes size, the display turns, a present says it no longer fits as well as it could (once: Wayland's after its
//! buffers' best format changes), or the window gets another native window.

use super::VulkanDevice;
use super::texture::VkTexture;
use crate::device::{Format, TextureId, TextureUsage, log};
use ash::vk;
use std::time::Duration;

/// How long an acquire waits for an image at most (the frame isn't shown if none comes).
const ACQUIRE_TIMEOUT: Duration = Duration::from_millis(500);

/// The swapchain and its images.
pub(super) struct Swapchain {
    pub handle: vk::SwapchainKHR,
    pub format: Format,
    /// The surface's size it was made at (its images are turned: `turns`).
    pub extent: vk::Extent2D,
    /// The drawable's size it was made for (what tells a resize; `extent` may differ from it: the surface's own
    /// size, or clamped to what the surface takes); (0, 0): to be made again.
    pub drawable: (i32, i32),
    /// Made again because a present said the last one was suboptimal (if this one is too, it stays).
    pub for_suboptimal: bool,
    /// The display's transform when it was made.
    pub transform: vk::SurfaceTransformFlagsKHR,
    pub turns: i32,
    pub images: Vec<TextureId>,
    /// Per image: signaled when its frame's work is done, waited for by presenting.
    pub render_finished: Vec<vk::Semaphore>,
    /// Per image: `presents` when it was last presented (0: not since it was made).
    pub presented_at: Vec<u64>,
    pub presents: u64,
    /// The acquired image's age ([`crate::device::Device::swapchain_age`]).
    pub age: i32,
    pub index: u32,
    pub acquired: bool,
}

impl Default for Swapchain {
    fn default() -> Swapchain {
        Swapchain {
            handle: vk::SwapchainKHR::null(),
            format: Format::Bgra8,
            extent: vk::Extent2D::default(),
            drawable: (0, 0),
            for_suboptimal: false,
            transform: vk::SurfaceTransformFlagsKHR::IDENTITY,
            turns: 0,
            images: Vec::new(),
            render_finished: Vec::new(),
            presented_at: Vec::new(),
            presents: 0,
            age: 0,
            index: 0,
            acquired: false,
        }
    }
}

impl VulkanDevice {
    /// The swapchain's images' views and framebuffers, and their semaphores, destroyed (the GPU done with them).
    fn destroy_swapchain_images(&mut self) {
        for id in std::mem::take(&mut self.swapchain.images) {
            if let Some(image) = self.textures.remove(id) {
                self.forget_texture(id);
                self.destroy_texture(image);
            }
        }
        for semaphore in std::mem::take(&mut self.swapchain.render_finished) {
            // SAFETY: nothing waits on it any more (the device is idle when images go).
            unsafe { self.device.destroy_semaphore(semaphore, None) };
        }
    }

    pub(super) fn destroy_swapchain(&mut self) {
        self.destroy_swapchain_images();
        if let Some(swapchain_fn) = &self.swapchain_fn
            && self.swapchain.handle != vk::SwapchainKHR::null()
        {
            // SAFETY: the device is idle and the swapchain's images are let go of.
            unsafe { swapchain_fn.destroy_swapchain(self.swapchain.handle, None) };
        }
        self.swapchain.handle = vk::SwapchainKHR::null();
    }

    /// The deletions of the frames not recorded now run (the device idle): what they hold may be the old
    /// swapchain's or surface's.
    fn idle_and_clean(&mut self) {
        // SAFETY: waits for the device.
        unsafe {
            let _ = self.device.device_wait_idle();
        }
        for i in 0..super::frame::FRAMES_IN_FLIGHT {
            if i != self.frame_index {
                self.run_deletions(i);
            }
        }
    }

    /// (Re)makes the swapchain at the drawable's size; false if it is 0 x 0 (minimized) or can't be made.
    fn make_swapchain(&mut self) -> bool {
        let (Some(surface_fn), Some(swapchain_fn)) = (self.instance.surface_fn.clone(), self.swapchain_fn.clone())
        else {
            return false;
        };
        let surface = self.instance.surface;
        let (w, h) = self.drawable_size.as_ref().map_or((0, 0), |size| size());
        // SAFETY: queries of the surface, made on this instance for this physical device's queue.
        let Ok(caps) = (unsafe { surface_fn.get_physical_device_surface_capabilities(self.choice.physical, surface) })
        else {
            return false;
        };
        let extent = if caps.current_extent.width != u32::MAX {
            caps.current_extent
        } else {
            vk::Extent2D {
                width: (w.max(0) as u32).clamp(caps.min_image_extent.width, caps.max_image_extent.width),
                height: (h.max(0) as u32).clamp(caps.min_image_extent.height, caps.max_image_extent.height),
            }
        };
        if extent.width == 0 || extent.height == 0 {
            return false;
        }
        self.idle_and_clean();
        let old = self.swapchain.handle;
        self.destroy_swapchain_images();

        // SAFETY: as above.
        let formats = unsafe { surface_fn.get_physical_device_surface_formats(self.choice.physical, surface) }
            .unwrap_or_default();
        // UNORM: blending as Kompoz always has.
        let chosen = formats
            .iter()
            .find(|f| {
                (f.format == vk::Format::B8G8R8A8_UNORM || f.format == vk::Format::R8G8B8A8_UNORM)
                    && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .or(formats.first())
            .copied()
            .unwrap_or(vk::SurfaceFormatKHR {
                format: vk::Format::B8G8R8A8_UNORM,
                color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
            });
        self.swapchain.format = if chosen.format == vk::Format::R8G8B8A8_UNORM { Format::Rgba8 } else { Format::Bgra8 };
        let mut min_images = caps.min_image_count.max(2) + 1;
        if caps.max_image_count > 0 {
            min_images = min_images.min(caps.max_image_count);
        }
        // Turned as the display is, by us: its images the display's way up, the content drawn turned into them
        // (swapchain_turns), not turned by the compositor every frame (a present it has to turn is "suboptimal"). A
        // mirrored display is left to it.
        let mut transform = caps.current_transform;
        self.swapchain.turns = match transform {
            vk::SurfaceTransformFlagsKHR::ROTATE_90 => 1,
            vk::SurfaceTransformFlagsKHR::ROTATE_180 => 2,
            vk::SurfaceTransformFlagsKHR::ROTATE_270 => 3,
            _ => 0,
        };
        if self.swapchain.turns == 0 && caps.supported_transforms.contains(vk::SurfaceTransformFlagsKHR::IDENTITY) {
            transform = vk::SurfaceTransformFlagsKHR::IDENTITY;
        }
        let image_extent = if self.swapchain.turns & 1 == 1 {
            vk::Extent2D { width: extent.height, height: extent.width }
        } else {
            extent
        };
        let composite_alpha = composite_alpha(caps.supported_composite_alpha, self.transparent);
        let info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(min_images)
            .image_format(chosen.format)
            .image_color_space(chosen.color_space)
            .image_extent(image_extent)
            .image_array_layers(1)
            .image_usage(
                vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | (caps.supported_usage_flags & vk::ImageUsageFlags::TRANSFER_SRC),
            )
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(transform)
            .composite_alpha(composite_alpha)
            // (Vsync, always there.)
            .present_mode(vk::PresentModeKHR::FIFO)
            // (What is kept in an image: all of it, a part hidden by another window too.)
            .clipped(false)
            .old_swapchain(old);
        // SAFETY: a valid swapchain info for the surface; the old swapchain is retired by it and destroyed after
        // (the device is idle, its images let go of).
        let made = unsafe {
            let made = swapchain_fn.create_swapchain(&info, None);
            if old != vk::SwapchainKHR::null() {
                swapchain_fn.destroy_swapchain(old, None);
            }
            made
        };
        self.swapchain.handle = vk::SwapchainKHR::null();
        let swapchain = match made {
            Ok(swapchain) => swapchain,
            Err(e) => {
                log(&format!("kompoz: vkCreateSwapchainKHR failed ({})", e.as_raw()));
                return false;
            }
        };
        self.swapchain.handle = swapchain;
        self.swapchain.extent = extent;
        self.swapchain.drawable = (w, h);
        self.swapchain.transform = caps.current_transform;
        // SAFETY: the swapchain was just made.
        let images = unsafe { swapchain_fn.get_swapchain_images(swapchain) }.unwrap_or_default();
        self.swapchain.presented_at = vec![0; images.len()];
        for image in images {
            let view = self.make_view(image, chosen.format, 1);
            let id =
                self.textures.reserve(image_extent.width as i32, image_extent.height as i32, self.swapchain.format);
            self.textures.fill(VkTexture {
                id,
                image,
                view,
                allocation: None,
                layout: vk::ImageLayout::UNDEFINED,
                mip_levels: 1,
                usage: TextureUsage::RENDER_TARGET,
                framebuffers: Vec::new(),
                discarded: false,
            });
            self.swapchain.images.push(id);
            // SAFETY: a valid semaphore info; destroyed with the images.
            let semaphore =
                unsafe { self.device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) }.unwrap_or_default();
            self.swapchain.render_finished.push(semaphore);
        }
        true
    }

    /// The window's native window is another (Android's, the app back from the background: the old one is gone, and
    /// presenting to it fails for good), or the surface was lost: the swapchain and the surface made again for it.
    /// False while there is none.
    pub(super) fn surface_current(&mut self) -> bool {
        let Some(native_window) = self.instance.window.as_ref().and_then(|w| w.native_window.as_ref()) else {
            return self.instance.surface != vk::SurfaceKHR::null();
        };
        let now = native_window();
        if now == self.instance.surface_window && !std::mem::take(&mut self.surface_lost) {
            return now != 0;
        }
        self.idle_and_clean();
        self.destroy_swapchain();
        if let Some(surface_fn) = &self.instance.surface_fn {
            // SAFETY: the device is idle and the swapchain made for the surface is gone.
            unsafe { surface_fn.destroy_surface(self.instance.surface, None) };
        }
        self.instance.surface = vk::SurfaceKHR::null();
        self.instance.surface_window = now;
        if now == 0 {
            return false;
        }
        let made = self
            .instance
            .window
            .as_ref()
            .and_then(|w| (w.create_surface)(&self.instance.entry, &self.instance.instance));
        let Some(surface) = made else { return false };
        self.instance.surface = surface;
        // (The queue picked for the first: any of Android's presents to all.)
        let presents = self.instance.surface_fn.as_ref().is_some_and(|f| {
            // SAFETY: a query of the new surface for the device's queue family.
            unsafe { f.get_physical_device_surface_support(self.choice.physical, self.choice.queue_family, surface) }
                .unwrap_or(false)
        });
        if !presents {
            log("kompoz: the window's new surface can't be presented to from the queue picked");
        }
        presents
    }

    pub(super) fn acquire(&mut self) -> Option<TextureId> {
        let window = self.instance.window.as_ref()?;
        if self.instance.surface == vk::SurfaceKHR::null() && window.native_window.is_none() {
            return None;
        }
        if !self.swapchain.acquired {
            if !self.surface_current() || self.instance.surface == vk::SurfaceKHR::null() {
                return None;
            }
            let (w, h) = self.drawable_size.as_ref().map_or((0, 0), |size| size());
            if w <= 0 || h <= 0 {
                return None; // minimized
            }
            // (Made again when the drawable changed, not whenever the surface's size differs from it: a window
            // system whose size lags the drawable's would have it made every frame.)
            if self.swapchain.handle == vk::SwapchainKHR::null() || self.swapchain.drawable != (w, h) {
                if self.swapchain.drawable != (0, 0) {
                    self.swapchain.for_suboptimal = false; // (resized: a new start)
                }
                if !self.make_swapchain() {
                    return None;
                }
            }
            // What was recorded so far (the kept frame drawn) goes to the GPU now: in the submit that waits for the
            // image, it would wait for it too.
            if self.recorded {
                self.submit(vk::Fence::null(), false);
                self.start_commands();
            }
            let swapchain_fn = self.swapchain_fn.as_ref()?;
            let semaphore = self.frames[self.frame_index].image_available;
            let timeout = ACQUIRE_TIMEOUT.as_nanos() as u64;
            // SAFETY: the swapchain is the device's; the frame's semaphore is unsignaled (its last wait was submitted).
            let result = unsafe {
                swapchain_fn.acquire_next_image(self.swapchain.handle, timeout, semaphore, vk::Fence::null())
            };
            match result {
                Ok((index, _suboptimal)) => {
                    self.swapchain.index = index;
                    self.swapchain.acquired = true;
                }
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    // Resized meanwhile: made again at the next frame.
                    self.idle_and_clean();
                    self.destroy_swapchain();
                }
                Err(vk::Result::ERROR_SURFACE_LOST_KHR) => self.surface_lost = true,
                // (TIMEOUT, NOT_READY: no image free yet; the frame isn't shown, the next one tries again.)
                Err(_) => {}
            }
            self.wait_image = self.swapchain.acquired;
            let s = &mut self.swapchain;
            // Presented before: it holds what it was presented with (presenting only reads it, and taken out of the
            // present layout it is as it was), unless it is new.
            let at = s.presented_at.get(s.index as usize).copied().unwrap_or(0);
            s.age = if s.acquired && at != 0 { (s.presents - at + 1).min(1000) as i32 } else { 0 };
            if s.acquired {
                let (id, age) = (s.images[s.index as usize], s.age);
                if let Some(t) = self.textures.get_mut(id) {
                    t.layout = if age != 0 { vk::ImageLayout::PRESENT_SRC_KHR } else { vk::ImageLayout::UNDEFINED };
                }
            }
        }
        self.swapchain.acquired.then(|| self.swapchain.images[self.swapchain.index as usize])
    }

    /// The acquired image presented (its frame submitted, signaling its semaphore).
    pub(super) fn present(&mut self) {
        let Some(swapchain_fn) = self.swapchain_fn.clone() else { return };
        let index = self.swapchain.index;
        let waits = [self.swapchain.render_finished[index as usize]];
        let swapchains = [self.swapchain.handle];
        let indices = [index];
        let info =
            vk::PresentInfoKHR::default().wait_semaphores(&waits).swapchains(&swapchains).image_indices(&indices);
        // SAFETY: the image was acquired and its frame submitted, signaling the semaphore waited for.
        match unsafe { swapchain_fn.queue_present(self.queue, &info) } {
            Ok(true) => {
                // Made again if the display turned since; else once (a Wayland compositor's buffers' best format
                // changed: one made now gets it), and not again if that one is suboptimal too (then it is the
                // compositor's to fit, as it can).
                let turned = self.instance.surface_fn.as_ref().is_some_and(|surface_fn| {
                    // SAFETY: a query of the surface.
                    let caps = unsafe {
                        surface_fn.get_physical_device_surface_capabilities(self.choice.physical, self.instance.surface)
                    };
                    caps.is_ok_and(|c| c.current_transform != self.swapchain.transform)
                });
                if turned || !self.swapchain.for_suboptimal {
                    self.swapchain.drawable = (0, 0);
                    self.swapchain.for_suboptimal = !turned;
                }
            }
            Ok(false) => self.swapchain.for_suboptimal = false,
            // Made again next frame.
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.swapchain.drawable = (0, 0);
                self.swapchain.for_suboptimal = false;
            }
            Err(vk::Result::ERROR_SURFACE_LOST_KHR) => self.surface_lost = true,
            Err(_) => {}
        }
    }
}

/// How the swapchain's images are composited with what is behind the window: an opaque window's the first of
/// opaque, inherited (the window system's own way), premultiplied, straight the surface has; a transparent window's
/// with its alpha — premultiplied (what the renderer draws), else straight, else inherited (Wayland's and X11's
/// 32-bit visuals take the alpha as premultiplied that way), opaque only where the surface has nothing else.
pub(crate) fn composite_alpha(supported: vk::CompositeAlphaFlagsKHR, transparent: bool) -> vk::CompositeAlphaFlagsKHR {
    use vk::CompositeAlphaFlagsKHR as A;
    let order = if transparent {
        [A::PRE_MULTIPLIED, A::POST_MULTIPLIED, A::INHERIT, A::OPAQUE]
    } else {
        [A::OPAQUE, A::INHERIT, A::PRE_MULTIPLIED, A::POST_MULTIPLIED]
    };
    order.into_iter().find(|&a| supported.contains(a)).unwrap_or(A::OPAQUE)
}

#[cfg(test)]
mod composite_alpha_tests {
    use super::*;
    use vk::CompositeAlphaFlagsKHR as A;

    #[test]
    fn a_transparent_window_is_composited_with_its_alpha() {
        let all = A::OPAQUE | A::INHERIT | A::PRE_MULTIPLIED | A::POST_MULTIPLIED;
        assert_eq!(composite_alpha(all, false), A::OPAQUE);
        assert_eq!(composite_alpha(all, true), A::PRE_MULTIPLIED);
        assert_eq!(composite_alpha(A::OPAQUE | A::POST_MULTIPLIED, true), A::POST_MULTIPLIED);
        assert_eq!(composite_alpha(A::OPAQUE | A::INHERIT, true), A::INHERIT);
        assert_eq!(composite_alpha(A::OPAQUE, true), A::OPAQUE);
        assert_eq!(composite_alpha(A::INHERIT | A::PRE_MULTIPLIED, false), A::INHERIT);
        assert_eq!(composite_alpha(A::empty(), true), A::OPAQUE);
    }
}
