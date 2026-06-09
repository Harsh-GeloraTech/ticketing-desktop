// Raw ESC/POS command builder.
//
// We emit a plain `Vec<u8>` and hand it to the OS spooler as a raw job, so
// the printer renders everything itself — including the QR code via the
// native `GS ( k` (model 2) commands. No image rasterizing, no extra deps.
//
// This module is pure (no I/O), which means it is fully unit-testable without
// a physical printer — see the tests at the bottom.

// --- Control bytes ---------------------------------------------------------
const ESC: u8 = 0x1B;
const GS: u8 = 0x1D;
const LF: u8 = 0x0A;

/// A small builder that accumulates ESC/POS bytes.
#[derive(Default)]
pub struct EscPos {
    buf: Vec<u8>,
}

impl EscPos {
    pub fn new() -> Self {
        let mut b = Self { buf: Vec::new() };
        b.init();
        b
    }

    /// `ESC @` — reset the printer to a known state.
    fn init(&mut self) -> &mut Self {
        self.buf.extend_from_slice(&[ESC, b'@']);
        self
    }

    /// `ESC a n` — 0=left, 1=center, 2=right.
    pub fn align(&mut self, n: u8) -> &mut Self {
        self.buf.extend_from_slice(&[ESC, b'a', n]);
        self
    }

    /// `GS ! n` — character size. Low nibble = height mult, high nibble = width mult.
    /// `double()` => width x2, height x2.
    pub fn size(&mut self, width_mult: u8, height_mult: u8) -> &mut Self {
        let w = (width_mult.saturating_sub(1) & 0x07) << 4;
        let h = height_mult.saturating_sub(1) & 0x07;
        self.buf.extend_from_slice(&[GS, b'!', w | h]);
        self
    }

    /// `ESC E n` — bold on/off.
    pub fn bold(&mut self, on: bool) -> &mut Self {
        self.buf.extend_from_slice(&[ESC, b'E', on as u8]);
        self
    }

    /// Write a line of text followed by a line feed. Non-ASCII is replaced with '?'
    /// to stay within the printer's default code page.
    pub fn line(&mut self, text: &str) -> &mut Self {
        for ch in text.chars() {
            self.buf.push(if ch.is_ascii() { ch as u8 } else { b'?' });
        }
        self.buf.push(LF);
        self
    }

    /// Feed `n` blank lines.
    pub fn feed(&mut self, n: u8) -> &mut Self {
        self.buf.extend_from_slice(&[ESC, b'd', n]);
        self
    }

    /// Native ESC/POS QR code (`GS ( k`, model 2).
    /// Steps: select model -> set module size -> set error correction -> store data -> print.
    pub fn qr(&mut self, data: &str, module_size: u8) -> &mut Self {
        let bytes = data.as_bytes();

        // [Fn 165] Select QR model 2.
        self.buf
            .extend_from_slice(&[GS, b'(', b'k', 0x04, 0x00, 0x31, 0x41, 0x32, 0x00]);

        // [Fn 167] Module size (1..=16).
        let size = module_size.clamp(1, 16);
        self.buf
            .extend_from_slice(&[GS, b'(', b'k', 0x03, 0x00, 0x31, 0x43, size]);

        // [Fn 169] Error correction level: 48=L,49=M,50=Q,51=H. Use H for robustness.
        self.buf
            .extend_from_slice(&[GS, b'(', b'k', 0x03, 0x00, 0x31, 0x45, 0x33]);

        // [Fn 180] Store the data in the symbol buffer.
        // pL/pH = len(data) + 3.
        let len = bytes.len() + 3;
        let pl = (len & 0xFF) as u8;
        let ph = ((len >> 8) & 0xFF) as u8;
        self.buf
            .extend_from_slice(&[GS, b'(', b'k', pl, ph, 0x31, 0x50, 0x30]);
        self.buf.extend_from_slice(bytes);

        // [Fn 181] Print the stored symbol.
        self.buf
            .extend_from_slice(&[GS, b'(', b'k', 0x03, 0x00, 0x31, 0x51, 0x30]);
        self
    }

