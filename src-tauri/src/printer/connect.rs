// Explicit "connect to the printer" step.
//
// The app must establish and CONFIRM a real connection to the selected printer
// before any other action (save default / test / print) is allowed. Connecting
// does two things:
//
//   1. Confirms the printer is present and online (native enumeration).
//   2. Proves the device is genuinely reachable by opening a real OS printer
//      handle (Windows: OpenPrinterW; Unix: CUPS reports the queue enabled).
//
// On success we return a stable `printer_id` (the system printer name, which is
// the handle's identity) that the caller stores. If the printer is offline,
// powered off, or unreachable, `connected` is false and the UI blocks further
// actions.

use crate::printer::status;
use serde::{Deserialize, Serialize};

/// Result of an explicit connect attempt, surfaced to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectResult {
    /// True only when a real connection was established AND confirmed.
    pub connected: bool,
    /// Stable identity of the connected printer (the system name). `None` on failure.
    pub printer_id: Option<String>,
    /// Human-readable detail ("Connected", "Powered off or offline", …).
    pub detail: String,
}

impl ConnectResult {
    fn ok(id: &str, detail: impl Into<String>) -> Self {
        Self {
            connected: true,
            printer_id: Some(id.to_string()),
            detail: detail.into(),
        }
    }
    fn fail(detail: impl Into<String>) -> Self {
        Self {
            connected: false,
            printer_id: None,
            detail: detail.into(),
        }
    }
}

/// Establish and confirm a connection to `name`. Never errors — an unreachable
/// printer is a `connected: false` result, not a failure.
pub async fn connect(name: &str) -> ConnectResult {
    // 1. Is it even present + online according to the OS?
    let st = status::status_of(name).await;
    if !st.connected {
        return ConnectResult::fail(st.detail);
    }

    // 2. Prove reachability by opening a real handle to the device.
    match open_handle_check(name).await {
        Ok(()) => ConnectResult::ok(name, "Connected"),
        Err(detail) => ConnectResult::fail(detail),
    }
}

/// Open (and immediately close) a real OS handle to confirm the device is
/// reachable beyond merely being listed. Returns Err(detail) if it can't.
#[cfg(windows)]
async fn open_handle_check(name: &str) -> Result<(), String> {
    let name = name.to_string();
    tokio::task::spawn_blocking(move || win_open_check(&name))
        .await
        .map_err(|e| format!("connect task panicked: {e}"))?
}

#[cfg(windows)]
fn win_open_check(name: &str) -> Result<(), String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Graphics::Printing::{ClosePrinter, OpenPrinterW, PRINTER_DEFAULTSW};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let mut printer_name = wide(name);
    unsafe {
        let mut handle = HANDLE::default();
        let defaults = PRINTER_DEFAULTSW::default();
        OpenPrinterW(
            PCWSTR(printer_name.as_mut_ptr()),
            &mut handle,
            Some(&defaults),
        )
        .map_err(|_| "Printer is not reachable (powered off or disconnected)".to_string())?;

        // We only needed to prove the handle opens. Release it immediately.
        let _ = ClosePrinter(handle);
        Ok(())
    }
}

/// Unix: the status check (CUPS `lpstat -p`) already confirms the queue is
/// enabled/reachable, so a passing status is our connection proof.
#[cfg(unix)]
async fn open_handle_check(_name: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(any(unix, windows)))]
async fn open_handle_check(_name: &str) -> Result<(), String> {
    Err("Connecting is unsupported on this platform".into())
}
