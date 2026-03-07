import type React from 'react';

// ── Field wrapper ─────────────────────────────────────────────────────────────

export function Field({
  label,
  hint,
  children,
  className,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <label className={`field${className ? ` ${className}` : ''}`}>
      {label}
      {children}
      {hint && <span className="field-hint">{hint}</span>}
    </label>
  );
}

// ── Text input ────────────────────────────────────────────────────────────────

export function TextInput({
  value,
  onChange,
  placeholder,
  disabled,
  type = 'text',
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  disabled?: boolean;
  type?: string;
}) {
  return (
    <input
      type={type}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      disabled={disabled}
    />
  );
}

// ── Text area ─────────────────────────────────────────────────────────────────

export function TextArea({
  value,
  onChange,
  placeholder,
  rows = 3,
  disabled,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  rows?: number;
  disabled?: boolean;
}) {
  return (
    <textarea
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      rows={rows}
      disabled={disabled}
    />
  );
}

// ── Toggle / checkbox ─────────────────────────────────────────────────────────

export function Toggle({
  label,
  checked,
  onChange,
  disabled,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <div className="toggle-row">
      <label>
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => onChange(e.target.checked)}
          disabled={disabled}
        />
        {label}
      </label>
    </div>
  );
}

// ── Accordion section ─────────────────────────────────────────────────────────

export function Accordion({
  id,
  icon,
  title,
  summary,
  open,
  onToggle,
  children,
}: {
  id: string;
  icon?: string;
  title: string;
  summary?: string;
  open: boolean;
  onToggle: (id: string) => void;
  children: React.ReactNode;
}) {
  return (
    <div className="accordion">
      <button className="accordion-header" type="button" onClick={() => onToggle(id)}>
        <div className="accordion-title">
          {icon && <span className="accordion-icon">{icon}</span>}
          <div>
            <h3>{title}</h3>
            {summary && <div className="accordion-summary">{summary}</div>}
          </div>
        </div>
        <span className={`chevron${open ? ' open' : ''}`}>▶</span>
      </button>
      {open && <div className="accordion-body">{children}</div>}
    </div>
  );
}

// ── useAccordion hook ─────────────────────────────────────────────────────────

import { useState } from 'react';

export function useAccordion(initial: string[] = []) {
  const [open, setOpen] = useState(new Set(initial));

  const toggle = (id: string) => {
    setOpen((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const is = (id: string) => open.has(id);

  return { toggle, is };
}
