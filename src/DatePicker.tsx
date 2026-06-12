// A small, dependency-free date picker we fully control.
//
// Replaces the native <input type="date">, whose popup does NOT reliably close
// on outside-click in WebKitGTK (Linux). This one closes on outside-click and
// on Escape, on every platform, and emits a "YYYY-MM-DD" string so callers are
// unchanged.

import { useEffect, useRef, useState } from "react";

interface Props {
  value: string; // "YYYY-MM-DD" or ""
  onChange: (value: string) => void;
  placeholder?: string;
}

const WEEKDAYS = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
const MONTHS = [
  "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December",
];

function pad(n: number): string {
  return n < 10 ? `0${n}` : `${n}`;
}

function toISO(y: number, m: number, d: number): string {
  return `${y}-${pad(m + 1)}-${pad(d)}`;
}

// Parse "YYYY-MM-DD" into parts, or return today's parts if empty/invalid.
function parse(value: string): { y: number; m: number; d: number } {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (match) {
    return { y: +match[1], m: +match[2] - 1, d: +match[3] };
  }
  const now = new Date();
  return { y: now.getFullYear(), m: now.getMonth(), d: now.getDate() };
}

export default function DatePicker({ value, onChange, placeholder }: Props) {
  const [open, setOpen] = useState(false);
  // The month currently shown in the calendar grid.
  const initial = parse(value);
  const [viewY, setViewY] = useState(initial.y);
  const [viewM, setViewM] = useState(initial.m);
  const rootRef = useRef<HTMLDivElement | null>(null);

  // Keep the visible month in sync when an external value arrives.
  useEffect(() => {
    const p = parse(value);
    setViewY(p.y);
    setViewM(p.m);
  }, [value]);

  // Close on outside-click and on Escape — the whole point of this component.
  useEffect(() => {
    if (!open) return;

    function onDown(e: MouseEvent) {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }

    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  function prevMonth() {
    setViewM((m) => {
      if (m === 0) {
        setViewY((y) => y - 1);
        return 11;
      }
      return m - 1;
    });
  }
  function nextMonth() {
    setViewM((m) => {
      if (m === 11) {
        setViewY((y) => y + 1);
        return 0;
      }
      return m + 1;
    });
  }

  function pick(day: number) {
    onChange(toISO(viewY, viewM, day));
    setOpen(false);
  }

  // Build the calendar grid for the current view month.
  const firstWeekday = new Date(viewY, viewM, 1).getDay(); // 0=Sun
  const daysInMonth = new Date(viewY, viewM + 1, 0).getDate();
  const cells: (number | null)[] = [];
  for (let i = 0; i < firstWeekday; i++) cells.push(null);
  for (let d = 1; d <= daysInMonth; d++) cells.push(d);

  const selected = parse(value);
  const hasValue = /^\d{4}-\d{2}-\d{2}$/.test(value);

  return (
    <div className="dp" ref={rootRef}>
      <input
        className="dp__input"
        type="text"
        readOnly
        value={value}
        placeholder={placeholder ?? "YYYY-MM-DD"}
        onClick={() => setOpen((o) => !o)}
      />

      {open && (
        <div className="dp__pop" role="dialog" aria-label="Choose date">
          <div className="dp__head">
            <button type="button" className="dp__nav" onClick={prevMonth} aria-label="Previous month">
              ‹
            </button>
            <span className="dp__title">
              {MONTHS[viewM]} {viewY}
            </span>
            <button type="button" className="dp__nav" onClick={nextMonth} aria-label="Next month">
              ›
            </button>
          </div>

          <div className="dp__grid dp__grid--head">
            {WEEKDAYS.map((w) => (
              <span key={w} className="dp__wd">
                {w}
              </span>
            ))}
          </div>

          <div className="dp__grid">
            {cells.map((d, i) =>
              d === null ? (
                <span key={`e${i}`} className="dp__cell dp__cell--empty" />
              ) : (
                <button
                  key={d}
                  type="button"
                  className={`dp__cell ${
                    hasValue &&
                    selected.y === viewY &&
                    selected.m === viewM &&
                    selected.d === d
                      ? "dp__cell--selected"
                      : ""
                  }`}
                  onClick={() => pick(d)}
                >
                  {d}
                </button>
              ),
            )}
          </div>
        </div>
      )}
    </div>
  );
}
