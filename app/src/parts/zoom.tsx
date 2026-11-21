// The app's size on the screen (Rosson's zoom, 50–150 %): ⌘+ / ⌘− / ⌘0 anywhere, or the control
// in the library's bar; kept on this computer.

import { useCallback, useEffect, useState } from 'react';

const STEPS = [0.5, 0.6, 0.7, 0.8, 0.9, 1, 1.1, 1.2, 1.3, 1.4, 1.5];

function stored(): number {
  try {
    const z = Number(localStorage.getItem('zoom'));
    return STEPS.includes(z) ? z : 1;
  } catch {
    return 1;
  }
}

function apply(z: number) {
  document.documentElement.style.setProperty('zoom', String(z));
  try {
    localStorage.setItem('zoom', String(z));
  } catch {
    /* a preference only */
  }
  window.dispatchEvent(new CustomEvent('zoom', { detail: z }));
}

/** The zoom, and steps up and down. */
export function useZoom() {
  const [zoom, setZoom] = useState(stored);
  useEffect(() => {
    const changed = (e: Event) => setZoom((e as CustomEvent<number>).detail);
    window.addEventListener('zoom', changed);
    return () => window.removeEventListener('zoom', changed);
  }, []);
  const step = useCallback((by: number) => {
    const i = STEPS.indexOf(stored());
    apply(by === 0 ? 1 : STEPS[Math.min(STEPS.length - 1, Math.max(0, (i < 0 ? 5 : i) + by))]);
  }, []);
  return { zoom, step };
}

/** ⌘+ / ⌘− / ⌘0, and the zoom kept from last time. */
export function useZoomKeys() {
  const { step } = useZoom();
  useEffect(() => {
    apply(stored());
    const key = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      if (e.key === '+' || e.key === '=') step(1);
      else if (e.key === '-') step(-1);
      else if (e.key === '0') step(0);
      else return;
      e.preventDefault();
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [step]);
}

/** − 100 % + (the settings). */
export function ZoomControl() {
  const { zoom, step } = useZoom();
  const button = 'grid h-8 w-8 place-items-center rounded-md text-ink-2 hover:bg-card hover:text-ink disabled:opacity-30 pointer-coarse:h-10 pointer-coarse:w-10';
  return (
    <div className="inline-flex items-center rounded-lg border border-line bg-sunken p-0.5 text-[13px] font-semibold" title="⌘+ ⌘− ⌘0">
      <button onClick={() => step(-1)} disabled={zoom <= STEPS[0]} className={button} aria-label="−">
        −
      </button>
      <button onClick={() => step(0)} className="min-w-14 px-1 text-ink tabular">
        {Math.round(zoom * 100)} %
      </button>
      <button onClick={() => step(1)} disabled={zoom >= STEPS[STEPS.length - 1]} className={button} aria-label="+">
        +
      </button>
    </div>
  );
}
