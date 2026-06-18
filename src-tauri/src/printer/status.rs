// Live connection/online status for a single printer.
//
//   Linux/macOS: `lpstat -p <name>` — text says "enabled"/"idle" (up) or
//                "disabled"/"offline"/"unable" (down). If the queue is absent
//                entirely, the device is gone (USB unplugged / removed).
//   Windows:     native EnumPrinters (via discovery.rs) — we look the name up
//                in the enumerated set and read its real status. NO PowerShell,
//                so the 3-second status poll never flashes a console window.
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

/// Windows: reuse the native enumeration and pick out this printer's row.
/// Absent from the set => the queue/device was removed.
#[cfg(windows)]
async fn windows_status(name: &str) -> PrinterStatus {
    match crate::printer::discovery::discover().await {
        Ok(list) => list
            .into_iter()
            .find(|d| d.name.eq_ignore_ascii_case(name))
            .and_then(|d| d.status)
            .unwrap_or_else(|| {
                PrinterStatus::disconnected("Not found (USB removed or printer deleted)")
            }),
        Err(e) => PrinterStatus::disconnected(format!("status check failed: {e}")),
    }
}
