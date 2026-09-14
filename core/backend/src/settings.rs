use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

const SETTINGS_FILE_NAME: &str = "settings.json";
const LOG_FILE_NAME: &str = "camux.log";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Appearance {
    Dark,
    Light,
    #[default]
    System,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PreferredQuality {
    #[default]
    Automatic,
    Hd,
    FullHd,
    UltraHd,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DisconnectBehavior {
    #[default]
    BlackFrame,
    FreezeLastFrame,
    TestPattern,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HardwareAcceleration {
    #[default]
    Automatic,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub appearance: Appearance,
    pub launch_at_login: bool,
    pub keep_running_when_closed: bool,
    pub restore_previous_session: bool,
    pub default_camera: Option<String>,
    pub preferred_quality: PreferredQuality,
    pub preferred_frame_rate: Option<f64>,
    pub disconnect_behavior: DisconnectBehavior,
    pub virtual_camera_name: String,
    pub hardware_acceleration: HardwareAcceleration,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            appearance: Appearance::System,
            launch_at_login: false,
            keep_running_when_closed: true,
            restore_previous_session: true,
            default_camera: None,
            preferred_quality: PreferredQuality::Automatic,
            preferred_frame_rate: None,
            disconnect_behavior: DisconnectBehavior::BlackFrame,
            virtual_camera_name: "Camux Camera".to_owned(),
            hardware_acceleration: HardwareAcceleration::Automatic,
        }
    }
}

impl AppSettings {
    fn validate(&mut self) -> Result<(), String> {
        self.virtual_camera_name = self.virtual_camera_name.trim().to_owned();

        if self.virtual_camera_name.is_empty() {
            return Err("Virtual camera name cannot be empty".to_owned());
        }
        if self.virtual_camera_name.chars().count() > 64 {
            return Err("Virtual camera name cannot be longer than 64 characters".to_owned());
        }
        if self.virtual_camera_name.chars().any(char::is_control) {
            return Err("Virtual camera name cannot contain control characters".to_owned());
        }
        if self.default_camera.as_ref().is_some_and(String::is_empty) {
            self.default_camera = None;
        }
        if self.preferred_frame_rate.is_some_and(|frame_rate| {
            !frame_rate.is_finite() || !(1.0..=240.0).contains(&frame_rate)
        }) {
            return Err("Preferred frame rate must be between 1 and 240 fps".to_owned());
        }

        Ok(())
    }
}

pub struct SettingsState {
    path: PathBuf,
    settings: Mutex<AppSettings>,
}

impl SettingsState {
    pub fn load(app: &AppHandle) -> Result<Self, String> {
        let path = app
            .path()
            .app_config_dir()
            .map_err(|error| error.to_string())?
            .join(SETTINGS_FILE_NAME);

        let mut settings = match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<AppSettings>(&contents) {
                Ok(settings) => settings,
                Err(error) => {
                    let backup = path.with_extension("invalid.json");
                    let _ = fs::rename(&path, &backup);
                    append_log(
                        app,
                        &format!("Invalid settings were reset and backed up: {error}"),
                    );
                    AppSettings::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => AppSettings::default(),
            Err(error) => return Err(format!("Could not read settings: {error}")),
        };
        settings.validate()?;
        settings.launch_at_login = launch_at_login_path(app)?.exists();

        Ok(Self {
            path,
            settings: Mutex::new(settings),
        })
    }

    pub fn snapshot(&self) -> Result<AppSettings, String> {
        self.settings
            .lock()
            .map(|settings| settings.clone())
            .map_err(|_| "Settings lock was poisoned".to_owned())
    }

    pub fn replace(&self, app: &AppHandle, mut next: AppSettings) -> Result<AppSettings, String> {
        next.validate()?;

        let mut current = self
            .settings
            .lock()
            .map_err(|_| "Settings lock was poisoned".to_owned())?;
        let previous = current.clone();

        if next.launch_at_login != previous.launch_at_login {
            configure_launch_at_login(app, next.launch_at_login)?;
        }

        if let Err(error) = write_settings(&self.path, &next) {
            if next.launch_at_login != previous.launch_at_login {
                let _ = configure_launch_at_login(app, previous.launch_at_login);
            }
            return Err(error);
        }

        *current = next.clone();
        append_log(app, "Settings updated");
        Ok(next)
    }
}

fn write_settings(path: &Path, settings: &AppSettings) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Settings path has no parent directory".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create settings directory: {error}"))?;

    let temporary = path.with_extension("json.tmp");
    let contents = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Could not serialize settings: {error}"))?;
    fs::write(&temporary, format!("{contents}\n"))
        .map_err(|error| format!("Could not write settings: {error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("Could not save settings: {error}"))
}

fn append_log(app: &AppHandle, message: &str) {
    use std::io::Write;

    let Ok(directory) = app.path().app_log_dir() else {
        return;
    };
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join(LOG_FILE_NAME))
    else {
        return;
    };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let _ = writeln!(file, "[{timestamp}] {message}");
}

