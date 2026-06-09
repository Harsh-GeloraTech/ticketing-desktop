// src/api.ts
// Thin typed client over the Axum backend.

const API = "http://localhost:8080";

export interface Ticket {
  id: number;
  ticket_code: string;
  invoice_id: number | null;
  valid_date: string;
  status: string;
  used_at: string | null;
  created_at: string;
}

export interface ValidationResult {
  valid: boolean;
  reason: string;
  ticket_code: string | null;
  valid_date: string | null;
}

async function handle<T>(res: Response): Promise<T> {
  if (!res.ok) {
    const detail = await res.text().catch(() => "");
    throw new Error(detail || `Request failed (${res.status})`);
  }
  if (res.status === 204) return undefined as T;
  return res.json();
}

export function listTickets(): Promise<Ticket[]> {
  return fetch(`${API}/tickets`).then((r) => handle<Ticket[]>(r));
}

export function createTicket(valid_date: string): Promise<Ticket> {
  return fetch(`${API}/tickets`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ valid_date }),
  }).then((r) => handle<Ticket>(r));
}

export function updateTicketStatus(id: number, status: string): Promise<Ticket> {
  return fetch(`${API}/tickets/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ status }),
  }).then((r) => handle<Ticket>(r));
}

export function deleteTicket(id: number): Promise<void> {
  return fetch(`${API}/tickets/${id}`, { method: "DELETE" }).then((r) =>
    handle<void>(r),
  );
}

// The scanner sends the decoded code here; backend grants or denies entry.
export function validateTicket(ticket_code: string): Promise<ValidationResult> {
  return fetch(`${API}/validate`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ ticket_code }),
  }).then((r) => handle<ValidationResult>(r));
}