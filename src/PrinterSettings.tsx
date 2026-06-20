// Printer Settings screen.
// Lists OS printers, lets the user pick + save a default, test the connection,
// and shows live status per printer.
import { useCallback, useEffect, useState } from "react";
import {
  PrinterInfo,
  connectPrinter,
  listPrinters,
  refreshPrinters,
  setDefaultPrinter,
  testPrint,
} from "./tauri";

interface Props {
  /** Notifies the parent (App) when the saved default changes. */
  onDefaultChange?: (name: string | null) => void;
}

export default function PrinterSettings({ onDefaultChange }: Props) {
  const [printers, setPrinters] = useState<PrinterInfo[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  // The printer ID we have a CONFIRMED connection to. Save/Test stay disabled
  // until this equals the selected printer.
  const [connectedId, setConnectedId] = useState<string | null>(null);

  // A connection is only valid for the printer it was made against.
  const isConnected = connectedId !== null && connectedId === selected;

  // Switching printers (or losing the list) invalidates any prior connection.
  function selectPrinter(name: string) {
    setSelected(name);
    setConnectedId(null);
    setMsg(null);
    setErr(null);
  }

  async function handleConnect() {
    if (!selected) return;
    setBusy(true);
    setMsg(null);
    setErr(null);
    try {
      const r = await connectPrinter(selected);
      if (r.connected) {
        setConnectedId(r.printer_id);
        setMsg(`Connected to "${r.printer_id}". You can now save or print.`);
      } else {
        setConnectedId(null);
        setErr(`Cannot connect to "${selected}": ${r.detail}`);
      }
    } catch (e) {
      setConnectedId(null);
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  const load = useCallback(
    async (rescan: boolean) => {
      setErr(null);
      try {
        const list = rescan ? await refreshPrinters() : await listPrinters();
        setPrinters(list);
        const def = list.find((p) => p.is_default)?.name ?? null;
        setSelected((cur) => cur ?? def ?? list[0]?.name ?? null);
        // A rescan can change device availability, so any prior connection must
        // be re-confirmed before actions are allowed again.
        if (rescan) setConnectedId(null);
      } catch (e) {
        setErr(String(e));
      }
    },
    [],
  );

  useEffect(() => {
    load(false);
  }, [load]);

  async function handleRefresh() {
    setBusy(true);
    setMsg(null);
    await load(true);
    setBusy(false);
  }

  async function handleSaveDefault() {
    if (!selected || !isConnected) return;
    setBusy(true);
    setErr(null);
    try {
      await setDefaultPrinter(selected);
      setMsg(`Saved "${selected}" as the default printer.`);
      onDefaultChange?.(selected);
      await load(false);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleTest() {
    if (!selected || !isConnected) return;
    setBusy(true);
    setMsg(null);
    setErr(null);
    try {
      const r = await testPrint(selected);
      if (r.ok) setMsg(r.message);
      else setErr(r.message);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="card">
      <div className="card__head">
        <h2 className="card__title">Printer settings</h2>
        <button className="btn" onClick={handleRefresh} disabled={busy}>
          {busy ? "Working…" : "Refresh list"}
        </button>
      </div>

      {msg && <div className="alert alert--ok">{msg}</div>}
      {err && <div className="alert">{err}</div>}

      {printers.length === 0 ? (
        <p className="muted">
          No printers detected. Connect a thermal printer (or add it in your OS
          print settings), then click “Refresh list”.
        </p>
      ) : (
        <table className="tbl">
          <thead>
            <tr>
              <th aria-label="select" />
              <th>Printer name</th>
              <th>Status</th>
              <th>Type</th>
              <th>Default</th>
            </tr>
          </thead>
          <tbody>
            {printers.map((p) => (
              <tr key={p.name}>
                <td>
                  <input
                    type="radio"
                    name="default-printer"
                    checked={selected === p.name}
                    onChange={() => selectPrinter(p.name)}
                  />
                </td>
                <td className="mono">{p.name}</td>
                <td>
                  <span
                    className={`pill ${
                      p.status.connected ? "pill--active" : "pill--cancelled"
                    }`}
                  >
                    {p.status.connected ? "🟢 Connected" : "🔴 Disconnected"}
                  </span>
                  <div className="muted small">{p.status.detail}</div>
                </td>
                <td>{p.kind}</td>
                <td>{p.is_default ? "✓" : ""}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <div className="form-row" style={{ marginTop: 16 }}>
        <button
          className="btn btn--primary"
          onClick={handleConnect}
          disabled={busy || !selected || isConnected}
        >
          {isConnected ? "Connected ✓" : "Connect"}
        </button>
        <button
          className="btn"
          onClick={handleSaveDefault}
          disabled={busy || !selected || !isConnected}
        >
          Save default
        </button>
        <button
          className="btn"
          onClick={handleTest}
          disabled={busy || !selected || !isConnected}
        >
          Test connection
        </button>
      </div>

      {selected && !isConnected && (
        <p className="muted small" style={{ marginTop: 8 }}>
          Connect to “{selected}” first — Save default and Test connection unlock
          only after a confirmed connection.
        </p>
      )}
    </section>
  );
}
