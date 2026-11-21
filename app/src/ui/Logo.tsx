// EquiEvents' mark (chosen by the user, 2026-11-15): a horseshoe on its side with a bar, the E
// of EquiEvents, in white on the accent (`--mark`: the accent chosen in the settings, coral by
// default); the name beside it. The same mark is the favicon (public/favicon.svg) and the
// desktop app's icon (src-tauri/icons, made from src-tauri/icons/mark.svg).

export function LogoMark({ className = 'size-8' }: { className?: string }) {
  return (
    <svg viewBox="0 0 32 32" className={className} aria-hidden>
      <rect width="32" height="32" rx="9" style={{ fill: 'var(--mark)' }} />
      <path d="M22.5 9.5H16a6.5 6.5 0 0 0 0 13h6.5M13 16h7.5" fill="none" stroke="#fff" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function Logo({ dark = false }: { dark?: boolean }) {
  return (
    <span className="inline-flex items-center gap-2.5">
      <LogoMark />
      <span className={`text-[17px] font-bold tracking-tight ${dark ? 'text-white' : 'text-ink'}`}>
        Equi<span className="text-accent">Events</span>
      </span>
    </span>
  );
}
