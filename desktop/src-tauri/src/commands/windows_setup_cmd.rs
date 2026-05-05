#[cfg(windows)]
use std::process::Command;

#[tauri::command]
pub fn run_windows_first_run_setup() -> Result<String, String> {
    run_platform_setup()
}

#[cfg(windows)]
fn run_platform_setup() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let exe = exe.to_string_lossy().replace('\'', "''");
    let script = format!(
        r#"
$args = @(
  'advfirewall', 'firewall', 'add', 'rule',
  'name=FocusBridge Local Sync',
  'dir=in',
  'action=allow',
  'protocol=TCP',
  'localport=9173',
  'profile=private,domain',
  'program={exe}',
  'enable=yes'
)
Start-Process -FilePath 'netsh.exe' -ArgumentList $args -Verb RunAs -Wait
"#
    );
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok("Windows firewall now allows FocusBridge local sync on TCP 9173. For correct toast identity, use the packaged FocusBridge installer/build rather than launching from PowerShell.".into())
    } else {
        Err(format!("Windows setup exited with status {status}"))
    }
}

#[cfg(not(windows))]
fn run_platform_setup() -> Result<String, String> {
    Ok("No Windows firewall setup is needed on this platform.".into())
}
