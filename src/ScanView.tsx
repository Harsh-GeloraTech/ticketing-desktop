// src/ScanView.tsx
import { useEffect, useRef, useState } from "react";
import { Html5Qrcode } from "html5-qrcode";
import { validateTicket, ValidationResult } from "./api";

export default function ScanView() {
  const [result, setResult] = useState<ValidationResult | null>(null);
  const [scanning, setScanning] = useState(false);
  const [manual, setManual] = useState("");
  const scannerRef = useRef<Html5Qrcode | null>(null);
  const lastCode = useRef<string>("");

  async function check(code: string) {
    setResult(null);
    try {
      const r = await validateTicket(code);
      setResult(r);
    } catch (e) {
      setResult({
        valid: false,
        reason: (e as Error).message,
        ticket_code: null,
        valid_date: null,
      });
    }
  }

  async function stopCamera() {
    const s = scannerRef.current;
    if (s) {
      try {
        await s.stop();
        await s.clear();
      } catch {
        /* ignore */
      }
      scannerRef.current = null;
    }
    setScanning(false);
  }

  async function startCamera() {
    setResult(null);
    lastCode.current = "";
    try {
      const cameras = await Html5Qrcode.getCameras();
      if (!cameras.length) throw new Error("No camera detected on this device");

      const scanner = new Html5Qrcode("reader");
      scannerRef.current = scanner;
      setScanning(true);

      await scanner.start(
        cameras[0].id,
        { fps: 10, qrbox: 240 },
        (decoded) => {
          // ignore repeated reads of the same code
          if (decoded && decoded !== lastCode.current) {
            lastCode.current = decoded;
            stopCamera();
            check(decoded);
          }
        },
        () => {
          /* per-frame decode misses are normal — ignore */
        },
      );
    } catch (e) {
      setScanning(false);
      setResult({
        valid: false,
        reason: "Camera unavailable: " + (e as Error).message,
        ticket_code: null,
        valid_date: null,
      });
    }
  }

  // Clean up the camera if the user leaves this view.
  useEffect(() => {
    return () => {
      stopCamera();
    };
  }, []);

  return (
    <div className="scan">
      <section className="card">
        <h2 className="card__title">Scan to validate entry</h2>

        <div id="reader" className="reader" />

        <div className="scan__controls">
          {!scanning ? (
            <button className="btn btn--primary" onClick={startCamera}>
              Start camera scan
            </button>
          ) : (
            <button className="btn" onClick={stopCamera}>
              Stop camera
            </button>
          )}
        </div>

        <div className="scan__manual">
          <p className="muted">Or enter a ticket code manually (demo fallback):</p>
          <div className="form-row">
            <input
              className="scan__input mono"
              placeholder="TKT-…"
              value={manual}
              onChange={(e) => setManual(e.target.value)}
            />
            <button
              className="btn"
              onClick={() => manual.trim() && check(manual.trim())}
            >
              Validate
            </button>
          </div>
        </div>
      </section>

      {result && (
        <div className={`verdict ${result.valid ? "verdict--ok" : "verdict--fail"}`}>
          <div className="verdict__icon">{result.valid ? "✓" : "✗"}</div>
          <div className="verdict__title">
            {result.valid ? "Entry granted" : "Entry denied"}
          </div>
          <div className="verdict__reason">{result.reason}</div>
          {result.ticket_code && (
            <div className="verdict__code mono">{result.ticket_code}</div>
          )}
        </div>
      )}
    </div>
  );
}