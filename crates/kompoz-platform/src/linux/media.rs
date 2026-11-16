//! Whether Linux has a microphone and a camera, for the media scope (C++'s media_devices.cpp): a capture device
//! ALSA lists (`/proc/asound/pcm`), a V4L2 device that captures video (`/dev/video*`, asked with VIDIOC_QUERYCAP).
//! Neither opens a stream: nothing asks the user, nothing lights the camera's light.

use super::sys;
use kompoz_ui::media_query::{MediaInfo, PointerPrecision};
use std::os::fd::AsFd;
use std::os::unix::fs::OpenOptionsExt;

/// Whether ALSA has a device that captures ("00-00: ALC269VB Analog : ALC269VB Analog : playback 1 : capture 1").
pub fn has_microphone() -> bool {
    std::fs::read_to_string("/proc/asound/pcm").is_ok_and(|pcm| pcm_captures(&pcm))
}

/// Whether one of `/proc/asound/pcm`'s lines is a capture device's.
pub fn pcm_captures(pcm: &str) -> bool {
    pcm.lines().any(|line| line.contains(": capture "))
}

/// Whether a `/dev/video*` device captures video.
pub fn has_camera() -> bool {
    let Ok(entries) = std::fs::read_dir("/dev") else { return false };
    entries.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("video")).any(|e| {
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(e.path())
            .is_ok_and(|device| sys::v4l2_captures_video(device.as_fd()).unwrap_or(false))
    })
}

/// The microphone and the camera a platform's media info tells, looked for once when first asked to probe (C++'s
/// platforms' `media`), kept after.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MediaDevices {
    probed: bool,
    microphone: bool,
    camera: bool,
}

impl MediaDevices {
    /// The media info (`Platform::media`): the pointer and the keyboard as the platform knows them, a microphone
    /// and a camera as probed (with `probe`, the first time: after the first frame).
    pub fn info(&mut self, probe: bool, pointer_precision: PointerPrecision, physical_keyboard: bool) -> MediaInfo {
        if probe && !self.probed {
            self.probed = true;
            self.microphone = has_microphone();
            self.camera = has_camera();
        }
        MediaInfo {
            pointer_precision,
            physical_keyboard,
            has_microphone: self.microphone,
            has_camera: self.camera,
            ..MediaInfo::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_devices_are_found_in_alsas_list() {
        let pcm = "00-00: ALC269VB Analog : ALC269VB Analog : playback 1 : capture 1\n\
                   00-03: HDMI 0 : HDMI 0 : playback 1\n";
        assert!(pcm_captures(pcm));
        assert!(!pcm_captures("00-03: HDMI 0 : HDMI 0 : playback 1\n"));
        assert!(!pcm_captures(""));
        // Not probed until asked; then kept.
        let mut devices = MediaDevices::default();
        let info = devices.info(false, PointerPrecision::Fine, true);
        assert!(!info.has_microphone && !info.has_camera);
        assert_eq!((info.pointer_precision, info.physical_keyboard), (PointerPrecision::Fine, true));
        let probed = devices.info(true, PointerPrecision::Coarse, false);
        assert_eq!(probed.has_microphone, has_microphone());
        assert_eq!(devices.info(false, PointerPrecision::Coarse, false), probed);
    }
}
