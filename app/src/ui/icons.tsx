// Icons lucide does not have, drawn its way (24×24, the current colour, 2px strokes).

/** A horseshoe: horses. */
export function Horseshoe({ className = 'size-4' }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" className={className} aria-hidden>
      <path d="M4.6 20.5C3.6 18 3 15.2 3 12.5a9 9 0 0 1 18 0c0 2.7-.6 5.5-1.6 8H16c.8-2.5 1-5 1-8a5 5 0 0 0-10 0c0 3 .2 5.5 1 8Z" />
    </svg>
  );
}