fn launch_at_login_path(app: &AppHandle) -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        return app
            .path()
            .home_dir()
            .map(|home| home.join("Library/LaunchAgents/dev.danfq.camux.plist"))
            .map_err(|error| error.to_string());
    }

    #[cfg(target_os = "linux")]
    {
        return app
            .path()
            .config_dir()
            .map(|config| config.join("autostart/dev.danfq.camux.desktop"))
            .map_err(|error| error.to_string());
    }

    #[allow(unreachable_code)]
    Err("Launch at login is unsupported on this platform".to_owned())
}

fn configure_launch_at_login(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let entry = launch_at_login_path(app)?;

    if !enabled {
        return match fs::remove_file(entry) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Could not disable launch at login: {error}")),
        };
    }

    let executable = std::env::current_exe()
        .map_err(|error| format!("Could not locate the Camux executable: {error}"))?;
    let parent = entry
        .parent()
        .ok_or_else(|| "Launch-at-login path has no parent directory".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create launch-at-login directory: {error}"))?;

    #[cfg(target_os = "macos")]
    let contents = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>dev.danfq.camux</string><key>ProgramArguments</key><array><string>{}</string></array><key>RunAtLoad</key><true/></dict></plist>\n",
        escape_xml(&executable.to_string_lossy())
    );

    #[cfg(target_os = "linux")]
    let contents = format!(
        "[Desktop Entry]\nType=Application\nName=Camux\nComment=Webcam splitter\nExec={}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        quote_desktop_exec(&executable.to_string_lossy())
    );

    fs::write(entry, contents).map_err(|error| format!("Could not enable launch at login: {error}"))
}

