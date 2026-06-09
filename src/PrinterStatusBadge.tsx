// Live 🟢/🔴 indicator for a printer. Polls the Rust status command on an
// interval so it reflects USB-removed / powered-off / reconnected without a
// page refresh.
import { useEffect, useState } from "react";
import { getPrinterStatus, PrinterStatus } from "./tauri";

const POLL_MS = 3000;

export default function PrinterStatusBadge({ name }: { name: string | null }) {
  const [status, setStatus] = useState<PrinterStatus | null>(null);

  useEffect(() => {
    if (!name) {
      setStatus(null);
      return;
    }
    let alive = true;

    async function poll() {
      try {
        const s = await getPrinterStatus(name as string);
        if (alive) setStatus(s);
      } catch {
        if (alive)
          setStatus({ connected: false, detail: "Status check failed" });
      }
    }

    poll();
    const id = setInterval(poll, POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [name]);

  if (!name) {
    return <span className="status status--none">No printer selected</span>;
  }

  const connected = status?.connected ?? false;
  return (
    <span className={`status ${connected ? "status--ok" : "status--down"}`}>
      <span className="status__dot">{connected ? "🟢" : "🔴"}</span>
      <span className="status__text">
        {connected ? "Connected" : "Disconnected"}
        {status?.detail ? ` · ${status.detail}` : ""}
      </span>
    </span>
  );
}
