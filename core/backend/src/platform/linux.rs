use std::collections::{BTreeSet, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use serde_json::json;
use v4l::Device;
use v4l::capability::Flags;
use v4l::control::{Control, Flags as ControlFlags, MenuItem, Type as ControlType, Value};
use v4l::format::description::Flags as FormatFlags;
use v4l::frameinterval::FrameIntervalEnum;
use v4l::framesize::FrameSizeEnum;
use v4l::video::Capture;

use super::{
    CameraActiveFormat, CameraAvailability, CameraBackend, CameraControl, CameraControlMenuItem,
    CameraDevice, CameraDriver, CameraFormat, CameraFrameRateRange, CameraFrameSize,
    CameraPosition,
};

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

            if is_virtual_camera(&capabilities.driver, &name, &capabilities.bus) {
                continue;
            }

            let hardware = linux_hardware_info(&path);
            let in_use_by = device_users(&path);
            let formats = camera_formats(&device);
            let active_format = active_format(&device);
            let controls = camera_controls(&device);

            devices.push(DiscoveredDevice {
                camera: CameraDevice {
                    id: path_string,
                    name,
                    manufacturer: hardware.manufacturer,
                    model: hardware.model,
                    serial_number: hardware.serial_number,
                    transport: non_empty_trimmed(&capabilities.bus),
                    device_type: Some("videoCapture".to_owned()),
                    position: CameraPosition::Unspecified,
                    connected: true,
                    in_use: Some(!in_use_by.is_empty()),
                    in_use_by,
                    suspended: None,
                    driver: Some(CameraDriver {
                        name: capabilities.driver.clone(),
                        version: format!(
                            "{}.{}.{}",
                            capabilities.version.0, capabilities.version.1, capabilities.version.2
                        ),
                        bus_info: capabilities.bus.clone(),
                    }),
                    capability_bits: Some(capabilities.capabilities.bits()),
                    capabilities: capability_names(capabilities.capabilities),
                    formats,
                    active_format,
                    controls,
                },
                bus: capabilities.bus,
                index,
            });
        }

        Ok(structure_devices(devices))
    }

    fn availability(&self, device_ids: &[String]) -> Result<Vec<CameraAvailability>, String> {
        Ok(device_ids
            .iter()
            .map(|device_id| {
                let path = Path::new(device_id);
                let connected = path.exists();
                let in_use_by = if connected {
                    device_users(path)
                } else {
                    Vec::new()
                };
                CameraAvailability {
                    id: device_id.clone(),
                    connected,
                    in_use: !in_use_by.is_empty(),
                    in_use_by,
                }
            })
            .collect())
    }

    fn set_control(
        &self,
        device_id: &str,
        control_id: u32,
        value: serde_json::Value,
    ) -> Result<Vec<CameraControl>, String> {
        let device = Device::with_path(device_id).map_err(|error| error.to_string())?;
        let description = device
            .query_controls()
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|control| control.id == control_id)
            .ok_or_else(|| format!("Camera control {control_id} was not found"))?;

        if description
            .flags
            .intersects(ControlFlags::DISABLED | ControlFlags::READ_ONLY | ControlFlags::INACTIVE)
        {
            return Err(format!("{} cannot be changed", description.name));
        }

        let value = match description.typ {
            ControlType::Boolean => Value::Boolean(
                value
                    .as_bool()
                    .ok_or_else(|| format!("{} expects a boolean", description.name))?,
            ),
            ControlType::Integer
            | ControlType::Integer64
            | ControlType::Menu
            | ControlType::IntegerMenu
            | ControlType::Bitmask => Value::Integer(
                value
                    .as_i64()
                    .ok_or_else(|| format!("{} expects an integer", description.name))?,
            ),
            ControlType::Button => Value::None,
            ControlType::String => Value::String(
                value
                    .as_str()
                    .ok_or_else(|| format!("{} expects text", description.name))?
                    .to_owned(),
            ),
            ControlType::CtrlClass
            | ControlType::U8
            | ControlType::U16
            | ControlType::U32
            | ControlType::Area => {
                return Err(format!("{} is not an editable control", description.name));
            }
        };

        device
            .set_control(Control {
                id: control_id,
                value,
            })
            .map_err(|error| error.to_string())?;

        Ok(camera_controls(&device))
    }
}

