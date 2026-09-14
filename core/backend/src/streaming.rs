use crate::settings::AppSettings;
use serde::Serialize;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraStreamInfo {
    pub session_id: u64,
    pub device_id: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub pixel_format: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraRouteStatus {
    pub device_id: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub detail: String,
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use crate::settings::PreferredQuality;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;
    use v4l::buffer::Type;
    use v4l::format::FourCC;
    use v4l::frameinterval::FrameIntervalEnum;
    use v4l::framesize::FrameSizeEnum;
    use v4l::io::mmap::Stream as MmapStream;
    use v4l::io::traits::{CaptureStream, OutputStream};
    use v4l::video::{Capture, Output};
    use v4l::{Device, Format, Fraction};

    const CONTROL_TIMEOUT: Duration = Duration::from_secs(3);
    const FRAME_TIMEOUT: Duration = Duration::from_millis(750);
    const MJPEG: FourCC = FourCC { repr: *b"MJPG" };

    struct SessionHandle {
        info: CameraStreamInfo,
        control: Sender<StreamControl>,
        worker: Option<JoinHandle<()>>,
        preview_active: Arc<AtomicBool>,
        frames: Arc<FrameMailbox>,
        preview_attached: bool,
        routing: bool,
    }

    #[derive(Default)]
    struct FrameSlot {
        sequence: u64,
        bytes: Vec<u8>,
        error: Option<String>,
    }

    #[derive(Default)]
    struct FrameMailbox {
        slot: Mutex<FrameSlot>,
        changed: Condvar,
        delivered_sequence: AtomicU64,
    }

    impl FrameMailbox {
        fn publish(&self, frame: &[u8]) {
            if let Ok(mut slot) = self.slot.lock() {
                slot.sequence = slot.sequence.wrapping_add(1).max(1);
                slot.bytes.clear();
                slot.bytes.extend_from_slice(frame);
                self.changed.notify_one();
            }
        }

        fn fail(&self, error: String) {
            if let Ok(mut slot) = self.slot.lock() {
                slot.error = Some(error);
                self.changed.notify_all();
            }
        }

        fn reset_reader(&self) {
            self.delivered_sequence.store(0, Ordering::Release);
            if let Ok(mut slot) = self.slot.lock() {
                slot.error = None;
            }
        }

        fn next(&self) -> Result<Vec<u8>, String> {
            let last_sequence = self.delivered_sequence.load(Ordering::Acquire);
            let slot = self
                .slot
                .lock()
                .map_err(|_| "Camera frame lock was poisoned".to_owned())?;
            let (slot, timeout) = self
                .changed
                .wait_timeout_while(slot, CONTROL_TIMEOUT, |slot| {
                    slot.sequence <= last_sequence && slot.error.is_none()
                })
                .map_err(|_| "Camera frame lock was poisoned".to_owned())?;
            if let Some(error) = &slot.error {
                return Err(error.clone());
            }
            if timeout.timed_out() || slot.sequence <= last_sequence {
                return Err("The camera did not produce another frame within 3 seconds".to_owned());
            }
            self.delivered_sequence
                .store(slot.sequence, Ordering::Release);
            Ok(slot.bytes.clone())
        }
    }

    impl SessionHandle {
        fn stop(mut self) {
            let _ = self.control.send(StreamControl::Stop);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }

    #[derive(Default)]
    struct StreamRegistry {
        sessions: HashMap<u64, SessionHandle>,
        active_route: Option<u64>,
    }

    pub struct CameraStreamState {
        next_id: AtomicU64,
        registry: Mutex<StreamRegistry>,
    }

    impl Default for CameraStreamState {
        fn default() -> Self {
            Self {
                next_id: AtomicU64::new(1),
                registry: Mutex::new(StreamRegistry::default()),
            }
        }
    }

    impl Drop for CameraStreamState {
        fn drop(&mut self) {
            if let Ok(registry) = self.registry.get_mut() {
                for (_, session) in registry.sessions.drain() {
                    session.stop();
                }
            }
        }
    }

    enum StreamControl {
        StartVirtual {
            camera_name: String,
            reply: SyncSender<Result<(), String>>,
        },
        StopVirtual {
            reply: SyncSender<()>,
        },
        Stop,
    }

    struct OutputHandle {
        frames: SyncSender<Vec<u8>>,
        worker: JoinHandle<()>,
    }

    impl OutputHandle {
        fn stop(self) {
            drop(self.frames);
            let _ = self.worker.join();
        }
    }

    #[derive(Debug, Clone)]
    struct CaptureMode {
        width: u32,
        height: u32,
        rates: Vec<(f64, f64)>,
    }

    impl CameraStreamState {
        pub fn start_preview(
            &self,
            device_id: String,
            settings: AppSettings,
        ) -> Result<CameraStreamInfo, String> {
            if let Some(existing) = self.existing_routed_session(&device_id) {
                existing.preview_active.store(true, Ordering::Release);
                existing.frames.reset_reader();
                let mut registry = self
                    .registry
                    .lock()
                    .map_err(|_| "Camera stream lock was poisoned".to_owned())?;
                if let Some(session) = registry.sessions.get_mut(&existing.info.session_id) {
                    session.preview_attached = true;
                }
                return Ok(existing.info);
            }

            let session_id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let (control_tx, control_rx) = mpsc::channel();
            let (ready_tx, ready_rx) = mpsc::sync_channel(1);
            let preview_active = Arc::new(AtomicBool::new(true));
            let frames = Arc::new(FrameMailbox::default());
            let thread_device_id = device_id.clone();
            let thread_preview_active = Arc::clone(&preview_active);
            let thread_frames = Arc::clone(&frames);
            let worker = thread::Builder::new()
                .name(format!("camux-capture-{session_id}"))
                .spawn(move || {
                    capture_camera(
                        session_id,
                        thread_device_id,
                        settings,
                        thread_preview_active,
                        thread_frames,
                        control_rx,
                        ready_tx,
                    )
                })
                .map_err(|error| format!("Could not start the camera worker: {error}"))?;

            let info = ready_rx
                .recv_timeout(CONTROL_TIMEOUT)
                .map_err(|_| "The camera did not produce a frame within 3 seconds".to_owned())??;
            let mut registry = self
                .registry
                .lock()
                .map_err(|_| "Camera stream lock was poisoned".to_owned())?;
            registry.sessions.insert(
                session_id,
                SessionHandle {
                    info: info.clone(),
                    control: control_tx,
                    worker: Some(worker),
                    preview_active,
                    frames,
                    preview_attached: true,
                    routing: false,
                },
            );
            Ok(info)
        }

        pub async fn next_frame(&self, session_id: u64) -> Result<Vec<u8>, String> {
            let frames = {
                let registry = self
                    .registry
                    .lock()
                    .map_err(|_| "Camera stream lock was poisoned".to_owned())?;
                let session = registry
                    .sessions
                    .get(&session_id)
                    .ok_or_else(|| "The camera preview is no longer running".to_owned())?;
                if !session.preview_attached {
                    return Err("The camera preview has been closed".to_owned());
                }
                Arc::clone(&session.frames)
            };
            tauri::async_runtime::spawn_blocking(move || frames.next())
                .await
                .map_err(|error| format!("Camera frame task failed: {error}"))?
        }

        pub fn detach_preview(&self, session_id: u64) -> Result<(), String> {
            let mut registry = self
                .registry
                .lock()
                .map_err(|_| "Camera stream lock was poisoned".to_owned())?;
            let Some(session) = registry.sessions.get_mut(&session_id) else {
                return Ok(());
            };
            session.preview_attached = false;
            session.preview_active.store(false, Ordering::Release);
            session
                .frames
                .fail("The camera preview has been closed".to_owned());
            if !session.routing {
                let session = registry
                    .sessions
                    .remove(&session_id)
                    .expect("session exists");
                session.stop();
            }
            Ok(())
        }

        pub fn route_to_virtual(
            &self,
            session_id: u64,
            camera_name: String,
        ) -> Result<CameraRouteStatus, String> {
            let (target, previous) = {
                let registry = self
                    .registry
                    .lock()
                    .map_err(|_| "Camera stream lock was poisoned".to_owned())?;
                let target = registry
                    .sessions
                    .get(&session_id)
                    .ok_or_else(|| "The camera preview is no longer running".to_owned())?;
                if target.routing {
                    return Ok(route_status(&target.info));
                }
                let previous = registry.active_route.and_then(|active_id| {
                    registry
                        .sessions
                        .get(&active_id)
                        .map(|session| (active_id, session.control.clone()))
                });
                (target.control.clone(), previous)
            };

            if let Some((_, control)) = &previous {
                let (reply_tx, reply_rx) = mpsc::sync_channel(1);
                let _ = control.send(StreamControl::StopVirtual { reply: reply_tx });
                let _ = reply_rx.recv_timeout(CONTROL_TIMEOUT);
            }

            let (reply_tx, reply_rx) = mpsc::sync_channel(1);
            target
                .send(StreamControl::StartVirtual {
                    camera_name,
                    reply: reply_tx,
                })
                .map_err(|_| "The camera stream stopped unexpectedly".to_owned())?;
            reply_rx.recv_timeout(CONTROL_TIMEOUT).map_err(|_| {
                "The virtual camera did not become ready within 3 seconds".to_owned()
            })??;

            let mut registry = self
                .registry
                .lock()
                .map_err(|_| "Camera stream lock was poisoned".to_owned())?;
            if let Some((previous_id, _)) = previous
                && let Some(previous_session) = registry.sessions.get_mut(&previous_id)
            {
                previous_session.routing = false;
                if !previous_session.preview_attached {
                    let previous_session = registry
                        .sessions
                        .remove(&previous_id)
                        .expect("previous session exists");
                    previous_session.stop();
                }
            }
            let target = registry
                .sessions
                .get_mut(&session_id)
                .ok_or_else(|| "The camera stream stopped unexpectedly".to_owned())?;
            target.routing = true;
            let status = route_status(&target.info);
            registry.active_route = Some(session_id);
            Ok(status)
        }

        pub fn stop_virtual(&self) -> Result<(), String> {
            let active = {
                let registry = self
                    .registry
                    .lock()
                    .map_err(|_| "Camera stream lock was poisoned".to_owned())?;
                registry.active_route.and_then(|session_id| {
                    registry
                        .sessions
                        .get(&session_id)
                        .map(|session| (session_id, session.control.clone()))
                })
            };
            let Some((session_id, control)) = active else {
                return Ok(());
            };

            let (reply_tx, reply_rx) = mpsc::sync_channel(1);
            control
                .send(StreamControl::StopVirtual { reply: reply_tx })
                .map_err(|_| "The camera stream stopped unexpectedly".to_owned())?;
            reply_rx
                .recv_timeout(CONTROL_TIMEOUT)
                .map_err(|_| "The virtual camera did not stop within 3 seconds".to_owned())?;

            let removed = {
                let mut registry = self
                    .registry
                    .lock()
                    .map_err(|_| "Camera stream lock was poisoned".to_owned())?;
                registry.active_route = None;
                let remove = registry
                    .sessions
                    .get_mut(&session_id)
                    .is_some_and(|session| {
                        session.routing = false;
                        !session.preview_attached
                    });
                remove
                    .then(|| registry.sessions.remove(&session_id))
                    .flatten()
            };
            if let Some(session) = removed {
                session.stop();
            }
            Ok(())
        }

        fn existing_routed_session(&self, device_id: &str) -> Option<ExistingSession> {
            let registry = self.registry.lock().ok()?;
            let session_id = registry.active_route?;
            let session = registry.sessions.get(&session_id)?;
            (session.info.device_id == device_id).then(|| ExistingSession {
                info: session.info.clone(),
                preview_active: Arc::clone(&session.preview_active),
                frames: Arc::clone(&session.frames),
            })
        }
    }

    struct ExistingSession {
        info: CameraStreamInfo,
        preview_active: Arc<AtomicBool>,
        frames: Arc<FrameMailbox>,
    }

    fn route_status(info: &CameraStreamInfo) -> CameraRouteStatus {
        CameraRouteStatus {
            device_id: info.device_id.clone(),
            width: info.width,
            height: info.height,
            frame_rate: info.frame_rate,
            detail: format!(
                "Streaming {}×{} at {:.0} fps to the Camux virtual camera",
                info.width, info.height, info.frame_rate
            ),
        }
    }

    fn capture_camera(
        session_id: u64,
        device_id: String,
        settings: AppSettings,
        preview_active: Arc<AtomicBool>,
        frames: Arc<FrameMailbox>,
        control_rx: Receiver<StreamControl>,
        ready: SyncSender<Result<CameraStreamInfo, String>>,
    ) {
        let result = run_capture(
            session_id,
            &device_id,
            &settings,
            &preview_active,
            &frames,
            control_rx,
            &ready,
        );
        if let Err(error) = result {
            frames.fail(error.clone());
            let _ = ready.send(Err(error));
        }
    }

    fn run_capture(
        session_id: u64,
        device_id: &str,
        settings: &AppSettings,
        preview_active: &AtomicBool,
        frames: &FrameMailbox,
        control_rx: Receiver<StreamControl>,
        ready: &SyncSender<Result<CameraStreamInfo, String>>,
    ) -> Result<(), String> {
        let device = Device::with_path(device_id)
            .map_err(|error| format!("Could not open {device_id}: {error}"))?;
        let mode = choose_capture_mode(&device, settings)?;
        let requested = Format::new(mode.width, mode.height, MJPEG);
        let actual = Capture::set_format(&device, &requested).map_err(|error| {
            format!(
                "Could not select {}×{} MJPEG: {error}",
                mode.width, mode.height
            )
        })?;
        if actual.width != mode.width || actual.height != mode.height || actual.fourcc != MJPEG {
            return Err(format!(
                "The camera selected {}×{} {}, not the requested {}×{} MJPEG",
                actual.width, actual.height, actual.fourcc, mode.width, mode.height
            ));
        }

        let requested_rate =
            selected_rate(&mode, settings.preferred_frame_rate, capture_rate(&device));
        let interval_denominator = (requested_rate * 1000.0).round().max(1.0) as u32;
        let parameters =
            v4l::video::capture::Parameters::new(Fraction::new(1000, interval_denominator));
        let actual_parameters = Capture::set_params(&device, &parameters)
            .map_err(|error| format!("Could not select {requested_rate:.0} fps: {error}"))?;
        let actual_rate = fraction_rate(actual_parameters.interval);
        if settings
            .preferred_frame_rate
            .is_some_and(|preferred| (actual_rate - preferred).abs() > 0.5)
        {
            return Err(format!(
                "The camera negotiated {actual_rate:.1} fps instead of the requested {:.1} fps",
                settings.preferred_frame_rate.unwrap_or_default()
            ));
        }

        let info = CameraStreamInfo {
            session_id,
            device_id: device_id.to_owned(),
            width: actual.width,
            height: actual.height,
            frame_rate: actual_rate,
            pixel_format: "MJPEG".to_owned(),
        };
        let mut stream = MmapStream::with_buffers(&device, Type::VideoCapture, 4)
            .map_err(|error| format!("Could not allocate camera buffers: {error}"))?;
        stream.set_timeout(FRAME_TIMEOUT);
        let (first_buffer, first_metadata) = CaptureStream::next(&mut stream).map_err(|error| {
            if error.kind() == std::io::ErrorKind::TimedOut {
                "The camera opened but did not deliver a frame within 750 ms".to_owned()
            } else {
                format!("Could not start camera capture: {error}")
            }
        })?;
        let first_frame = complete_mjpeg_frame(first_buffer, first_metadata.bytesused)?;
        frames.publish(first_frame);
        ready
            .send(Ok(info.clone()))
            .map_err(|_| "The preview was closed while the camera was starting".to_owned())?;

        let mut output: Option<OutputHandle> = None;

        loop {
            loop {
                match control_rx.try_recv() {
                    Ok(StreamControl::StartVirtual { camera_name, reply }) => {
                        if let Some(existing) = output.take() {
                            existing.stop();
                        }
                        match start_virtual_output(&camera_name, &info) {
                            Ok(handle) => {
                                output = Some(handle);
                                let _ = reply.send(Ok(()));
                            }
                            Err(error) => {
                                let _ = reply.send(Err(error));
                            }
                        }
                    }
                    Ok(StreamControl::StopVirtual { reply }) => {
                        if let Some(existing) = output.take() {
                            existing.stop();
                        }
                        let _ = reply.send(());
                    }
                    Ok(StreamControl::Stop) | Err(TryRecvError::Disconnected) => {
                        if let Some(existing) = output.take() {
                            existing.stop();
                        }
                        return Ok(());
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }

            match CaptureStream::next(&mut stream) {
                Ok((buffer, metadata)) => {
                    if let Ok(frame) = complete_mjpeg_frame(buffer, metadata.bytesused) {
                        dispatch_frame(frame, preview_active, frames, output.as_ref());
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::TimedOut => continue,
                Err(error) => return Err(format!("Camera capture stopped: {error}")),
            }
        }
    }

    fn dispatch_frame(
        frame: &[u8],
        preview_active: &AtomicBool,
        frames: &FrameMailbox,
        output: Option<&OutputHandle>,
    ) {
        if let Some(output) = output {
            let _ = output.frames.try_send(frame.to_vec());
        }
        if preview_active.load(Ordering::Acquire) {
            frames.publish(frame);
        }
    }

    fn complete_mjpeg_frame(buffer: &[u8], bytes_used: u32) -> Result<&[u8], String> {
        let length = usize::try_from(bytes_used)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let frame = &buffer[..length];
        if frame.len() < 4 || !frame.starts_with(&[0xff, 0xd8]) {
            return Err("The camera returned an incomplete MJPEG frame".to_owned());
        }
        let end = frame
            .windows(2)
            .rposition(|marker| marker == [0xff, 0xd9])
            .map(|position| position + 2)
            .ok_or_else(|| "The camera returned an incomplete MJPEG frame".to_owned())?;
        Ok(&frame[..end])
    }

    fn start_virtual_output(
        camera_name: &str,
        info: &CameraStreamInfo,
    ) -> Result<OutputHandle, String> {
        let device_path = find_virtual_camera(camera_name)?;
        let (frame_tx, frame_rx) = mpsc::sync_channel::<Vec<u8>>(2);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let info = info.clone();
        let worker = thread::Builder::new()
            .name(format!("camux-output-{}", info.session_id))
            .spawn(move || virtual_output_worker(device_path, info, frame_rx, ready_tx))
            .map_err(|error| format!("Could not start the virtual-camera worker: {error}"))?;
        ready_rx.recv_timeout(CONTROL_TIMEOUT).map_err(|_| {
            "The virtual camera did not accept its format within 3 seconds".to_owned()
        })??;
        Ok(OutputHandle {
            frames: frame_tx,
            worker,
        })
    }

    fn virtual_output_worker(
        device_path: PathBuf,
        info: CameraStreamInfo,
        frames: Receiver<Vec<u8>>,
        ready: SyncSender<Result<(), String>>,
    ) {
        let result = (|| -> Result<(), String> {
            let device = Device::with_path(&device_path).map_err(|error| {
                format!(
                    "Could not open virtual camera {}: {error}",
                    device_path.display()
                )
            })?;
            let requested = Format::new(info.width, info.height, MJPEG);
            let actual = Output::set_format(&device, &requested)
                .map_err(|error| format!("The virtual camera rejected MJPEG output: {error}"))?;
            if actual.width != info.width || actual.height != info.height || actual.fourcc != MJPEG
            {
                return Err(format!(
                    "The virtual camera selected {}×{} {}, not {}×{} MJPEG",
                    actual.width, actual.height, actual.fourcc, info.width, info.height
                ));
            }
            let denominator = (info.frame_rate * 1000.0).round().max(1.0) as u32;
            let parameters = v4l::video::output::Parameters::new(Fraction::new(1000, denominator));
            let _ = Output::set_params(&device, &parameters);
            let mut stream = MmapStream::with_buffers(&device, Type::VideoOutput, 4)
                .map_err(|error| format!("Could not allocate virtual-camera buffers: {error}"))?;
            stream.set_timeout(FRAME_TIMEOUT);
            ready
                .send(Ok(()))
                .map_err(|_| "The camera route was cancelled".to_owned())?;

            while let Ok(frame) = frames.recv() {
                let (buffer, metadata) = OutputStream::next(&mut stream)
                    .map_err(|error| format!("Could not write a virtual-camera frame: {error}"))?;
                if frame.len() > buffer.len() {
                    return Err(format!(
                        "A {} byte camera frame exceeds the virtual camera's {} byte buffer",
                        frame.len(),
                        buffer.len()
                    ));
                }
                buffer[..frame.len()].copy_from_slice(&frame);
                metadata.bytesused = frame.len() as u32;
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = ready.send(Err(error));
        }
    }

    fn find_virtual_camera(camera_name: &str) -> Result<PathBuf, String> {
        let entries = fs::read_dir("/sys/class/video4linux")
            .map_err(|error| format!("Could not inspect virtual cameras: {error}"))?;
        let expected = camera_name.trim().to_lowercase();
        for entry in entries.filter_map(Result::ok) {
            let name = fs::read_to_string(entry.path().join("name"))
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            if name != expected && !name.contains("camux") {
                continue;
            }
            let path = Path::new("/dev").join(entry.file_name());
            if let Ok(device) = Device::with_path(&path)
                && let Ok(capabilities) = device.query_caps()
                && capabilities
                    .capabilities
                    .contains(v4l::capability::Flags::VIDEO_OUTPUT)
            {
                return Ok(path);
            }
        }
        Err(format!(
            "{camera_name} is not ready. Repair the virtual camera in Settings first."
        ))
    }

    fn choose_capture_mode(device: &Device, settings: &AppSettings) -> Result<CaptureMode, String> {
        let requested_dimensions = match settings.preferred_quality {
            PreferredQuality::Automatic => None,
            PreferredQuality::Hd => Some((1280, 720)),
            PreferredQuality::FullHd => Some((1920, 1080)),
            PreferredQuality::UltraHd => Some((3840, 2160)),
        };
        let current_format = Capture::format(device).ok();
        let formats = Capture::enum_formats(device)
            .map_err(|error| format!("Could not enumerate camera formats: {error}"))?;
        let mut modes = Vec::new();

        for format in formats.into_iter().filter(|format| format.fourcc == MJPEG) {
            let sizes = Capture::enum_framesizes(device, format.fourcc).unwrap_or_default();
            for size in sizes {
                let (width, height) = match size.size {
                    FrameSizeEnum::Discrete(size) => (size.width, size.height),
                    FrameSizeEnum::Stepwise(size) => match requested_dimensions {
                        Some((width, height))
                            if width >= size.min_width
                                && width <= size.max_width
                                && height >= size.min_height
                                && height <= size.max_height =>
                        {
                            (width, height)
                        }
                        Some(_) => continue,
                        None => (size.max_width, size.max_height),
                    },
                };
                if requested_dimensions.is_some_and(|dimensions| dimensions != (width, height)) {
                    continue;
                }
                let rates = Capture::enum_frameintervals(device, format.fourcc, width, height)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|interval| match interval.interval {
                        FrameIntervalEnum::Discrete(interval) => {
                            let rate = interval_rate(interval.numerator, interval.denominator)?;
                            Some((rate, rate))
                        }
                        FrameIntervalEnum::Stepwise(interval) => Some((
                            interval_rate(interval.max.numerator, interval.max.denominator)?,
                            interval_rate(interval.min.numerator, interval.min.denominator)?,
                        )),
                    })
                    .collect::<Vec<_>>();
                if settings
                    .preferred_frame_rate
                    .is_some_and(|rate| !supports_rate(&rates, rate))
                {
                    continue;
                }
                modes.push(CaptureMode {
                    width,
                    height,
                    rates,
                });
            }
        }

        modes.sort_by(|left, right| {
            let left_current = current_format.as_ref().is_some_and(|format| {
                format.fourcc == MJPEG && format.width == left.width && format.height == left.height
            });
            let right_current = current_format.as_ref().is_some_and(|format| {
                format.fourcc == MJPEG
                    && format.width == right.width
                    && format.height == right.height
            });
            let preserve_current =
                requested_dimensions.is_none() && settings.preferred_frame_rate.is_none();
            preserve_current
                .then_some(right_current.cmp(&left_current))
                .into_iter()
                .find(|ordering| !ordering.is_eq())
                .unwrap_or_else(|| {
                    (right.width as u64 * right.height as u64)
                        .cmp(&(left.width as u64 * left.height as u64))
                })
        });

        let Some(mode) = modes.into_iter().next() else {
            let quality = requested_dimensions
                .map(|(width, height)| format!("{width}×{height}"))
                .unwrap_or_else(|| "an automatic resolution".to_owned());
            let rate = settings
                .preferred_frame_rate
                .map(|rate| format!(" at {rate:.0} fps"))
                .unwrap_or_default();
            return Err(format!(
                "This camera does not expose a native MJPEG mode for {quality}{rate}"
            ));
        };
        Ok(mode)
    }

    fn selected_rate(mode: &CaptureMode, preferred: Option<f64>, current: Option<f64>) -> f64 {
        if let Some(preferred) = preferred {
            return preferred;
        }
        if let Some(current) = current
            && supports_rate(&mode.rates, current)
        {
            return current;
        }
        let maximum = mode
            .rates
            .iter()
            .map(|(_, maximum)| *maximum)
            .fold(30.0_f64, f64::max);
        maximum.min(60.0)
    }

    fn capture_rate(device: &Device) -> Option<f64> {
        Capture::params(device)
            .ok()
            .map(|params| fraction_rate(params.interval))
    }

    fn fraction_rate(interval: Fraction) -> f64 {
        if interval.numerator == 0 {
            0.0
        } else {
            interval.denominator as f64 / interval.numerator as f64
        }
    }

    fn interval_rate(numerator: u32, denominator: u32) -> Option<f64> {
        (numerator != 0).then(|| denominator as f64 / numerator as f64)
    }

    fn supports_rate(rates: &[(f64, f64)], requested: f64) -> bool {
        rates.is_empty()
            || rates.iter().any(|(minimum, maximum)| {
                requested >= minimum - 0.01 && requested <= maximum + 0.01
            })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn selects_preferred_or_stable_frame_rate() {
            let mode = CaptureMode {
                width: 1920,
                height: 1080,
                rates: vec![(30.0, 30.0), (60.0, 60.0)],
            };
            assert_eq!(selected_rate(&mode, Some(60.0), Some(30.0)), 60.0);
            assert_eq!(selected_rate(&mode, None, Some(30.0)), 30.0);
            assert_eq!(selected_rate(&mode, None, None), 60.0);
        }

        #[test]
        fn checks_discrete_and_range_frame_rates() {
            assert!(supports_rate(&[(60.0, 60.0)], 60.0));
            assert!(!supports_rate(&[(30.0, 30.0)], 60.0));
            assert!(supports_rate(&[(15.0, 90.0)], 60.0));
        }

        #[test]
        fn keeps_only_complete_mjpeg_data() {
            let buffer = [0xff, 0xd8, 1, 2, 0xff, 0xd9, 0, 0];
            assert_eq!(complete_mjpeg_frame(&buffer, 8).unwrap(), &buffer[..6]);
            assert!(complete_mjpeg_frame(&buffer[..5], 5).is_err());
        }

        #[test]
        fn frame_mailbox_retains_only_the_latest_frame() {
            let mailbox = FrameMailbox::default();
            mailbox.publish(&[1]);
            mailbox.publish(&[2]);
            assert_eq!(mailbox.next().unwrap(), vec![2]);
            mailbox.publish(&[3]);
            assert_eq!(mailbox.next().unwrap(), vec![3]);
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::*;

    #[derive(Default)]
    pub struct CameraStreamState;

    impl CameraStreamState {
        pub fn start_preview(
            &self,
            _device_id: String,
            _settings: AppSettings,
        ) -> Result<CameraStreamInfo, String> {
            Err("Native camera streaming is not implemented on this platform yet".to_owned())
        }

        pub async fn next_frame(&self, _session_id: u64) -> Result<Vec<u8>, String> {
            Err("Native camera streaming is not implemented on this platform yet".to_owned())
        }

        pub fn detach_preview(&self, _session_id: u64) -> Result<(), String> {
            Ok(())
        }

        pub fn route_to_virtual(
            &self,
            _session_id: u64,
            _camera_name: String,
        ) -> Result<CameraRouteStatus, String> {
            Err("Virtual-camera routing is not implemented on this platform yet".to_owned())
        }

        pub fn stop_virtual(&self) -> Result<(), String> {
            Ok(())
        }
    }
}

pub use platform::CameraStreamState;
