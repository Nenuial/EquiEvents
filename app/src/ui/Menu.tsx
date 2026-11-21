// Menus: actions that belong together behind one button (prints, exports, the "more" of a
// screen), so a screen shows its main actions only. Never clipped: they place themselves where
// there is room, keyboard included (↑↓, Enter, Esc).

import type { ComponentType, ReactNode } from 'react';
import { DropdownMenu as M } from 'radix-ui';
import { Check, ChevronRight } from 'lucide-react';

type Icon = ComponentType<{ className?: string }>;

const item =
  'group relative flex min-h-9 cursor-default items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-[13px] text-ink outline-none select-none data-[disabled]:pointer-events-none data-[disabled]:opacity-45 data-[highlighted]:bg-hover pointer-coarse:min-h-11';

export function Menu({
  trigger,
  children,
  align = 'end',
  side = 'bottom',
  offset = 6,
  width = 'w-64',
}: {
  trigger: ReactNode;
  children: ReactNode;
  align?: 'start' | 'end' | 'center';
  side?: 'top' | 'right' | 'bottom' | 'left';
  /** Room between the button and the menu. */
  offset?: number;
  width?: string;
}) {
  return (
    <M.Root modal={false}>
      <M.Trigger asChild>{trigger}</M.Trigger>
      <M.Portal>
        <M.Content
          align={align}
          side={side}
          sideOffset={offset}
          collisionPadding={10}
          className={`z-[55] max-h-[var(--radix-dropdown-menu-content-available-height)] ${width} max-w-[calc(100vw-20px)] animate-pop-in overflow-auto rounded-xl border border-line bg-card p-1.5 shadow-pop`}
        >
          {children}
        </M.Content>
      </M.Portal>
    </M.Root>
  );
}

export function MenuItem({
  icon: Icon,
  children,
  hint,
  onSelect,
  kbd,
  danger,
  disabled,
}: {
  icon?: Icon;
  children: ReactNode;
  /** A second line, quieter. */
  hint?: ReactNode;
  onSelect: () => void;
  kbd?: string;
  danger?: boolean;
  disabled?: boolean;
}) {
  return (
    <M.Item disabled={disabled} onSelect={onSelect} className={`${item} ${danger ? 'text-danger data-[highlighted]:bg-danger/10' : ''}`}>
      {Icon && <Icon className={`size-4 shrink-0 ${danger ? '' : 'text-ink-3 group-data-[highlighted]:text-ink'}`} />}
      <span className="min-w-0 flex-1">
        <span className="block truncate">{children}</span>
        {hint && <span className="block truncate text-[11px] text-ink-3">{hint}</span>}
      </span>
      {kbd && <kbd className="rounded bg-sunken px-1.5 font-sans text-[10px] font-semibold text-ink-3">{kbd}</kbd>}
    </M.Item>
  );
}

export function MenuCheck({ checked, onChange, children }: { checked: boolean; onChange: (v: boolean) => void; children: ReactNode }) {
  return (
    <M.CheckboxItem checked={checked} onCheckedChange={v => onChange(v === true)} onSelect={e => e.preventDefault()} className={item}>
      <span className="grid size-4 place-items-center">{checked && <Check className="size-4 text-accent" />}</span>
      {children}
    </M.CheckboxItem>
  );
}

export function MenuLabel({ children }: { children: ReactNode }) {
  return <M.Label className="px-2.5 pt-2 pb-1 text-[11px] font-semibold tracking-wide text-ink-4 uppercase">{children}</M.Label>;
}

export function MenuSeparator() {
  return <M.Separator className="my-1.5 h-px bg-line" />;
}

export function SubMenu({ icon: Icon, label, children }: { icon?: Icon; label: ReactNode; children: ReactNode }) {
  return (
    <M.Sub>
      <M.SubTrigger className={`${item} data-[state=open]:bg-hover`}>
        {Icon && <Icon className="size-4 shrink-0 text-ink-3" />}
        <span className="min-w-0 flex-1 truncate">{label}</span>
        <ChevronRight className="size-4 text-ink-4" />
      </M.SubTrigger>
      <M.Portal>
        <M.SubContent sideOffset={6} collisionPadding={10} className="z-[56] w-60 animate-pop-in rounded-xl border border-line bg-card p-1.5 shadow-pop">
          {children}
        </M.SubContent>
      </M.Portal>
    </M.Sub>
  );
}