#[derive(Default)]
struct LinuxHardwareInfo {
    manufacturer: Option<String>,
    model: Option<String>,
    serial_number: Option<String>,
}

fn linux_hardware_info(device_path: &Path) -> LinuxHardwareInfo {
    let Some(file_name) = device_path.file_name() else {
        return LinuxHardwareInfo::default();
    };
    let Ok(sysfs_device) = fs::canonicalize(
        Path::new("/sys/class/video4linux")
            .join(file_name)
            .join("device"),
    ) else {
        return LinuxHardwareInfo::default();
    };

    for ancestor in sysfs_device.ancestors() {
        let manufacturer = read_trimmed(ancestor.join("manufacturer"));
        let model = read_trimmed(ancestor.join("product"));
        let serial_number = read_trimmed(ancestor.join("serial"));
        if manufacturer.is_some() || model.is_some() || serial_number.is_some() {
            return LinuxHardwareInfo {
                manufacturer,
                model,
                serial_number,
            };
        }
    }

    LinuxHardwareInfo::default()
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .and_then(|value| non_empty_trimmed(&value))
}

fn device_users(device_path: &Path) -> Vec<String> {
    let Ok(device_path) = fs::canonicalize(device_path) else {
        return Vec::new();
    };
    let own_pid = std::process::id();
    let mut users = BTreeSet::new();
    let Ok(processes) = fs::read_dir("/proc") else {
        return Vec::new();
    };

    for process in processes.filter_map(Result::ok) {
        let Some(pid) = process
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == own_pid {
            continue;
        }
        let descriptors = process.path().join("fd");
        let Ok(descriptors) = fs::read_dir(descriptors) else {
            continue;
        };
        let owns_device = descriptors.filter_map(Result::ok).any(|descriptor| {
            fs::read_link(descriptor.path())
                .ok()
                .and_then(|target| fs::canonicalize(target).ok())
                .is_some_and(|target| target == device_path)
        });
        if owns_device {
            users.insert(process_display_name(&process.path(), pid));
        }
    }

    users.into_iter().collect()
}

fn process_display_name(process_path: &Path, pid: u32) -> String {
    let name = read_trimmed(process_path.join("comm"))
        .or_else(|| {
            fs::read(process_path.join("cmdline"))
                .ok()
                .and_then(|bytes| {
                    let executable = bytes.split(|byte| *byte == 0).next()?;
                    let executable = Path::new(std::str::from_utf8(executable).ok()?);
                    executable.file_name()?.to_str().map(ToOwned::to_owned)
                })
        })
        .unwrap_or_else(|| format!("process {pid}"));
    let name = name.trim_end_matches(".bin").trim_end_matches(".exe");
    let mut characters = name.chars();
    characters
        .next()
        .map(|first| first.to_uppercase().chain(characters).collect())
        .unwrap_or_else(|| format!("process {pid}"))
}

fn fourcc_string(fourcc: v4l::FourCC) -> String {
    fourcc
        .str()
        .map(str::to_owned)
        .unwrap_or_else(|_| format!("0x{:08x}", u32::from(fourcc)))
}

fn camera_formats(device: &Device) -> Vec<CameraFormat> {
    device
        .enum_formats()
        .unwrap_or_default()
        .into_iter()
        .map(|format| {
            let frame_sizes = device
                .enum_framesizes(format.fourcc)
                .unwrap_or_default()
                .into_iter()
                .map(|size| match size.size {
                    FrameSizeEnum::Discrete(size) => CameraFrameSize::Discrete {
                        width: size.width,
                        height: size.height,
                        frame_rates: frame_rates(device, format.fourcc, size.width, size.height),
                    },
                    FrameSizeEnum::Stepwise(size) => CameraFrameSize::Stepwise {
                        min_width: size.min_width,
                        max_width: size.max_width,
                        width_step: size.step_width,
                        min_height: size.min_height,
                        max_height: size.max_height,
                        height_step: size.step_height,
                        // V4L2 only accepts a concrete size for frame-interval queries.
                        // Querying the largest advertised size gives callers a useful
                        // conservative range while the size bounds remain lossless.
                        frame_rates: frame_rates(
                            device,
                            format.fourcc,
                            size.max_width,
                            size.max_height,
                        ),
                    },
                })
                .collect();

            CameraFormat {
                pixel_format: fourcc_string(format.fourcc),
                description: non_empty_trimmed(&format.description),
                compressed: format.flags.contains(FormatFlags::COMPRESSED),
                emulated: format.flags.contains(FormatFlags::EMULATED),
                flag_bits: Some(format.flags.bits()),
                frame_sizes,
                field_of_view: None,
                binned: None,
                hdr: None,
                min_iso: None,
                max_iso: None,
            }
        })
        .collect()
}

