// Test ticket generation + reprint.
// Generates a sample ESC/POS ticket (company, ticket #, date/time, QR, footer)
// and prints it via the Rust spooler. "Reprint last" re-sends the same ticket
// number — no new ID is generated.
import { useEffect, useState } from "react";
import { QRCodeSVG } from "qrcode.react";
import {
  Ticket,
  getLastTicket,
  printTicket,
  reprintLast,
} from "./tauri";

interface Props {
  /** Default printer name resolved by the parent; null if none chosen. */
  printer: string | null;
}

const DEFAULT_COMPANY = "TEST EVENT";
const DEFAULT_FOOTER = "Thank You";

export default function TestTicket({ printer }: Props) {
  const [company, setCompany] = useState(DEFAULT_COMPANY);
  const [footer, setFooter] = useState(DEFAULT_FOOTER);
  const [last, setLast] = useState<Ticket | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  async function loadLast() {
    try {
      setLast(await getLastTicket());
    } catch {
      /* non-fatal: the panel just stays empty */
    }
  }

  useEffect(() => {
    loadLast();
  }, []);

  function guard(): boolean {
    if (!printer) {
      setErr("No default printer. Choose one in Printer settings first.");
      return false;
    }
    return true;
  }

  async function handlePrint() {
    if (!guard()) return;
    setBusy(true);
    setMsg(null);
    setErr(null);
    try {
      const r = await printTicket(printer as string, company, footer);
      if (r.ok) {
        setMsg(r.message);
        if (r.ticket) setLast(r.ticket);
      } else {
        setErr(r.message);
      }
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleReprint() {
    if (!guard()) return;
    setBusy(true);
    setMsg(null);
    setErr(null);
    try {
      const r = await reprintLast(printer as string);
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
      <h2 className="card__title">Test ticket</h2>
      <p className="muted">
        Prints a sample thermal ticket through the selected printer using raw
        ESC/POS commands (the QR is rendered by the printer itself).
      </p>

      <div className="form-row">
        <label className="field">
          <span>Company / event name</span>
          <input value={company} onChange={(e) => setCompany(e.target.value)} />
        </label>
        <label className="field">
          <span>Footer message</span>
          <input value={footer} onChange={(e) => setFooter(e.target.value)} />
        </label>
      </div>

      <div className="form-row" style={{ marginTop: 12 }}>
        <button
          className="btn btn--primary"
          onClick={handlePrint}
          disabled={busy}
        >
          {busy ? "Printing…" : "Test print"}
        </button>
        <button
          className="btn"
          onClick={handleReprint}
          disabled={busy || !last}
        >
          Reprint last ticket
        </button>
      </div>

      {msg && <div className="alert alert--ok" style={{ marginTop: 12 }}>{msg}</div>}
      {err && <div className="alert" style={{ marginTop: 12 }}>{err}</div>}

      {last && (
        <div className="last-ticket">
          <h3 className="card__title">Last printed ticket</h3>
          <div className="ticket ticket--preview">
            <p className="ticket__brand">{last.company || DEFAULT_COMPANY}</p>
            <div className="ticket__divider" />
            <p className="ticket__meta">Ticket ID: {last.ticket_number}</p>
            <p className="ticket__meta">Date: {last.printed_at}</p>
            <QRCodeSVG value={last.qr_data} size={140} level="H" />
            <p className="ticket__note">{last.footer || DEFAULT_FOOTER}</p>
          </div>
        </div>
      )}
    </section>
  );
}
