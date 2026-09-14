use objc2_av_foundation::{
    AVCaptureDevice, AVCaptureDeviceDiscoverySession, AVCaptureDeviceFormat,
    AVCaptureDevicePosition, AVCaptureDeviceTypeBuiltInWideAngleCamera,
    AVCaptureDeviceTypeContinuityCamera, AVCaptureDeviceTypeExternal, AVCaptureExposureMode,
    AVCaptureFocusMode, AVCaptureWhiteBalanceMode, AVMediaTypeVideo,
};
use objc2_core_media::CMVideoFormatDescriptionGetDimensions;
use objc2_foundation::NSArray;

use super::{
    CameraActiveFormat, CameraAvailability, CameraBackend, CameraDevice, CameraFormat,
    CameraFrameRateRange, CameraFrameSize, CameraPosition,
};

const VIRTUAL_TRANSPORT_TYPE: i32 = i32::from_be_bytes(*b"virt");

pub struct MacOSBackend;

impl MacOSBackend {
    pub fn new() -> Self {
        Self
    }
}

impl CameraBackend for MacOSBackend {
    fn devices(&self) -> Result<Vec<CameraDevice>, String> {
        let (session, devices) = unsafe {
            let media_type = AVMediaTypeVideo.ok_or("AVMediaTypeVideo is unavailable")?;
            let device_types = NSArray::from_slice(&[
                AVCaptureDeviceTypeBuiltInWideAngleCamera,
                AVCaptureDeviceTypeExternal,
                AVCaptureDeviceTypeContinuityCamera,
            ]);
            let session =
                AVCaptureDeviceDiscoverySession::discoverySessionWithDeviceTypes_mediaType_position(
                    &device_types,
                    Some(media_type),
                    AVCaptureDevicePosition::Unspecified,
                );
            let devices = session.devices();

            (session, devices)
        };

        let cameras = devices
            .iter()
            .filter_map(|device| {
                let name = unsafe { device.localizedName() };
                let id = unsafe { device.uniqueID() };
                let manufacturer = unsafe { device.manufacturer() };
                let model = unsafe { device.modelID() };
                let transport_type = unsafe { device.transportType() };

                if is_virtual_camera(
                    transport_type,
                    &name.to_string(),
                    &manufacturer.to_string(),
                    &model.to_string(),
                ) {
                    return None;
                }

                Some(CameraDevice {
                    id: id.to_string(),
                    name: name.to_string(),
                    manufacturer: non_empty(&manufacturer.to_string()),
                    model: non_empty(&model.to_string()),
                    serial_number: None,
                    transport: Some(fourcc_string(transport_type as u32)),
                    device_type: Some(unsafe { device.deviceType() }.to_string()),
                    position: camera_position(unsafe { device.position() }),
                    connected: unsafe { device.isConnected() },
                    in_use: Some(unsafe { device.isInUseByAnotherApplication() }),
                    in_use_by: Vec::new(),
                    suspended: Some(unsafe { device.isSuspended() }),
                    driver: None,
                    capability_bits: None,
                    capabilities: camera_capabilities(&device),
                    formats: unsafe { camera_formats(&device) },
                    active_format: unsafe { active_format(&device) },
                    controls: Vec::new(),
                })
            })
            .collect();

        drop(session);
        Ok(cameras)
    }

    fn availability(&self, device_ids: &[String]) -> Result<Vec<CameraAvailability>, String> {
        let devices = self.devices()?;
        Ok(device_ids
            .iter()
            .map(|device_id| {
                let device = devices.iter().find(|device| device.id == *device_id);
                CameraAvailability {
                    id: device_id.clone(),
                    connected: device.is_some_and(|device| device.connected),
                    in_use: device.and_then(|device| device.in_use).unwrap_or(false),
                    in_use_by: Vec::new(),
                }
            })
            .collect())
    }

