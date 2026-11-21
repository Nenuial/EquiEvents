// Buttons, one family: a primary action per screen (coral), secondary ones (outlined), quiet
// ones (ghost), and the dangerous ones (red). Large enough for a finger on a touch screen.

import { forwardRef, type ButtonHTMLAttributes, type ComponentType, type ReactNode } from 'react';
import { LoaderCircle } from 'lucide-react';
import { Tooltip } from './Tooltip';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'subtle' | 'danger' | 'danger-solid' | 'danger-ghost' | 'brand';
/** `xs`: tools inside a table or a card (still a finger's size on a touch screen). */
export type ButtonSize = 'xs' | 'sm' | 'md' | 'lg';
type Icon = ComponentType<{ className?: string; 'aria-hidden'?: boolean | 'true' }>;

const base =
  'relative inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-lg font-semibold select-none transition-[background-color,border-color,color,box-shadow,transform] duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-info/60 focus-visible:ring-offset-2 focus-visible:ring-offset-page disabled:pointer-events-none disabled:opacity-45 active:translate-y-px';

const sizes: Record<ButtonSize, string> = {
  xs: 'h-6 px-2 text-[11px] pointer-coarse:h-10 pointer-coarse:px-3',
  sm: 'h-8 px-2.5 text-xs pointer-coarse:h-10 pointer-coarse:px-3.5',
  md: 'h-9 px-3.5 text-[13px] pointer-coarse:h-11 pointer-coarse:px-4',
  lg: 'h-11 px-5 text-sm',
};

const variants: Record<ButtonVariant, string> = {
  primary: 'bg-accent text-on-accent shadow-[inset_0_1px_0_rgb(255_255_255/0.18),0_1px_2px_rgb(var(--shadow-rgb)/0.2)] hover:bg-accent-dark',
  brand: 'bg-brand text-white shadow-[inset_0_1px_0_rgb(255_255_255/0.1),0_1px_2px_rgb(var(--shadow-rgb)/0.2)] hover:bg-brand-light',
  secondary: 'border border-line-strong bg-card text-ink shadow-xs hover:border-ink-4/60 hover:bg-hover',
  ghost: 'text-ink-2 hover:bg-hover hover:text-ink',
  subtle: 'bg-sunken text-ink hover:bg-line',
  danger: 'border border-danger/35 bg-card text-danger shadow-xs hover:bg-danger/8',
  'danger-solid': 'bg-danger text-white shadow-xs hover:brightness-110',
  'danger-ghost': 'text-ink-3 hover:bg-danger/10 hover:text-danger',
};

const iconSizes: Record<ButtonSize, string> = { xs: 'size-3.5', sm: 'size-3.5', md: 'size-4', lg: 'size-[18px]' };

export function buttonClass(variant: ButtonVariant = 'secondary', size: ButtonSize = 'md', extra = '') {
  return `${base} ${sizes[size]} ${variants[variant]} ${extra}`;
}

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  icon?: Icon;
  iconRight?: Icon;
  loading?: boolean;
  /** A key to press for it, shown at its end (`⌘K`, `E`). */
  kbd?: string;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = 'secondary', size = 'md', icon: Icon, iconRight: IconRight, loading, kbd, className = '', children, type = 'button', ...rest },
  ref,
) {
  return (
    <button ref={ref} type={type} className={buttonClass(variant, size, className)} {...rest}>
      {loading ? <LoaderCircle className={`${iconSizes[size]} animate-spin`} aria-hidden /> : Icon && <Icon className={iconSizes[size]} aria-hidden />}
      {children}
      {IconRight && <IconRight className={`${iconSizes[size]} opacity-70`} aria-hidden />}
      {kbd && (
        <kbd
          className={`ml-0.5 hidden rounded px-1 font-sans text-[10px] font-semibold md:inline ${variant === 'primary' || variant === 'brand' || variant === 'danger-solid' ? 'bg-white/20' : 'bg-sunken text-ink-3'}`}
        >
          {kbd}
        </kbd>
      )}
    </button>
  );
});

const squares: Record<ButtonSize, string> = { xs: 'size-6 pointer-coarse:size-10', sm: 'size-8 pointer-coarse:size-10', md: 'size-9 pointer-coarse:size-11', lg: 'size-11' };

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon: Icon;
  /** Said by screen readers and shown as a tooltip. */
  label: string;
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** The tooltip's key hint. */
  kbd?: string;
  /** No tooltip (inside a tooltip trigger already). */
  bare?: boolean;
  active?: boolean;
  /** A count on the icon (what waits there). */
  badge?: number;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { icon: Icon, label, variant = 'ghost', size = 'md', kbd, bare, active, badge, className = '', type = 'button', ...rest },
  ref,
) {
  const button = (
    <button ref={ref} type={type} aria-label={label} className={`${base} ${squares[size]} ${variants[variant]} ${active ? 'bg-hover text-ink' : ''} ${className}`} {...rest}>
      <Icon className={iconSizes[size]} aria-hidden />
      {badge !== undefined && (
        <span className="absolute -top-1.5 -right-1.5 grid h-4 min-w-4 place-items-center rounded-full bg-warn px-1 text-[10px] leading-none font-bold text-white tabular ring-2 ring-card">
          {badge}
        </span>
      )}
    </button>
  );
  return bare ? (
    button
  ) : (
    <Tooltip label={label} kbd={kbd}>
      {button}
    </Tooltip>
  );
});

/** Buttons side by side as one control (a choice among a few). */
export function Segmented<T extends string>({
  value,
  onChange,
  options,
  size = 'md',
  className = '',
}: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: ReactNode; title?: string; icon?: Icon }[];
  size?: 'sm' | 'md';
  className?: string;
}) {
  return (
    <div role="radiogroup" className={`inline-flex rounded-lg border border-line bg-sunken p-0.5 ${className}`}>
      {options.map(o => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={value === o.value}
          title={o.title}
          onClick={() => onChange(o.value)}
          className={`inline-flex items-center justify-center gap-1.5 rounded-md font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-info/60 ${
            size === 'sm' ? 'h-7 px-2.5 text-xs pointer-coarse:h-9' : 'h-8 px-3 text-[13px] pointer-coarse:h-10'
          } ${value === o.value ? 'bg-card text-ink shadow-card' : 'text-ink-3 hover:text-ink'}`}
        >
          {o.icon && <o.icon className="size-3.5" aria-hidden />}
          {o.label}
        </button>
      ))}
    </div>
  );
}
