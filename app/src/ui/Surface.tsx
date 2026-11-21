// What screens are made of: page headers, cards, pills, notes, empty states, fields with labels,
// switches. The look lives here, the screens only compose.

import { useId, type ComponentType, type ReactNode } from 'react';
import { Switch as S } from 'radix-ui';
import { Check, CircleAlert, Info, Minus, TriangleAlert } from 'lucide-react';

type Icon = ComponentType<{ className?: string }>;

/** A screen's title, what it is about, and its main actions. */
export function PageHeader({ title, subtitle, icon: Icon, actions, eyebrow }: { title: ReactNode; subtitle?: ReactNode; icon?: Icon; actions?: ReactNode; eyebrow?: ReactNode }) {
  return (
    <div className="flex flex-wrap items-end gap-x-4 gap-y-3">
      {/* At least a phone's width for the title: the actions go under it, never squeeze it. */}
      <div className="flex min-w-0 flex-1 basis-72 items-center gap-3">
        {Icon && (
          <div className="hidden size-11 shrink-0 place-items-center rounded-xl bg-gradient-to-br from-brand-light to-brand text-white shadow-lift sm:grid">
            <Icon className="size-5" />
          </div>
        )}
        <div className="min-w-0">
          {eyebrow && <div className="mb-0.5 text-xs font-semibold text-ink-3">{eyebrow}</div>}
          <h1 className="truncate text-[22px] leading-tight font-bold tracking-tight text-ink sm:text-2xl">{title}</h1>
          {subtitle && <div className="mt-0.5 line-clamp-2 text-[13px] text-ink-3 sm:line-clamp-1">{subtitle}</div>}
        </div>
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </div>
  );
}

export function Card({ children, className = '', padded = false }: { children: ReactNode; className?: string; padded?: boolean }) {
  return <section className={`rounded-2xl border border-line bg-card shadow-card ${padded ? 'p-4 sm:p-5' : ''} ${className}`}>{children}</section>;
}

/** A card's title row, with actions on the right. */
export function CardHeader({ title, subtitle, icon: Icon, actions, className = '' }: { title: ReactNode; subtitle?: ReactNode; icon?: Icon; actions?: ReactNode; className?: string }) {
  return (
    <div className={`flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-line px-4 py-3 sm:px-5 ${className}`}>
      {Icon && (
        <div className="grid size-8 shrink-0 place-items-center rounded-lg bg-sunken text-ink-2">
          <Icon className="size-4" />
        </div>
      )}
      <div className="min-w-0 flex-1 basis-48">
        <h2 className="truncate text-[15px] font-semibold tracking-tight text-ink">{title}</h2>
        {subtitle && <div className="line-clamp-2 text-xs text-ink-3 sm:line-clamp-1">{subtitle}</div>}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-1.5">{actions}</div>}
    </div>
  );
}

export type PillTone = 'neutral' | 'ok' | 'warn' | 'danger' | 'info' | 'accent' | 'brand';

const pillTones: Record<PillTone, string> = {
  neutral: 'bg-sunken text-ink-2 ring-line',
  ok: 'bg-ok/10 text-ok ring-ok/20',
  warn: 'bg-warn/12 text-warn ring-warn/25',
  danger: 'bg-danger/10 text-danger ring-danger/20',
  info: 'bg-info/10 text-info ring-info/20',
  accent: 'bg-accent-soft text-accent ring-accent/20',
  brand: 'bg-brand text-white ring-brand',
};

const dotTones: Record<PillTone, string> = { neutral: 'bg-ink-4', ok: 'bg-ok', warn: 'bg-warn', danger: 'bg-danger', info: 'bg-info', accent: 'bg-accent', brand: 'bg-white' };

/** A small label: a status, a mark (RW, NP, HC), a count. */
export function Pill({ children, tone = 'neutral', dot, icon: Icon, title, className = '' }: { children: ReactNode; tone?: PillTone; dot?: boolean; icon?: Icon; title?: string; className?: string }) {
  return (
    <span
      title={title}
      className={`inline-flex h-[22px] shrink-0 items-center gap-1.5 rounded-full px-2 text-[11px] font-semibold whitespace-nowrap ring-1 ring-inset ${pillTones[tone]} ${className}`}
    >
      {dot && <span className={`size-1.5 rounded-full ${dotTones[tone]}`} aria-hidden />}
      {Icon && <Icon className="size-3" />}
      {children}
    </span>
  );
}