fn frame_rates(
    device: &Device,
    fourcc: v4l::FourCC,
    width: u32,
    height: u32,
) -> Vec<CameraFrameRateRange> {
    device
        .enum_frameintervals(fourcc, width, height)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|interval| match interval.interval {
            FrameIntervalEnum::Discrete(interval) => rate(interval.numerator, interval.denominator)
                .map(|fps| CameraFrameRateRange { min: fps, max: fps }),
            FrameIntervalEnum::Stepwise(interval) => Some(CameraFrameRateRange {
                min: rate(interval.max.numerator, interval.max.denominator)?,
                max: rate(interval.min.numerator, interval.min.denominator)?,
            }),
        })
        .collect()
}

fn rate(numerator: u32, denominator: u32) -> Option<f64> {
    (numerator != 0).then(|| denominator as f64 / numerator as f64)
}

fn active_format(device: &Device) -> Option<CameraActiveFormat> {
    let format = device.format().ok()?;
    let frame_rate = device
        .params()
        .ok()
        .and_then(|params| rate(params.interval.numerator, params.interval.denominator));

    Some(CameraActiveFormat {
        pixel_format: fourcc_string(format.fourcc),
        width: format.width,
        height: format.height,
        frame_rate,
        stride: Some(format.stride),
        image_size: Some(format.size),
        field_order: Some(format.format_field()),
        color_space: Some(format.format_colorspace()),
        quantization: Some(format.format_quantization()),
        transfer_function: Some(format.format_transfer()),
    })
}

trait FormatLabels {
    fn format_field(&self) -> String;
    fn format_colorspace(&self) -> String;
    fn format_quantization(&self) -> String;
    fn format_transfer(&self) -> String;
}

impl FormatLabels for v4l::Format {
    fn format_field(&self) -> String {
        format!("{:?}", self.field_order)
    }

    fn format_colorspace(&self) -> String {
        format!("{:?}", self.colorspace)
    }

    fn format_quantization(&self) -> String {
        format!("{:?}", self.quantization)
    }

    fn format_transfer(&self) -> String {
        format!("{:?}", self.transfer)
    }
}

fn camera_controls(device: &Device) -> Vec<CameraControl> {
    device
        .query_controls()
        .unwrap_or_default()
        .into_iter()
        .map(|control| {
            let value = device
                .control(control.id)
                .ok()
                .map(|control| control_value_json(control.value));
            let menu_items = control
                .items
                .unwrap_or_default()
                .into_iter()
                .map(|(index, item)| match item {
                    MenuItem::Name(name) => CameraControlMenuItem {
                        index,
                        name: Some(name),
                        value: None,
                    },
                    MenuItem::Value(value) => CameraControlMenuItem {
                        index,
                        name: None,
                        value: Some(value),
                    },
                })
                .collect();

            CameraControl {
                id: control.id,
                name: control.name,
                control_type: format!("{:?}", control.typ),
                minimum: control.minimum,
                maximum: control.maximum,
                step: control.step,
                default: control.default,
                value,
                flag_bits: control.flags.bits(),
                flags: control_flag_names(control.flags),
                menu_items,
            }
        })
        .collect()
}

fn control_value_json(value: Value) -> serde_json::Value {
    match value {
        Value::None => serde_json::Value::Null,
        Value::Integer(value) => json!(value),
        Value::Boolean(value) => json!(value),
        Value::String(value) => json!(value),
        Value::CompoundU8(value) => json!(value),
        Value::CompoundU16(value) => json!(value),
        Value::CompoundU32(value) => json!(value),
        Value::CompoundPtr(value) => json!(value),
    }
}