#[cfg(target_os = "macos")]
fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(target_os = "linux")]
fn quote_desktop_exec(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualCameraStatus {
    state: VirtualCameraState,
    detail: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum VirtualCameraState {
    Ready,
    #[cfg(target_os = "linux")]
    NeedsRepair,
    NotInstalled,
}

pub fn virtual_camera_status(_app: &AppHandle, name: &str) -> Result<VirtualCameraStatus, String> {
    #[cfg(target_os = "macos")]
    {
        let home = _app.path().home_dir().map_err(|error| error.to_string())?;
        let candidates = [
            PathBuf::from("/Library/CoreMediaIO/Plug-Ins/DAL/CamuxCamera.plugin"),
            home.join("Library/CoreMediaIO/Plug-Ins/DAL/CamuxCamera.plugin"),
        ];
        if candidates.iter().any(|path| path.is_dir()) {
            return Ok(VirtualCameraStatus {
                state: VirtualCameraState::Ready,
                detail: format!("{name} is available to other apps"),
            });
        }

        return Ok(VirtualCameraStatus {
            state: VirtualCameraState::NotInstalled,
            detail: "The Camux virtual camera is not installed".to_owned(),
        });
    }

    #[cfg(target_os = "linux")]
    {
        let video_root = Path::new("/sys/class/video4linux");
        if let Ok(entries) = fs::read_dir(video_root) {
            let expected = name.to_lowercase();
            let found = entries.filter_map(Result::ok).any(|entry| {
                fs::read_to_string(entry.path().join("name")).is_ok_and(|device_name| {
                    let device_name = device_name.trim().to_lowercase();
                    device_name == expected || device_name.contains("camux")
                })
            });
            if found {
                let driver_version = fs::read_to_string("/sys/module/camux_v4l2loopback/version")
                    .unwrap_or_default();
                if driver_version.trim() != "0.15.4-camux1" {
                    return Ok(VirtualCameraStatus {
                        state: VirtualCameraState::NeedsRepair,
                        detail: format!(
                            "{name} uses a driver that cannot be shared between apps; repair it to install Camux multi-reader support"
                        ),
                    });
                }
                return Ok(VirtualCameraStatus {
                    state: VirtualCameraState::Ready,
                    detail: format!("{name} is available to multiple apps at the same time"),
                });
            }
        }

        let module_loaded = Path::new("/sys/module/v4l2loopback").exists()
            || Path::new("/sys/module/camux_v4l2loopback").exists();
        return Ok(VirtualCameraStatus {
            state: if module_loaded {
                VirtualCameraState::NeedsRepair
            } else {
                VirtualCameraState::NotInstalled
            },
            detail: if module_loaded {
                "v4l2loopback is loaded, but the Camux camera is unavailable".to_owned()
            } else {
                "v4l2loopback is not installed or loaded".to_owned()
            },
        });
    }

    #[allow(unreachable_code, unused_variables)]
    Ok(VirtualCameraStatus {
        state: VirtualCameraState::NotInstalled,
        detail: "Virtual cameras are unsupported on this platform".to_owned(),
    })
}

pub fn repair_virtual_camera(app: &AppHandle, name: &str) -> Result<VirtualCameraStatus, String> {
    #[cfg(target_os = "macos")]
    {
        let resources = app
            .path()
            .resource_dir()
            .map_err(|error| error.to_string())?;
        let source = [
            resources.join("virtual-camera/CamuxCamera.plugin"),
            resources.join("CamuxCamera.plugin"),
        ]
        .into_iter()
        .find(|path| path.is_dir())
        .ok_or_else(|| "CamuxCamera.plugin is not bundled with this build".to_owned())?;
        let destination = app
            .path()
            .home_dir()
            .map_err(|error| error.to_string())?
            .join("Library/CoreMediaIO/Plug-Ins/DAL/CamuxCamera.plugin");

        if destination.exists() {
            fs::remove_dir_all(&destination)
                .map_err(|error| format!("Could not remove the old virtual camera: {error}"))?;
        }
        copy_directory(&source, &destination)?;
        append_log(app, "Virtual camera repaired");
        return virtual_camera_status(app, name);
    }

    #[cfg(target_os = "linux")]
    {
        let pkexec = ["/usr/bin/pkexec", "/bin/pkexec"]
            .into_iter()
            .find(|candidate| Path::new(candidate).is_file())
            .ok_or_else(|| {
                "PolicyKit is unavailable; install polkit or load v4l2loopback as an administrator"
                    .to_owned()
            })?;
        let source = bundled_v4l2loopback_source(app)?;
        let installer = source.join("install.sh");
        let output = Command::new(pkexec)
            .arg("/bin/sh")
            .arg(installer)
            .arg(&source)
            .arg(name)
            .output()
            .map_err(|error| format!("Could not request administrator access: {error}"))?;
        if !output.status.success() {
            let reason = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(if reason.is_empty() {
                "Could not install the Camux multi-reader driver; administrator access may be required".to_owned()
            } else {
                format!("Could not install the Camux multi-reader driver: {reason}")
            });
        }
        append_log(
            app,
            "Camux multi-reader virtual camera installed and loaded",
        );
        return virtual_camera_status(app, name);
    }

    #[allow(unreachable_code, unused_variables)]
    Err("Virtual cameras are unsupported on this platform".to_owned())
}

#[cfg(target_os = "linux")]
fn bundled_v4l2loopback_source(app: &AppHandle) -> Result<PathBuf, String> {
    let resources = app
        .path()
        .resource_dir()
        .map_err(|error| error.to_string())?;
    let mut candidates = vec![resources.join("v4l2loopback")];

    #[cfg(debug_assertions)]
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/v4l2loopback"));

    candidates
        .into_iter()
        .find(|path| path.join("install.sh").is_file() && path.join("v4l2loopback.c").is_file())
        .ok_or_else(|| "The Camux multi-reader driver is not bundled with this build".to_owned())
}

#[cfg(target_os = "macos")]
fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("Could not create virtual camera directory: {error}"))?;
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

pub fn open_logs(app: &AppHandle) -> Result<(), String> {
    let directory = app
        .path()
        .app_log_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create log directory: {error}"))?;
    append_log(app, "Logs opened");

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(&directory);
        command
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(&directory);
        command
    };

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return Err("Opening logs is unsupported on this platform".to_owned());

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open logs: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_virtual_camera_names() {
        let mut settings = AppSettings {
            virtual_camera_name: "  ".to_owned(),
            ..AppSettings::default()
        };
        assert!(settings.validate().is_err());

        settings.virtual_camera_name = "Camux\nCamera".to_owned();
        assert!(settings.validate().is_err());
    }

    #[test]
    fn trims_valid_virtual_camera_names() {
        let mut settings = AppSettings {
            virtual_camera_name: "  Studio Camera  ".to_owned(),
            ..AppSettings::default()
        };
        settings.validate().unwrap();
        assert_eq!(settings.virtual_camera_name, "Studio Camera");
    }

    #[test]
    fn serializes_the_frontend_settings_contract() {
        let value = serde_json::to_value(AppSettings::default()).unwrap();
        assert_eq!(value["appearance"], "system");
        assert_eq!(value["keepRunningWhenClosed"], true);
        assert_eq!(value["preferredQuality"], "automatic");
        assert_eq!(value["preferredFrameRate"], serde_json::Value::Null);
        assert_eq!(value["disconnectBehavior"], "blackFrame");
        assert_eq!(value["hardwareAcceleration"], "automatic");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn escapes_launch_agent_values() {
        assert_eq!(escape_xml("A&B <C>"), "A&amp;B &lt;C&gt;");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn quotes_desktop_entry_executables() {
        assert_eq!(quote_desktop_exec("/tmp/Camux App"), "\"/tmp/Camux App\"");
    }
}
