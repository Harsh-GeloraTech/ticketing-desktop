// Enumerate printers installed on the host OS.
//
//   Linux/macOS: CUPS (`lpstat -e` lists queue names).
//   Windows:     native EnumPrinters (NO process is spawned, so no console
//                window ever flashes). One call returns the name, live status,
//                and driver/port info we use to classify the printer.
//
// On Windows we return richer rows (status + kind already resolved) so callers
// don't have to spawn a second process per printer. On Unix we still return
// names only and resolve status separately in `status.rs`.

use crate::printer::error::PrinterError;
use crate::printer::model::PrinterStatus;

/// A printer discovered from the OS, with whatever the platform could resolve
/// cheaply in one shot. `status`/`kind` are `None` on platforms (Unix) where we
/// resolve them lazily; `Some` on Windows where EnumPrinters gives them for free.
pub struct Discovered {
    pub name: String,
    pub status: Option<PrinterStatus>,
    pub kind: Option<String>,
}

/// Full discovery: name + (where cheap) status and kind, in one OS call.
pub async fn discover() -> Result<Vec<Discovered>, PrinterError> {
    #[cfg(unix)]
    {
        // Unix: names only; status.rs resolves the rest per-printer.
        Ok(cups_list()
            .await?
            .into_iter()
            .map(|name| Discovered {
                name,
                status: None,
                kind: None,
            })
            .collect())
    }
    #[cfg(windows)]
    {
        // Native enumeration runs on a blocking thread (the Win32 calls block).
        tokio::task::spawn_blocking(windows_enum)
            .await
            .map_err(|e| PrinterError::Discovery(format!("enum task panicked: {e}")))?
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

/// Native Windows enumeration via EnumPrintersW (level 2). No subprocess.
///
/// Numeric Win32 constants are used directly: their values are fixed by the
/// Windows ABI (they never change across OS or windows-crate versions), which
/// also sidesteps per-version feature-gating of the named constants.
#[cfg(windows)]
fn windows_enum() -> Result<Vec<Discovered>, PrinterError> {
    use windows::Win32::Graphics::Printing::{EnumPrintersW, PRINTER_INFO_2W};

    // Flags: enumerate locally-installed printers + per-user network connections.
    const PRINTER_ENUM_LOCAL: u32 = 0x0000_0002;
    const PRINTER_ENUM_CONNECTIONS: u32 = 0x0000_0004;
    const LEVEL_2: u32 = 2;

    unsafe {
        // First call: ask how many bytes the buffer needs.
        let mut needed: u32 = 0;
        let mut returned: u32 = 0;
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;

        // Probe call — expected to "fail" with ERROR_INSUFFICIENT_BUFFER and set `needed`.
        let _ = EnumPrintersW(flags, None, LEVEL_2, None, &mut needed, &mut returned);

        if needed == 0 {
            // No printers installed at all.
            return Ok(Vec::new());
        }

        // Allocate the exact buffer and enumerate for real.
        let mut buffer = vec![0u8; needed as usize];
        EnumPrintersW(
            flags,
            None,
            LEVEL_2,
            Some(buffer.as_mut_slice()),
            &mut needed,
            &mut returned,
        )
        .map_err(|e| PrinterError::Discovery(format!("EnumPrinters failed: {e}")))?;

        // The buffer is an array of PRINTER_INFO_2W structs.
        let info_ptr = buffer.as_ptr() as *const PRINTER_INFO_2W;
        let infos = std::slice::from_raw_parts(info_ptr, returned as usize);

        let mut out = Vec::with_capacity(infos.len());
        for info in infos {
            let name = pwstr_to_string(info.pPrinterName);
            if name.is_empty() {
                continue;
            }
            let driver = pwstr_to_string(info.pDriverName);
            let port = pwstr_to_string(info.pPortName);

            out.push(Discovered {
                status: Some(status_from_flags(info.Status, info.Attributes, &name)),
                kind: Some(classify(&driver, &port, &name)),
                name,
            });
        }
        Ok(out)
    }
}

/// Decode the WORK_OFFLINE attribute + PrinterStatus bitfield into our model.
#[cfg(windows)]
fn status_from_flags(status: u32, attributes: u32, _name: &str) -> PrinterStatus {
    // PRINTER_ATTRIBUTE_WORK_OFFLINE — user/driver marked the queue offline.
    const ATTR_WORK_OFFLINE: u32 = 0x0000_0400;
    // PrinterStatus bits we care about.
    const PS_PAUSED: u32 = 0x0000_0001;
    const PS_ERROR: u32 = 0x0000_0002;
    const PS_PAPER_OUT: u32 = 0x0000_0010;
    const PS_OFFLINE: u32 = 0x0000_0080;
    const PS_NOT_AVAILABLE: u32 = 0x0020_0000;
    const PS_PRINTING: u32 = 0x0000_0400;

    if attributes & ATTR_WORK_OFFLINE != 0 || status & PS_OFFLINE != 0 {
        return PrinterStatus::disconnected("Powered off or offline");
    }
    if status & PS_NOT_AVAILABLE != 0 {
        return PrinterStatus::disconnected("Not available");
    }
    if status & PS_PAPER_OUT != 0 {
        return PrinterStatus::disconnected("Out of paper");
    }
    if status & PS_ERROR != 0 {
        return PrinterStatus::disconnected("Printer error");
    }
    if status & PS_PAUSED != 0 {
        return PrinterStatus::connected("Paused");
    }
    if status & PS_PRINTING != 0 {
        return PrinterStatus::connected("Printing");
    }
    // status == 0 means "ready/idle" in the Win32 model.
    PrinterStatus::connected("Ready")
}

/// Best-effort classification so virtual printers don't all read "thermal".
/// We can't positively confirm a thermal device from metadata alone, so we
/// recognise common virtual/non-thermal drivers and label the rest "unknown".
#[cfg(windows)]
fn classify(driver: &str, port: &str, name: &str) -> String {
    let hay = format!("{} {} {}", driver, port, name).to_lowercase();
    let virtual_markers = [
        "pdf", "xps", "onenote", "fax", "microsoft print", "anydesk", "nitro",
        "cutepdf", "to file", "file:", "nul:",
    ];
    if virtual_markers.iter().any(|m| hay.contains(m)) {
        return "virtual".into();
    }
    // Heuristic: real thermal/receipt drivers and raw/USB ports are likely physical.
    let thermal_markers = ["thermal", "receipt", "pos-", "esc/pos", "tm-t", "tsp", "generic / text"];
    if thermal_markers.iter().any(|m| hay.contains(m)) {
        return "thermal".into();
    }
    "unknown".into()
}

/// Copy a null-terminated wide string out of the EnumPrinters buffer.
#[cfg(windows)]
unsafe fn pwstr_to_string(p: windows::core::PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *p.0.add(len) != 0 {
        len += 1;
    }
    let slice = std::slice::from_raw_parts(p.0, len);
    String::from_utf16_lossy(slice)
}
