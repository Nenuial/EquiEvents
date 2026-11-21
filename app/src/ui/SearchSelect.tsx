// A choice among many (a scoring table, a dressage test): a field showing the one chosen; opened,
// a search over the list, every word typed matching the code or the name (accents and case do
// not count). ↑↓ move, Enter picks, Esc closes. Never clipped: it places itself where there is
// room, a phone included.

import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { Popover as P } from 'radix-ui';
import { Check, ChevronDown, Search } from 'lucide-react';

export interface SearchOption<V> {
  value: V;
  /** What it is called (a test's or table's code). */
  label: string;
  /** Beside it, quieter (its full name). */
  detail?: string;
}

const fold = (s: string) =>
  s
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase();

export function SearchSelect<V extends string | number>({
  value,
  options,
  onChange,
  disabled,
  placeholder = '—',
  search,
  empty,
  id,
}: {
  value: V | null;
  options: SearchOption<V>[];
  onChange: (v: V) => void;
  disabled?: boolean;
  /** Shown when nothing is chosen. */
  placeholder?: string;
  /** The search field's placeholder. */
  search: string;
  /** Said when nothing matches. */
  empty: string;
  id?: string;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const listId = useId();
  const shown = useMemo(() => {
    const words = fold(query).split(/\s+/).filter(Boolean);
    return options.filter(o => words.every(w => fold(`${o.label} ${o.detail ?? ''}`).includes(w)));
  }, [options, query]);
  const chosen = options.find(o => o.value === value);
  // Opened: the search empty, the one chosen marked and in view.
  useEffect(() => {
    if (!open) return;
    setQuery('');
    setActive(
      Math.max(
        0,
        options.findIndex(o => o.value === value),
      ),
    );
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    list.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [active, open, shown]);
  const pick = (o: SearchOption<V>) => {
    if (o.value !== value) onChange(o.value);
    setOpen(false);
  };
  const key = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      setActive(a => Math.min(Math.max(a + (e.key === 'ArrowDown' ? 1 : -1), 0), Math.max(shown.length - 1, 0)));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (shown[active]) pick(shown[active]);
    }
  };
  const optionId = (i: number) => `${listId}-${i}`;
  return (
    <P.Root open={open} onOpenChange={setOpen}>
      <P.Trigger asChild disabled={disabled}>
        <button type="button" id={id} aria-haspopup="listbox" className="field flex items-center gap-2 text-left enabled:cursor-pointer disabled:cursor-not-allowed">
          <span className="min-w-0 flex-1 truncate">
            {chosen ? (
              <>
                {chosen.label}
                {chosen.detail && <span className="text-ink-3"> · {chosen.detail}</span>}
              </>
            ) : (
              <span className="text-ink-4">{placeholder}</span>
            )}
          </span>
          <ChevronDown className="size-4 shrink-0 text-ink-3" aria-hidden />
        </button>
      </P.Trigger>
      <P.Portal>
        <P.Content
          align="start"
          sideOffset={6}
          collisionPadding={10}
          onOpenAutoFocus={e => {
            e.preventDefault();
            input.current?.focus();
          }}
          className="z-[60] flex max-h-[min(26rem,var(--radix-popover-content-available-height))] w-[max(var(--radix-popover-trigger-width),20rem)] max-w-[calc(100vw-20px)] animate-pop-in flex-col overflow-hidden rounded-xl border border-line bg-card shadow-pop"
        >
          <div className="relative border-b border-line p-2">
            <Search className="pointer-events-none absolute top-1/2 left-5 size-4 -translate-y-1/2 text-ink-4" aria-hidden />
            <input
              ref={input}
              value={query}
              onChange={e => {
                setQuery(e.target.value);
                setActive(0);
              }}
              onKeyDown={key}
              placeholder={search}
              role="combobox"
              aria-expanded
              aria-controls={listId}
              aria-activedescendant={shown[active] ? optionId(active) : undefined}
              className="field pl-9"
            />
          </div>
          <div ref={list} id={listId} role="listbox" className="min-h-0 flex-1 overflow-auto p-1.5">
            {shown.map((o, i) => (
              <div
                key={String(o.value)}
                id={optionId(i)}
                role="option"
                aria-selected={o.value === value}
                data-index={i}
                onMouseMove={() => setActive(i)}
                onClick={() => pick(o)}
                className={`flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] pointer-coarse:py-3 ${i === active ? 'bg-hover' : ''}`}
              >
                <span className="min-w-0 flex-1">
                  <span className="block truncate font-semibold">{o.label}</span>
                  {o.detail && <span className="block truncate text-xs text-ink-3">{o.detail}</span>}
                </span>
                {o.value === value && <Check className="size-4 shrink-0 text-accent" aria-hidden />}
              </div>
            ))}
            {shown.length === 0 && <div className="px-2.5 py-3 text-xs text-ink-3">{empty}</div>}
          </div>
        </P.Content>
      </P.Portal>
    </P.Root>
  );
}
