// src/App.tsx
import { useEffect, useState } from "react";
import { QRCodeSVG } from "qrcode.react";
import {
  Ticket,
  listTickets,
  createTicket,
  deleteTicket,
  updateTicketStatus,
} from "./api";
import ScanView from "./ScanView";
import PrinterSettings from "./PrinterSettings";
import PrinterStatusBadge from "./PrinterStatusBadge";
import TestTicket from "./TestTicket";
import { getDefaultPrinter } from "./tauri";
import "./App.css";

type Tab = "console" | "scan" | "printer";

function App() {
  const [tab, setTab] = useState<Tab>("console");
  const [tickets, setTickets] = useState<Ticket[]>([]);
  const [validDate, setValidDate] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [printTicket, setPrintTicket] = useState<Ticket | null>(null);
  const [defaultPrinter, setDefaultPrinter] = useState<string | null>(null);

  async function refresh() {
    try {
      setTickets(await listTickets());
      setError(null);
    } catch (e) {
      setError((e as Error).message);
    }
  }

  useEffect(() => {
    if (tab === "console") refresh();
  }, [tab]);

  // Load the saved default printer once on startup so the Printer tab and the
  // status badge know which device to target.
  useEffect(() => {
    getDefaultPrinter()
      .then(setDefaultPrinter)
      .catch(() => setDefaultPrinter(null));
  }, []);

  async function handleCreate() {
    if (!validDate) {
      setError("Pick a valid date first.");
      return;
    }
    setBusy(true);
    try {
      await createTicket(validDate);
      setValidDate("");
      await refresh();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  async function handleToggleUsed(t: Ticket) {
    await updateTicketStatus(t.id, t.status === "used" ? "active" : "used");
    await refresh();
  }

  async function handleDelete(id: number) {
    await deleteTicket(id);
    await refresh();
  }

  useEffect(() => {
    if (!printTicket) return;
    const timer = setTimeout(() => {
      window.print();
      setPrintTicket(null);
    }, 120);
    return () => clearTimeout(timer);
  }, [printTicket]);

  return (
    <div className="app">
      <header className="app__header">
        <div>
          <p className="app__eyebrow">Eminence · Ticketing Platform</p>
          <h1 className="app__title">Ticket Console</h1>
        </div>
        <PrinterStatusBadge name={defaultPrinter} />
      </header>

      <nav className="tabs">
        <button
          className={`tab ${tab === "console" ? "tab--active" : ""}`}
          onClick={() => setTab("console")}
        >
          Issue &amp; manage
        </button>
        <button
          className={`tab ${tab === "scan" ? "tab--active" : ""}`}
          onClick={() => setTab("scan")}
        >
          Scan entry
        </button>
        <button
          className={`tab ${tab === "printer" ? "tab--active" : ""}`}
          onClick={() => setTab("printer")}
        >
          Printer
        </button>
      </nav>

      {tab === "scan" ? (
        <ScanView />
      ) : tab === "printer" ? (
        <>
          <PrinterSettings onDefaultChange={setDefaultPrinter} />
          <TestTicket printer={defaultPrinter} />
        </>
      ) : (
        <>
          {error && <div className="alert">{error}</div>}

          <section className="card card--create">
            <h2 className="card__title">Issue a ticket</h2>
            <div className="form-row">
              <label className="field">
                <span>Valid date</span>
                <input
                  type="date"
                  value={validDate}
                  onChange={(e) => setValidDate(e.target.value)}
                />
              </label>
              <button
                className="btn btn--primary"
                onClick={handleCreate}
                disabled={busy}
              >
                {busy ? "Issuing…" : "Issue ticket"}
              </button>
            </div>
          </section>

          <section className="card">
            <div className="card__head">
              <h2 className="card__title">Tickets</h2>
              <span className="count">{tickets.length}</span>
            </div>

            {tickets.length === 0 ? (
              <p className="muted">No tickets yet — issue one above.</p>
            ) : (
              <table className="tbl">
                <thead>
                  <tr>
                    <th>Code</th>
                    <th>Valid date</th>
                    <th>Status</th>
                    <th>QR</th>
                    <th aria-label="actions" />
                  </tr>
                </thead>
                <tbody>
                  {tickets.map((t) => (
                    <tr key={t.id}>
                      <td className="mono">{t.ticket_code}</td>
                      <td>{t.valid_date}</td>
                      <td>
                        <span className={`pill pill--${t.status}`}>{t.status}</span>
                      </td>
                      <td>
                        <QRCodeSVG value={t.ticket_code} size={48} level="M" />
                      </td>
                      <td className="actions">
                        <button className="btn" onClick={() => setPrintTicket(t)}>
                          Print
                        </button>
                        <button
                          className="btn btn--ghost"
                          onClick={() => handleToggleUsed(t)}
                        >
                          {t.status === "used" ? "Reactivate" : "Mark used"}
                        </button>
                        <button
                          className="btn btn--danger"
                          onClick={() => handleDelete(t.id)}
                        >
                          Delete
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>
        </>
      )}

      {printTicket && (
        <div id="print-area">
          <div className="ticket">
            <p className="ticket__brand">TEST  EVENTS</p>
            <div className="ticket__divider" />
            <QRCodeSVG value={printTicket.ticket_code} size={220} level="H" />
            <p className="ticket__code">{printTicket.ticket_code}</p>
            <p className="ticket__meta">Valid on {printTicket.valid_date}</p>
            <p className="ticket__note">Present this ticket at the entrance.</p>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;