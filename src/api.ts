// src/api.ts
// Local-first ticket client. These now call the Tauri Rust core via invoke()
// instead of the HTTP backend, so the built app is fully self-contained (no
// localhost server needed). Cloud sync happens in the background in Rust.
//
// The exported function NAMES and TYPES are unchanged, so App.tsx and
// ScanView.tsx need no modifications. Tauri maps the camelCase argument keys
// below to the snake_case Rust command parameters automatically.

import { invoke } from "@tauri-apps/api/core";

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

export function listTickets(): Promise<Ticket[]> {
  return invoke<Ticket[]>("list_tickets");
}

export function createTicket(valid_date: string): Promise<Ticket> {
  // Rust param `valid_date` <- camelCase `validDate`.
  return invoke<Ticket>("create_ticket", { validDate: valid_date });
}

export function updateTicketStatus(id: number, status: string): Promise<Ticket> {
  return invoke<Ticket>("update_ticket", { id, status });
}

export function deleteTicket(id: number): Promise<void> {
  return invoke<void>("delete_ticket", { id });
}

// The scanner sends the decoded code here; the local core grants or denies entry
// (and queues the change for cloud sync).
export function validateTicket(ticket_code: string): Promise<ValidationResult> {
  return invoke<ValidationResult>("validate_ticket", { ticketCode: ticket_code });
}
