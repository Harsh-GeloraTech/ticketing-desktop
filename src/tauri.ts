// Typed wrappers over the Tauri printer commands (src-tauri/src/printer/commands.rs).
// All hardware access happens in Rust; the UI only calls these.

import { invoke } from "@tauri-apps/api/core";

export interface PrinterStatus {
  connected: boolean;
  detail: string;
}

export interface PrinterInfo {
  name: string;
  kind: string;
  is_default: boolean;
  is_system: boolean;
  status: PrinterStatus;
}

export interface Ticket {
  id: number;
  ticket_number: string;
  qr_data: string;
  company: string;
  footer: string;
  printed_at: string;
}

export interface PrintResult {
  ok: boolean;
  message: string;
  ticket: Ticket | null;
}

export interface ConnectResult {
  connected: boolean;
  printer_id: string | null;
  detail: string;
}

export const listPrinters = () => invoke<PrinterInfo[]>("list_printers");
export const refreshPrinters = () => invoke<PrinterInfo[]>("refresh_printers");
export const getPrinterStatus = (name: string) =>
  invoke<PrinterStatus>("get_printer_status", { name });
export const connectPrinter = (name: string) =>
  invoke<ConnectResult>("connect_printer", { name });
export const setDefaultPrinter = (name: string) =>
  invoke<void>("set_default_printer", { name });
export const getDefaultPrinter = () =>
  invoke<string | null>("get_default_printer");
export const testPrint = (name: string) =>
  invoke<PrintResult>("test_print", { name });
export const printTicket = (name: string, company: string, footer: string) =>
  invoke<PrintResult>("print_ticket", { name, company, footer });
export const getLastTicket = () => invoke<Ticket | null>("get_last_ticket");
export const reprintLast = (name: string) =>
  invoke<PrintResult>("reprint_last", { name });
