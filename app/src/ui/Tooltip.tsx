// A short label on hover or keyboard focus (never the only way to learn what a button does on a
// touch screen: icon buttons there also say it to screen readers).

import type { ReactElement } from 'react';
import { Tooltip as T } from 'radix-ui';

export function TooltipProvider({ children }: { children: ReactElement }) {
  return (
    <T.Provider delayDuration={350} skipDelayDuration={200}>
      {children}
    </T.Provider>
  );
}

export function Tooltip({ label, kbd, side = 'top', children }: { label: string; kbd?: string; side?: 'top' | 'bottom' | 'left' | 'right'; children: ReactElement }) {
  return (
    <T.Root>
      <T.Trigger asChild>{children}</T.Trigger>
      <T.Portal>
        <T.Content
          side={side}
          sideOffset={6}
          collisionPadding={8}
          className="z-[70] flex animate-pop-in items-center gap-2 rounded-md bg-[#0b2234] px-2 py-1 text-xs font-medium text-white shadow-lift"
        >
          {label}
          {kbd && <kbd className="rounded bg-white/15 px-1 font-sans text-[10px] font-semibold">{kbd}</kbd>}
        </T.Content>
      </T.Portal>
    </T.Root>
  );
}
