// Enumerate printers installed on the host OS.
//
//   Linux/macOS: CUPS (`lpstat -e` lists queue names).
//   Windows:     PowerShell `Get-Printer` (names, one per line).
//
// We only return *names* here; live status is resolved separately in `status.rs`.

use crate::printer::error::PrinterError;

/// Names of all system-known printers.
pub async fn system_printers() -> Result<Vec<String>, PrinterError> {
    #[cfg(unix)]
    {
        cups_list().await
    }
    #[cfg(windows)]
    {
        windows_list().await
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(Vec::new())
    }
}

#[cfg(unix)]
async fn cups_list() -> Result<Vec<String>, PrinterError> {
    use tokio::process::Command;

    // `lpstat -e` prints every available destination, one name per line.
    let out = Command::new("lpstat")
        .arg("-e")
        .output()
        .await
        .map_err(|e| PrinterError::Discovery(format!("failed to run lpstat: {e}")))?;

    // `lpstat -e` returns non-zero / empty when CUPS has no queues — treat as "none".
    if !out.status.success() && out.stdout.is_empty() {
        return Ok(Vec::new());
    }

    let names = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    Ok(names)
}

#[cfg(windows)]
async fn windows_list() -> Result<Vec<String>, PrinterError> {
    use tokio::process::Command;

    // -NoProfile keeps it fast; select just the Name and emit one per line.
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-Printer | Select-Object -ExpandProperty Name",
        ])
        .output()
        .await
        .map_err(|e| PrinterError::Discovery(format!("failed to run Get-Printer: {e}")))?;

    let names = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    Ok(names)
}
