use objc2_av_foundation::{
    AVCaptureDeviceDiscoverySession, AVCaptureDevicePosition,
    AVCaptureDeviceTypeBuiltInWideAngleCamera, AVCaptureDeviceTypeContinuityCamera,
    AVCaptureDeviceTypeExternal, AVMediaTypeVideo,
};
use objc2_foundation::NSArray;

use super::{CameraBackend, CameraDevice};

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
                })
            })
            .collect();

        drop(session);
        Ok(cameras)
    }
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