/** A note in the flow of a screen: something to know, to watch, or that went wrong. */
export function Callout({ tone = 'info', title, children, actions }: { tone?: 'info' | 'warn' | 'danger'; title?: ReactNode; children?: ReactNode; actions?: ReactNode }) {
  const Icon = tone === 'danger' ? CircleAlert : tone === 'warn' ? TriangleAlert : Info;
  const tones = { info: 'border-info/25 bg-info/6 text-info', warn: 'border-warn/30 bg-warn/8 text-warn', danger: 'border-danger/30 bg-danger/8 text-danger' };
  return (
    <div className={`flex flex-wrap items-start gap-3 rounded-xl border px-4 py-3 ${tones[tone]}`}>
      <Icon className="mt-0.5 size-[18px] shrink-0" />
      <div className="min-w-0 flex-1 text-[13px] text-ink-2">
        {title && <div className="font-semibold text-ink">{title}</div>}
        {children}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </div>
  );
}

export function EmptyState({ icon: Icon, title, children, action }: { icon: Icon; title: ReactNode; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center px-6 py-14 text-center">
      <div className="mb-4 grid size-14 place-items-center rounded-2xl bg-gradient-to-br from-accent-soft to-sunken text-accent ring-1 ring-line">
        <Icon className="size-6" />
      </div>
      <div className="text-[15px] font-semibold text-ink">{title}</div>
      {children && <div className="mt-1 max-w-md text-[13px] text-ink-3">{children}</div>}
      {action && <div className="mt-5">{action}</div>}
    </div>
  );
}

/** A labelled field (the label above, a hint under). */
export function Field({ label, hint, children, className = '', group = false }: { label: ReactNode; hint?: ReactNode; children: ReactNode; className?: string; group?: boolean }) {
  const id = useId();
  const body = (
    <>
      <span id={id} className="text-xs font-semibold text-ink-2 first-letter:uppercase">
        {label}
      </span>
      {children}
      {hint && <span className="text-[11px] leading-snug text-ink-3">{hint}</span>}
    </>
  );
  // A group of choices (`group`): not a <label>, which hands every click inside to the first
  // button it holds (a picker in one of the choices would take the others' clicks).
  return group ? (
    <div role="group" aria-labelledby={id} className={`flex min-w-0 flex-col gap-1.5 ${className}`}>
      {body}
    </div>
  ) : (
    <label className={`flex min-w-0 flex-col gap-1.5 ${className}`}>{body}</label>
  );
}

