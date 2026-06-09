// Live connection/online status for a single printer.
//
//   Linux/macOS: `lpstat -p <name>` — text says "enabled"/"idle" (up) or
//                "disabled"/"offline"/"unable" (down). If the queue is absent
//                entirely, the device is gone (USB unplugged / removed).
//   Windows:     PowerShell `Get-Printer <name>` — PrinterStatus and
//                WorkOffline tell us whether it's reachable.
//
// The result drives the 🟢/🔴 badge and the pre-print gate.

use crate::printer::model::PrinterStatus;

/// Resolve the status of `name`. Never errors — an unreachable printer is a
/// `disconnected` status, not a failure.
pub async fn status_of(name: &str) -> PrinterStatus {
    #[cfg(unix)]
    {
        cups_status(name).await
    }
    #[cfg(windows)]
    {
        windows_status(name).await
    }
    #[cfg(not(any(unix, windows)))]
    {
        PrinterStatus::disconnected("Unsupported platform")
    }
}

#[cfg(unix)]
async fn cups_status(name: &str) -> PrinterStatus {
    use tokio::process::Command;

    let out = match Command::new("lpstat").args(["-p", name]).output().await {
        Ok(o) => o,
        Err(e) => return PrinterStatus::disconnected(format!("lpstat error: {e}")),
    };

    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .to_lowercase();

    // No mention of the printer at all => the queue/device is gone.
    if text.trim().is_empty() || text.contains("unknown") || text.contains("no destinations") {
        return PrinterStatus::disconnected("Not found (USB removed or printer deleted)");
    }

    // CUPS marks a queue "disabled" when the backend can't reach the device
    // (powered off / unplugged), and includes words like "offline"/"unable".
    if text.contains("disabled") || text.contains("offline") || text.contains("unable") {
        return PrinterStatus::disconnected("Powered off or offline");
    }
    if text.contains("idle") {
        return PrinterStatus::connected("Idle");
    }
    if text.contains("printing") {
        return PrinterStatus::connected("Printing");
    }
    if text.contains("enabled") {
        return PrinterStatus::connected("Ready");
    }

    // Present but unrecognized wording — treat as connected but note it.
    PrinterStatus::connected("Available")
}

#[cfg(windows)]
async fn windows_status(name: &str) -> PrinterStatus {
    use tokio::process::Command;

    // Emit "<PrinterStatus>|<WorkOffline>" so we can parse without JSON deps.
    let script = format!(
        "$p = Get-Printer -Name '{}' -ErrorAction SilentlyContinue; \
         if ($null -eq $p) {{ 'MISSING' }} else {{ \"$($p.PrinterStatus)|$($p.WorkOffline)\" }}",
        name.replace('\'', "''")
    );

    let out = match Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) => return PrinterStatus::disconnected(format!("Get-Printer error: {e}")),
    };

    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();

    if line.is_empty() || line.eq_ignore_ascii_case("MISSING") {
        return PrinterStatus::disconnected("Not found (USB removed or printer deleted)");
    }

    let mut parts = line.splitn(2, '|');
    let pstatus = parts.next().unwrap_or("").to_lowercase();
    let offline = parts.next().unwrap_or("").to_lowercase();

    if offline == "true" || pstatus.contains("offline") {
        return PrinterStatus::disconnected("Powered off or offline");
    }
    if pstatus.contains("error") || pstatus.contains("paperout") {
        return PrinterStatus::disconnected("Printer error / out of paper");
    }
    PrinterStatus::connected("Ready")
}