    /// `GS V 66 n` — feed and partial cut.
    pub fn cut(&mut self) -> &mut Self {
        self.buf.extend_from_slice(&[GS, b'V', 66, 0x00]);
        self
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

/// Build a full ticket buffer.
/// `printed_at` is passed in (the caller stamps the time) to keep this pure.
pub fn build_ticket(
    company: &str,
    ticket_number: &str,
    printed_at: &str,
    qr_data: &str,
    footer: &str,
) -> Vec<u8> {
    let mut p = EscPos::new();
    p.align(1) // center
        .size(2, 2)
        .bold(true)
        .line(company)
        .bold(false)
        .size(1, 1)
        .line("--------------------------------")
        .align(0) // left
        .line(&format!("Ticket: {ticket_number}"))
        .line(&format!("Date:   {printed_at}"))
        .feed(1)
        .align(1) // center QR
        .qr(qr_data, 6)
        .feed(1)
        .line(ticket_number)
        .feed(1)
        .line(footer)
        .feed(3)
        .cut();
    p.into_bytes()
}

/// A tiny buffer used to validate the print path without issuing a real ticket.
pub fn build_test_page(printer_name: &str, printed_at: &str) -> Vec<u8> {
    let mut p = EscPos::new();
    p.align(1)
        .size(2, 2)
        .bold(true)
        .line("PRINTER OK")
        .bold(false)
        .size(1, 1)
        .line("--------------------------------")
        .align(0)
        .line(&format!("Printer: {printer_name}"))
        .line(&format!("Time:    {printed_at}"))
        .align(1)
        .feed(1)
        .qr("PRINTER-TEST", 5)
        .feed(1)
        .line("Thermal test successful")
        .feed(3)
        .cut();
    p.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_init() {
        let bytes = EscPos::new().into_bytes();
        assert_eq!(&bytes[0..2], &[ESC, b'@'], "buffer must start with ESC @");
    }

    #[test]
    fn align_center_emitted() {
        let mut p = EscPos::new();
        p.align(1);
        let bytes = p.into_bytes();
        // after the 2-byte init, the next 3 bytes are ESC a 1
        assert_eq!(&bytes[2..5], &[ESC, b'a', 1]);
    }

    #[test]
    fn qr_embeds_payload_and_print_command() {
        let mut p = EscPos::new();
        p.qr("HELLO", 6);
        let bytes = p.into_bytes();
        // The raw payload bytes must appear in the stream.
        let needle = b"HELLO";
        assert!(
            bytes.windows(needle.len()).any(|w| w == needle),
            "QR payload must be embedded in the buffer"
        );
        // The print command (Fn 181) must be present at the end.
        let print_cmd = [GS, b'(', b'k', 0x03, 0x00, 0x31, 0x51, 0x30];
        assert!(
            bytes
                .windows(print_cmd.len())
                .any(|w| w == print_cmd),
            "QR print command (Fn 181) must be present"
        );
    }

    #[test]
    fn ticket_contains_number_and_cut() {
        let bytes = build_ticket("TEST EVENT", "TKT-0001", "2026-06-09 10:00", "TKT-0001", "Thank You");
        let needle = b"TKT-0001";
        assert!(
            bytes.windows(needle.len()).any(|w| w == needle),
            "ticket number must appear in the buffer"
        );
        let cut = [GS, b'V', 66, 0x00];
        assert!(
            bytes.windows(cut.len()).any(|w| w == cut),
            "ticket must end with a cut command"
        );
    }

    #[test]
    fn non_ascii_is_sanitized() {
        let mut p = EscPos::new();
        p.line("café");
        let bytes = p.into_bytes();
        // 'é' becomes '?', so the byte for '?' (0x3F) must be present.
        assert!(bytes.contains(&0x3F), "non-ascii should be replaced with '?'");
    }
}
