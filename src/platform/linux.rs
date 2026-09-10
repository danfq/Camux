use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use v4l::Device;
use v4l::capability::Flags;

use super::{CameraBackend, CameraDevice};

#[derive(Debug)]
struct DiscoveredDevice {
    camera: CameraDevice,
    bus: String,
    index: u64,
}

pub struct LinuxBackend;

impl LinuxBackend {
    pub fn new() -> Self {
        Self
    }
}

impl CameraBackend for LinuxBackend {
    fn devices(&self) -> Result<Vec<CameraDevice>, String> {
        let entries = fs::read_dir("/dev").map_err(|error| error.to_string())?;

        let mut paths = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let index = video_device_index(&entry.file_name())?;
                Some((index, entry.path()))
            })
            .collect::<Vec<_>>();

        // read_dir does not guarantee an order
        // inspecting the lowest numbered node first
        // also makes the node retained during deduplication stable
        paths.sort_by_key(|(index, _)| *index);

        let mut devices = Vec::new();

        for (index, path) in paths {
            let path_string = path.display().to_string();

            let Ok(device) = Device::with_path(&path) else {
                continue;
            };

            let Ok(capabilities) = device.query_caps() else {
                continue;
            };

            if !is_capture_device(capabilities.capabilities) {
                continue;
            }

            let name = non_empty_trimmed(&capabilities.card)
                .unwrap_or_else(|| path_file_name(&path).unwrap_or_else(|| path_string.clone()));

            devices.push(DiscoveredDevice {
                camera: CameraDevice {
                    id: path_string,
                    name,
                },
                bus: capabilities.bus,
                index,
            });
        }

        Ok(structure_devices(devices))
    }
}

fn video_device_index(name: &OsStr) -> Option<u64> {
    let suffix = name.to_str()?.strip_prefix("video")?;

    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    suffix.parse().ok()
}

fn is_capture_device(flags: Flags) -> bool {
    flags.intersects(Flags::VIDEO_CAPTURE | Flags::VIDEO_CAPTURE_MPLANE)
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn path_file_name(path: &Path) -> Option<String> {
    path.file_name()?.to_str().map(ToOwned::to_owned)
}

fn identity_part(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn structure_devices(mut devices: Vec<DiscoveredDevice>) -> Vec<CameraDevice> {
    devices.sort_by_key(|device| device.index);

    let mut seen = HashSet::new();
    devices.retain(|device| {
        let bus = identity_part(&device.bus);

        // bus_info identifies the physical connection, while card identifies
        // a capture endpoint on that connection (for example RGB versus IR)
        //
        // do not guess when drivers omit bus_info; distinct virtual cameras
        // commonly have identical names
        bus.is_empty() || seen.insert((bus, identity_part(&device.camera.name)))
    });

    devices.sort_by(|left, right| {
        left.camera
            .name
            .to_lowercase()
            .cmp(&right.camera.name.to_lowercase())
            .then_with(|| left.index.cmp(&right.index))
    });

    devices.into_iter().map(|device| device.camera).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discovered(index: u64, name: &str, bus: &str) -> DiscoveredDevice {
        DiscoveredDevice {
            camera: CameraDevice {
                id: format!("/dev/video{index}"),
                name: name.to_owned(),
            },
            bus: bus.to_owned(),
            index,
        }
    }

    #[test]
    fn accepts_only_numbered_video_nodes() {
        assert_eq!(video_device_index(OsStr::new("video0")), Some(0));
        assert_eq!(video_device_index(OsStr::new("video12")), Some(12));
        assert_eq!(video_device_index(OsStr::new("video")), None);
        assert_eq!(video_device_index(OsStr::new("video-decoder")), None);
        assert_eq!(video_device_index(OsStr::new("video1-extra")), None);
    }

    #[test]
    fn accepts_single_and_multi_planar_capture_devices() {
        assert!(is_capture_device(Flags::VIDEO_CAPTURE));
        assert!(is_capture_device(Flags::VIDEO_CAPTURE_MPLANE));
        assert!(!is_capture_device(Flags::META_CAPTURE));
        assert!(!is_capture_device(Flags::VIDEO_OUTPUT));
    }

    #[test]
    fn deduplicates_same_endpoint_and_keeps_lowest_numbered_node() {
        let devices = structure_devices(vec![
            discovered(10, "USB Camera", "usb-1"),
            discovered(2, "USB Camera", "usb-1"),
            discovered(4, "USB Camera IR", "usb-1"),
        ]);

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "/dev/video2");
        assert_eq!(devices[1].id, "/dev/video4");
    }

    #[test]
    fn does_not_deduplicate_devices_without_bus_information() {
        let devices = structure_devices(vec![
            discovered(0, "Virtual Camera", ""),
            discovered(1, "Virtual Camera", ""),
        ]);

        assert_eq!(devices.len(), 2);
    }
}