/** An on/off setting, with what it does. */
export function Switch({ checked, onChange, label, hint, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: ReactNode; hint?: ReactNode; disabled?: boolean }) {
  return (
    <label className={`flex items-start justify-between gap-4 ${disabled ? 'opacity-50' : 'cursor-pointer'}`}>
      <span className="min-w-0">
        <span className="block text-[13px] font-semibold text-ink">{label}</span>
        {hint && <span className="block text-xs text-ink-3">{hint}</span>}
      </span>
      <S.Root
        checked={checked}
        onCheckedChange={onChange}
        disabled={disabled}
        className="relative mt-0.5 h-6 w-10 shrink-0 rounded-full bg-line-strong transition-colors outline-none focus-visible:ring-2 focus-visible:ring-info/50 data-[state=checked]:bg-accent"
      >
        <S.Thumb className="block size-5 translate-x-0.5 rounded-full bg-white shadow-card transition-transform data-[state=checked]:translate-x-[18px]" />
      </S.Root>
    </label>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="inline-flex h-5 min-w-5 items-center justify-center rounded border border-line-strong bg-card px-1 font-sans text-[10px] font-semibold text-ink-3 shadow-xs">{children}</kbd>;
}

/** A number with its label, large (a summary at the top of a screen). */
export function Stat({ label, value, icon: Icon }: { label: ReactNode; value: ReactNode; icon?: Icon }) {
  return (
    <div className="flex items-center gap-3 rounded-xl border border-line bg-card px-4 py-3 shadow-card">
      {Icon && (
        <div className="grid size-9 place-items-center rounded-lg bg-sunken text-ink-2">
          <Icon className="size-[18px]" />
        </div>
      )}
      <div className="min-w-0">
        <div className="text-lg leading-tight font-bold text-ink tabular">{value}</div>
        <div className="truncate text-xs text-ink-3">{label}</div>
      </div>
    </div>
  );
}

/** While a screen loads: its shape, softly pulsing (no jump when it arrives). */
export function Loading({ rows = 6 }: { rows?: number }) {
  return (
    <div className="mx-auto max-w-[1180px] animate-pulse space-y-5 px-3 py-5 sm:px-6 sm:py-7" aria-busy="true">
      <div className="flex items-center gap-3">
        <div className="size-11 rounded-xl bg-sunken" />
        <div className="space-y-2">
          <div className="h-5 w-48 rounded-md bg-sunken" />
          <div className="h-3 w-72 max-w-[60vw] rounded-md bg-sunken" />
        </div>
      </div>
      <div className="h-10 w-full max-w-md rounded-lg bg-sunken" />
      <div className="overflow-hidden rounded-2xl border border-line bg-card">
        {Array.from({ length: rows }, (_, i) => (
          <div key={i} className="flex items-center gap-3 border-t border-line px-4 py-3.5 first:border-t-0">
            <div className="size-10 rounded-xl bg-sunken" />
            <div className="flex-1 space-y-2">
              <div className="h-3.5 w-1/3 rounded bg-sunken" />
              <div className="h-3 w-1/5 rounded bg-sunken" />
            </div>
            <div className="h-5 w-16 rounded-full bg-sunken" />
          </div>
        ))}
      </div>
    </div>
  );
}

const avatarTones = ['bg-info/12 text-info', 'bg-ok/12 text-ok', 'bg-warn/14 text-warn', 'bg-accent-soft text-accent'];

/** A person's initials in a circle, always the same colour for the same name. */
export function Avatar({ name, size = 'md' }: { name: string; size?: 'sm' | 'md' }) {
  const initials = name
    .split(/[\s-]+/)
    .filter(w => /\p{L}/u.test(w))
    .slice(0, 2)
    .map(w => w.match(/\p{L}/u)?.[0] ?? '')
    .join('')
    .toUpperCase();
  let hash = 0;
  for (const c of name) hash = (hash * 31 + c.charCodeAt(0)) >>> 0;
  return (
    <span className={`grid shrink-0 place-items-center rounded-full font-semibold ${avatarTones[hash % avatarTones.length]} ${size === 'sm' ? 'size-8 text-[11px]' : 'size-9 text-xs'}`} aria-hidden>
      {initials || '?'}
    </span>
  );
}

/** One of several ways to do something, as a card to pick (a radio); its own settings inside,
 *  live once it is picked. One button applies the picked one (the dialog's). */
export function Choice({ selected, onSelect, title, hint, children }: { selected: boolean; onSelect: () => void; title: ReactNode; hint?: ReactNode; children?: ReactNode }) {
  return (
    <div
      role="radio"
      aria-checked={selected}
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={e => {
        if ((e.key === ' ' || e.key === 'Enter') && e.target === e.currentTarget) {
          e.preventDefault();
          onSelect();
        }
      }}
      className={`cursor-pointer rounded-xl border p-3.5 transition-[border-color,background-color,box-shadow] outline-none focus-visible:ring-2 focus-visible:ring-info/60 ${
        selected ? 'border-accent bg-accent-soft/40 ring-1 ring-accent' : 'border-line bg-card hover:border-line-strong'
      }`}
    >
      <div className="flex items-start gap-3">
        <span className={`mt-0.5 grid size-4 shrink-0 place-items-center rounded-full border-2 transition-colors ${selected ? 'border-accent' : 'border-line-strong'}`} aria-hidden>
          {selected && <span className="size-1.5 rounded-full bg-accent" />}
        </span>
        <div className="min-w-0 flex-1">
          <div className="text-[13px] font-semibold text-ink">{title}</div>
          {hint && <div className="mt-0.5 text-xs text-ink-3">{hint}</div>}
          {children && <div className={`mt-2.5 transition-opacity ${selected ? '' : 'opacity-60'}`}>{children}</div>}
        </div>
      </div>
    </div>
  );
}

/** A box to tick: ticked, not, or some of a list (`mixed`). */
export function Tick({ checked }: { checked: boolean | 'mixed' }) {
  return (
    <span
      aria-hidden
      className={`grid size-[18px] shrink-0 place-items-center rounded-[5px] border-2 transition-colors ${checked ? 'border-accent bg-accent text-on-accent' : 'border-line-strong bg-card'}`}
    >
      {checked === 'mixed' ? <Minus className="size-3" strokeWidth={3.5} /> : checked ? <Check className="size-3" strokeWidth={3.5} /> : null}
    </span>
  );
}

/** One of several things to tick (where a change applies): the whole row is the box. Rows sit in
 *  a bordered list, separated by lines. */
export function CheckRow({ checked, onChange, disabled, children }: { checked: boolean | 'mixed'; onChange: (checked: boolean) => void; disabled?: boolean; children: ReactNode }) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(checked !== true)}
      className="flex w-full items-center gap-3 border-t border-line px-3.5 py-2.5 text-left transition-colors first:border-t-0 hover:bg-hover focus-visible:bg-hover focus-visible:outline-none disabled:cursor-not-allowed disabled:hover:bg-transparent pointer-coarse:py-3.5"
    >
      <span className={disabled ? 'opacity-40' : ''}>
        <Tick checked={checked} />
      </span>
      <span className="flex min-w-0 flex-1 items-center gap-3">{children}</span>
    </button>
  );
}