fn capability_names(flags: Flags) -> Vec<String> {
    let known = [
        (Flags::VIDEO_CAPTURE, "videoCapture"),
        (Flags::VIDEO_CAPTURE_MPLANE, "videoCaptureMultiPlanar"),
        (Flags::READ_WRITE, "readWrite"),
        (Flags::ASYNC_IO, "asyncIo"),
        (Flags::STREAMING, "streaming"),
        (Flags::EXT_PIX_FORMAT, "extendedPixelFormat"),
        (Flags::AUDIO, "audio"),
    ];

    known
        .into_iter()
        .filter(|(flag, _)| flags.contains(*flag))
        .map(|(_, name)| name.to_owned())
        .collect()
}

fn control_flag_names(flags: ControlFlags) -> Vec<String> {
    let known = [
        (ControlFlags::DISABLED, "disabled"),
        (ControlFlags::GRABBED, "grabbed"),
        (ControlFlags::READ_ONLY, "readOnly"),
        (ControlFlags::UPDATE, "update"),
        (ControlFlags::INACTIVE, "inactive"),
        (ControlFlags::SLIDER, "slider"),
        (ControlFlags::WRITE_ONLY, "writeOnly"),
        (ControlFlags::VOLATILE, "volatile"),
        (ControlFlags::HAS_PAYLOAD, "hasPayload"),
        (ControlFlags::EXECUTE_ON_WRITE, "executeOnWrite"),
        (ControlFlags::MODIFY_LAYOUT, "modifyLayout"),
    ];

    known
        .into_iter()
        .filter(|(flag, _)| flags.contains(*flag))
        .map(|(_, name)| name.to_owned())
        .collect()
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

fn is_virtual_camera(driver: &str, name: &str, bus: &str) -> bool {
    let driver = identity_part(driver);
    let name = identity_part(name);
    let bus = identity_part(bus);

    [
        "v4l2loopback",
        "v4l2 loopback",
        "akvcam",
        "vivid",
        "vimc",
        "vicodec",
    ]
    .iter()
    .any(|virtual_driver| driver.contains(virtual_driver))
        || bus.contains("v4l2loopback")
        || name.contains("virtual camera")
        || name.contains("virtual webcam")
}

fn structure_devices(mut devices: Vec<DiscoveredDevice>) -> Vec<CameraDevice> {
    devices.sort_by_key(|device| device.index);

    let mut seen = HashSet::new();
    devices.retain(|device| {
        let bus = identity_part(&device.bus);

        // bus_info identifies the physical connection, while card identifies
        // a capture endpoint on that connection (for example RGB versus IR)
        //
        // do not guess when drivers omit bus_info; distinct capture devices
        // can have identical names
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
                manufacturer: None,
                model: None,
                serial_number: None,
                transport: non_empty_trimmed(bus),
                device_type: Some("videoCapture".to_owned()),
                position: CameraPosition::Unspecified,
                connected: true,
                in_use: None,
                in_use_by: Vec::new(),
                suspended: None,
                driver: None,
                capability_bits: None,
                capabilities: Vec::new(),
                formats: Vec::new(),
                active_format: None,
                controls: Vec::new(),
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
    fn rejects_virtual_camera_drivers_and_names() {
        assert!(is_virtual_camera(
            "v4l2 loopback",
            "OBS Virtual Camera",
            "platform:v4l2loopback-000"
        ));
        assert!(is_virtual_camera("akvcam", "Camera", "platform:akvcam"));
        assert!(is_virtual_camera("uvcvideo", "Virtual Webcam", "usb-1"));
        assert!(!is_virtual_camera(
            "uvcvideo",
            "EMEET SmartCam C60E 4K",
            "usb-0000:00:14.0-2"
        ));
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
    fn preserves_distinct_devices_without_bus_information() {
        let devices = structure_devices(vec![
            discovered(0, "Capture Device", ""),
            discovered(1, "Capture Device", ""),
        ]);

        assert_eq!(devices.len(), 2);
    }
}