    fn set_control(
        &self,
        _device_id: &str,
        _control_id: u32,
        _value: serde_json::Value,
    ) -> Result<Vec<super::CameraControl>, String> {
        Err("Camera controls are not available on macOS yet".to_owned())
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn fourcc_string(value: u32) -> String {
    let bytes = value.to_be_bytes();
    if bytes
        .iter()
        .all(|byte| byte.is_ascii_graphic() || *byte == b' ')
    {
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        format!("0x{value:08x}")
    }
}

fn camera_position(position: AVCaptureDevicePosition) -> CameraPosition {
    match position {
        AVCaptureDevicePosition::Front => CameraPosition::Front,
        AVCaptureDevicePosition::Back => CameraPosition::Back,
        _ => CameraPosition::Unspecified,
    }
}

fn camera_capabilities(device: &AVCaptureDevice) -> Vec<String> {
    let mut capabilities = vec!["videoCapture".to_owned(), "streaming".to_owned()];

    unsafe {
        if device.hasTorch() {
            capabilities.push("torch".to_owned());
        }
        if [
            AVCaptureFocusMode::Locked,
            AVCaptureFocusMode::AutoFocus,
            AVCaptureFocusMode::ContinuousAutoFocus,
        ]
        .into_iter()
        .any(|mode| device.isFocusModeSupported(mode))
        {
            capabilities.push("focus".to_owned());
        }
        if [
            AVCaptureExposureMode::Locked,
            AVCaptureExposureMode::AutoExpose,
            AVCaptureExposureMode::ContinuousAutoExposure,
            AVCaptureExposureMode::Custom,
        ]
        .into_iter()
        .any(|mode| device.isExposureModeSupported(mode))
        {
            capabilities.push("exposure".to_owned());
        }
        if [
            AVCaptureWhiteBalanceMode::Locked,
            AVCaptureWhiteBalanceMode::AutoWhiteBalance,
            AVCaptureWhiteBalanceMode::ContinuousAutoWhiteBalance,
        ]
        .into_iter()
        .any(|mode| device.isWhiteBalanceModeSupported(mode))
        {
            capabilities.push("whiteBalance".to_owned());
        }
        if device.maxAvailableVideoZoomFactor() > device.minAvailableVideoZoomFactor() {
            capabilities.push("zoom".to_owned());
        }
    }

    capabilities
}

unsafe fn camera_formats(device: &AVCaptureDevice) -> Vec<CameraFormat> {
    unsafe { device.formats() }
        .iter()
        .map(|format| unsafe { camera_format(&format) })
        .collect()
}

unsafe fn camera_format(format: &AVCaptureDeviceFormat) -> CameraFormat {
    let description = unsafe { format.formatDescription() };
    let dimensions = unsafe { CMVideoFormatDescriptionGetDimensions(&description) };
    let frame_rates = unsafe { format.videoSupportedFrameRateRanges() }
        .iter()
        .filter_map(|range| {
            let min = unsafe { range.minFrameRate() };
            let max = unsafe { range.maxFrameRate() };
            (min.is_finite() && max.is_finite()).then_some(CameraFrameRateRange { min, max })
        })
        .collect();

    CameraFormat {
        pixel_format: fourcc_string(unsafe { description.media_sub_type() }),
        description: None,
        compressed: false,
        emulated: false,
        flag_bits: None,
        frame_sizes: vec![CameraFrameSize::Discrete {
            width: dimensions.width.max(0) as u32,
            height: dimensions.height.max(0) as u32,
            frame_rates,
        }],
        field_of_view: finite_f32(unsafe { format.videoFieldOfView() }),
        binned: Some(unsafe { format.isVideoBinned() }),
        hdr: Some(unsafe { format.isVideoHDRSupported() }),
        min_iso: finite_f32(unsafe { format.minISO() }),
        max_iso: finite_f32(unsafe { format.maxISO() }),
    }
}

unsafe fn active_format(device: &AVCaptureDevice) -> Option<CameraActiveFormat> {
    let format = unsafe { device.activeFormat() };
    let description = unsafe { format.formatDescription() };
    let dimensions = unsafe { CMVideoFormatDescriptionGetDimensions(&description) };
    let duration = unsafe { device.activeVideoMinFrameDuration().seconds() };
    let frame_rate = (duration.is_finite() && duration > 0.0).then(|| 1.0 / duration);

    Some(CameraActiveFormat {
        pixel_format: fourcc_string(unsafe { description.media_sub_type() }),
        width: dimensions.width.max(0) as u32,
        height: dimensions.height.max(0) as u32,
        frame_rate,
        stride: None,
        image_size: None,
        field_order: None,
        color_space: None,
        quantization: None,
        transfer_function: None,
    })
}

fn finite_f32(value: f32) -> Option<f32> {
    value.is_finite().then_some(value)
}

fn is_virtual_camera(transport_type: i32, name: &str, manufacturer: &str, model: &str) -> bool {
    if transport_type == VIRTUAL_TRANSPORT_TYPE {
        return true;
    }

    [name, manufacturer, model].iter().any(|value| {
        let value = value.to_lowercase();
        value.contains("virtual camera") || value.contains("virtual webcam")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_virtual_transport_and_names() {
        assert!(is_virtual_camera(
            VIRTUAL_TRANSPORT_TYPE,
            "Camera",
            "Vendor",
            "Model"
        ));
        assert!(is_virtual_camera(0, "OBS Virtual Camera", "OBS", "Model"));
        assert!(!is_virtual_camera(
            i32::from_be_bytes(*b"usb "),
            "Studio Camera",
            "Vendor",
            "Model"
        ));
    }
}
