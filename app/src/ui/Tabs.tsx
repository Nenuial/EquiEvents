// Tabs: one screen's parts, one at a time (a class: its start list, its results, its ranking,
// its settings), each with a key to press. Arrows move between them.

import type { ComponentType, ReactNode } from 'react';
import { Tabs as T } from 'radix-ui';

type Icon = ComponentType<{ className?: string }>;

export interface TabItem<V extends string> {
  value: V;
  label: string;
  icon?: Icon;
  /** A number beside the label (how many). */
  count?: number | null;
  /** The key that opens it, shown on hover. */
  kbd?: string;
  /** Something needs attention there. */
  dot?: boolean;
}

export function Tabs<V extends string>({ value, onChange, items, className = '', children }: { value: V; onChange: (v: V) => void; items: TabItem<V>[]; className?: string; children?: ReactNode }) {
  return (
    <T.Root value={value} onValueChange={v => onChange(v as V)} activationMode="manual">
      {/* Sideways only, when the tabs are wider than the screen (a phone), with no scrollbar:
          never up and down. */}
      <T.List className={`flex min-w-0 gap-1 overflow-x-auto overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden ${className}`}>
        {items.map(i => (
          <T.Trigger
            key={i.value}
            value={i.value}
            title={i.kbd ? `${i.label} (${i.kbd})` : i.label}
            className="group relative inline-flex h-11 shrink-0 items-center gap-2 rounded-t-lg px-3 text-[13px] font-semibold text-ink-3 outline-none transition-colors hover:text-ink focus-visible:ring-2 focus-visible:ring-info/50 focus-visible:ring-inset data-[state=active]:text-ink pointer-coarse:h-12"
          >
            {i.icon && <i.icon className="size-4 text-ink-4 group-data-[state=active]:text-accent" />}
            {i.label}
            {i.count != null && (
              <span className="rounded-full bg-sunken px-1.5 py-px text-[11px] font-semibold text-ink-3 tabular group-data-[state=active]:bg-accent-soft group-data-[state=active]:text-accent">
                {i.count}
              </span>
            )}
            {i.dot && <span className="size-1.5 rounded-full bg-warn" aria-hidden />}
            <span className="absolute inset-x-2 bottom-0 h-0.5 rounded-full bg-transparent transition-colors group-data-[state=active]:bg-accent" aria-hidden />
          </T.Trigger>
        ))}
      </T.List>
      {children}
    </T.Root>
  );
}

/** A tab's content (inside `Tabs`), mounted only while open. */
export function TabPanel({ value, children, className = '' }: { value: string; children: ReactNode; className?: string }) {
  return (
    <T.Content value={value} className={`outline-none ${className}`}>
      {children}
    </T.Content>
  );
}
