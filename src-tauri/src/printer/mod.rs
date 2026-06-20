// Printer service for the Phase 1 prototype.
//
// All hardware access lives here, inside the Tauri process. The OS print
// spooler (CUPS / Windows) is the transport, so installed printer drivers
// handle the actual USB/network link.

pub mod commands;
pub mod connect;
pub mod discovery;
pub mod error;
pub mod escpos;
pub mod model;
pub mod spooler;
pub mod status;
