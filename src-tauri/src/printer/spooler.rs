// Send a raw ESC/POS byte buffer to a named system printer.
//
//   Linux/macOS: pipe the bytes to `lp -d <name> -o raw` over stdin. CUPS
//                passes them straight through to the device.
//   Windows:     winspool raw job (OpenPrinter -> StartDocPrinter "RAW" ->
//                WritePrinter -> EndDocPrinter). No driver rendering.
//
// A timeout guards against a wedged spooler / device.

use crate::printer::error::PrinterError;
use std::time::Duration;

const PRINT_TIMEOUT: Duration = Duration::from_secs(20);

/// Send `data` to the printer named `name`. Returns Ok(()) when the spooler
/// accepted the job.
pub async fn send_raw(name: &str, data: &[u8]) -> Result<(), PrinterError> {
    let fut = send_raw_inner(name, data.to_vec());
    match tokio::time::timeout(PRINT_TIMEOUT, fut).await {
        Ok(res) => res,
        Err(_) => Err(PrinterError::Timeout),
    }
}

#[cfg(unix)]
async fn send_raw_inner(name: &str, data: Vec<u8>) -> Result<(), PrinterError> {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;
    use tokio::process::Command;

    let mut child = Command::new("lp")
        .args(["-d", name, "-o", "raw"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| PrinterError::Print(format!("failed to launch lp: {e}")))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| PrinterError::Print("could not open lp stdin".into()))?;
        stdin
            .write_all(&data)
            .await
            .map_err(|e| PrinterError::Print(format!("writing to lp failed: {e}")))?;
        // stdin drops here -> EOF, so lp finishes.
    }

    let out = child
        .wait_with_output()
        .await
        .map_err(|e| PrinterError::Print(format!("lp did not complete: {e}")))?;

    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let err = if err.is_empty() {
            "lp returned a non-zero exit code".to_string()
        } else {
            err
        };
        // Common case: device not reachable.
        if err.to_lowercase().contains("unable") || err.to_lowercase().contains("not accepting") {
            Err(PrinterError::Offline(name.to_string()))
        } else {
            Err(PrinterError::Print(err))
        }
    }
}

#[cfg(windows)]
async fn send_raw_inner(name: &str, data: Vec<u8>) -> Result<(), PrinterError> {
    // The winspool calls are blocking, so run them on a blocking thread.
    let name = name.to_string();
    tokio::task::spawn_blocking(move || win_raw_print(&name, &data))
        .await
        .map_err(|e| PrinterError::Print(format!("print task panicked: {e}")))?
}

#[cfg(windows)]
fn win_raw_print(name: &str, data: &[u8]) -> Result<(), PrinterError> {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Graphics::Printing::{
        ClosePrinter, EndDocPrinter, EndPagePrinter, OpenPrinterW, StartDocPrinterW,
        StartPagePrinter, WritePrinter, DOC_INFO_1W, PRINTER_DEFAULTSW,
    };

    // UTF-16, null-terminated, for the Win32 W APIs.
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let mut printer_name = wide(name);
    let mut doc_name = wide("ESC/POS Ticket");
    let mut datatype = wide("RAW");

    unsafe {
        let mut handle = HANDLE::default();
        let defaults = PRINTER_DEFAULTSW::default();
        OpenPrinterW(
            PCWSTR(printer_name.as_mut_ptr()),
            &mut handle,
            Some(&defaults),
        )
        .map_err(|e| {
            // Access-denied / not-found surfaces here.
            PrinterError::Print(format!("OpenPrinter failed for '{name}': {e}"))
        })?;

        // DOC_INFO_1W fields are PWSTR (mutable wide-string pointers) in the
        // windows crate, hence PWSTR here rather than PCWSTR.
        let doc_info = DOC_INFO_1W {
            pDocName: PWSTR(doc_name.as_mut_ptr()),
            pOutputFile: PWSTR::null(),
            pDatatype: PWSTR(datatype.as_mut_ptr()),
        };

        let result = (|| {
            let job = StartDocPrinterW(handle, 1, &doc_info);
            if job == 0 {
                return Err(PrinterError::Offline(name.to_string()));
            }
            StartPagePrinter(handle)
                .ok()
                .map_err(|e| PrinterError::Print(format!("StartPagePrinter: {e}")))?;

            let mut written: u32 = 0;
            WritePrinter(
                handle,
                data.as_ptr() as *const _,
                data.len() as u32,
                &mut written,
            )
            .ok()
            .map_err(|e| PrinterError::Print(format!("WritePrinter: {e}")))?;

            if (written as usize) != data.len() {
                return Err(PrinterError::Print(format!(
                    "only {written} of {} bytes written",
                    data.len()
                )));
            }

            EndPagePrinter(handle)
                .ok()
                .map_err(|e| PrinterError::Print(format!("EndPagePrinter: {e}")))?;
            EndDocPrinter(handle)
                .ok()
                .map_err(|e| PrinterError::Print(format!("EndDocPrinter: {e}")))?;
            Ok(())
        })();

        let _ = ClosePrinter(handle);
        result
    }
}

#[cfg(not(any(unix, windows)))]
async fn send_raw_inner(_name: &str, _data: Vec<u8>) -> Result<(), PrinterError> {
    Err(PrinterError::Print("printing unsupported on this platform".into()))
}
