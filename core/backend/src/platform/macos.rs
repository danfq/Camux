use objc2_av_foundation::{
    AVCaptureDeviceDiscoverySession, AVCaptureDevicePosition,
    AVCaptureDeviceTypeBuiltInWideAngleCamera, AVCaptureDeviceTypeContinuityCamera,
    AVCaptureDeviceTypeExternal, AVMediaTypeVideo,
};
use objc2_foundation::NSArray;

use super::{CameraBackend, CameraDevice};

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
            .map(|device| {
                let name = unsafe { device.localizedName() };
                let id = unsafe { device.uniqueID() };

                CameraDevice {
                    id: id.to_string(),
                    name: name.to_string(),
                }
            })
            .collect();

        drop(session);
        Ok(cameras)
    }
}
