mod platform;

use platform::{CameraBackend, PlatformBackend};

fn main() {
    let backend = PlatformBackend::new();

    match backend.devices() {
        Ok(devices) => {
            for device in devices {
                println!("{}", device.name);
                println!("  id: {}", device.id);
            }
        }
        Err(error) => {
            eprintln!("Failed to enumerate cameras: {error}");
        }
    }
}
