import { useCallback, useEffect, useState, type ReactNode } from 'react';
import type { Language } from '../i18n';
import { Callout } from '../ui';

/** Load something from the core; reload when `deps` change. */
export function useLoad<T>(load: () => Promise<T>, deps: unknown[]): { data: T | null; error: string | null; reload: () => void } {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [round, setRound] = useState(0);
  useEffect(() => {
    let live = true;
    setError(null);
    load().then(
      d => live && setData(d),
      e => live && setError(String(e?.message ?? e)),
    );
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, round]);
  const reload = useCallback(() => setRound(r => r + 1), []);
  return { data, error, reload };
}

export function ErrorNote({ error }: { error: string | null }) {
  if (!error) return null;
  return (
    <div className="mx-auto max-w-[1400px] px-3 py-3 sm:px-6">
      <Callout tone="danger">{error}</Callout>
    </div>
  );
}

export function Badge({ children, tone = 'plain', title }: { children: ReactNode; tone?: 'plain' | 'ok' | 'warn' | 'accent' | 'muted'; title?: string }) {
  const tones = {
    plain: 'bg-hover text-ink-2',
    ok: 'bg-ok/15 text-ok',
    warn: 'bg-warn/15 text-warn',
    accent: 'bg-accent/15 text-accent-dark',
    muted: 'bg-hover text-ink-3',
  };
  return (
    <span title={title} className={`inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] font-semibold uppercase tracking-wide ${tones[tone]}`}>
      {children}
    </span>
  );
}

export function LanguageSwitch({ language, setLanguage }: { language: Language; setLanguage: (l: Language) => void }) {
  return (
    <div className="flex overflow-hidden rounded-md border border-white/20 text-xs font-semibold">
      {(['fr', 'de', 'it'] as Language[]).map(l => (
        <button key={l} onClick={() => setLanguage(l)} className={`px-2 py-1 uppercase ${l === language ? 'bg-white/20 text-white' : 'text-white/60 hover:text-white'}`}>
          {l}
        </button>
      ))}
    </div>
  );
}

/** Whether a media query matches, following the window. */
export function useMedia(query: string): boolean {
  const [matches, setMatches] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const media = window.matchMedia(query);
    const change = () => setMatches(media.matches);
    media.addEventListener('change', change);
    return () => media.removeEventListener('change', change);
  }, [query]);
  return matches;
}

/** A phone, or a window as narrow (U10): below Tailwind's `md`. */
export function useNarrow(): boolean {
  return useMedia('(max-width: 767px)');
}

/** Two panes side by side, the divider movable; the right one closes. On a phone there is no
 * room for two: the panes are one under the other (`stack`), or the right one takes the screen
 * while it is open (`right`). */
export function Split({ left, right, onCloseRight, narrow: onPhone = 'stack' }: { left: ReactNode; right: ReactNode | null; onCloseRight?: () => void; narrow?: 'stack' | 'right' }) {
  const narrow = useNarrow();
  const [share, setShare] = useState(() => Number(localStorage.getItem('split') ?? 0.5));
  const [dragging, setDragging] = useState(false);
  useEffect(() => {
    if (!dragging) return;
    const move = (e: MouseEvent) => setShare(Math.min(0.8, Math.max(0.2, e.clientX / window.innerWidth)));
    const up = () => {
      setDragging(false);
      localStorage.setItem('split', String(share));
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
    return () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
    };
  }, [dragging, share]);
  useEffect(() => {
    if (!right || !onCloseRight) return;
    const key = (e: KeyboardEvent) => e.key === 'Escape' && !(e.target instanceof HTMLInputElement) && onCloseRight();
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [right, onCloseRight]);
  if (!right) return <div className="h-full overflow-auto">{left}</div>;
  if (narrow)
    return (
      <div className="h-full overflow-auto">
        {onPhone === 'right' ? (
          right
        ) : (
          <>
            {left}
            <div className="border-t-4 border-line">{right}</div>
          </>
        )}
      </div>
    );
  return (
    <div className={`flex h-full ${dragging ? 'select-none' : ''}`}>
      <div className="h-full overflow-auto" style={{ width: `${share * 100}%` }}>
        {left}
      </div>
      <div onMouseDown={() => setDragging(true)} className="w-1.5 shrink-0 cursor-col-resize bg-line hover:bg-accent/60" title="↔" />
      <div className="h-full flex-1 overflow-auto bg-card">{right}</div>
    </div>
  );
}
