// The desktop app's own updates (docs/architecture/releases.md): asked once a moment after the
// library opens, and again from the settings; installed only when the office says so (the app
// starts again), never by itself in the middle of a show.

import { useEffect, useState, useSyncExternalStore } from 'react';
import { checkUpdate, inTauri, installUpdate, type UpdateProgress, type UpdateState } from '../api';
import { useT } from '../i18n';
import { Drawer } from './online';

interface Updates {
  /** The last answer (null: not asked yet). */
  state: UpdateState | null;
  checking: boolean;
  /** The check failed (offline, the server down): why. */
  failed: string | null;
  installing: UpdateProgress | null;
  installFailed: string | null;
}

// One for the whole app: the bar's button and the settings show the same.
let updates: Updates = { state: null, checking: false, failed: null, installing: null, installFailed: null };
const listeners = new Set<() => void>();
const set = (next: Partial<Updates>) => {
  updates = { ...updates, ...next };
  listeners.forEach(l => l());
};
const subscribe = (l: () => void) => {
  listeners.add(l);
  return () => void listeners.delete(l);
};
let asked = false;

async function check() {
  set({ checking: true, failed: null });
  try {
    set({ state: await checkUpdate(), checking: false });
  } catch (e) {
    set({ checking: false, failed: String((e as Error)?.message ?? e) });
  }
}

async function install() {
  set({ installing: {}, installFailed: null });
  try {
    await installUpdate(p => set({ installing: { ...updates.installing, ...p } }));
  } catch (e) {
    set({ installing: null, installFailed: String((e as Error)?.message ?? e) });
  }
}

function useUpdates(): Updates {
  return useSyncExternalStore(subscribe, () => updates);
}

/** In the library's bar: a newer version, when there is one (asked once per start of the app). */
export function UpdateButton() {
  const t = useT();
  const { state } = useUpdates();
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!inTauri || asked) return;
    // After the library has loaded; offline, quietly nothing.
    const timer = setTimeout(() => {
      asked = true;
      void check();
    }, 3000);
    return () => clearTimeout(timer);
  }, []);
  if (!state?.version) return null;
  return (
    <>
      <button onClick={() => setOpen(true)} className="min-h-8 rounded-md bg-white px-2.5 py-1 text-xs font-semibold text-brand hover:bg-white/90">
        <span className="hidden sm:inline">{t('update_new')} </span>
        {state.version}
      </button>
      {open && (
        <Drawer title={t('updates')} close={() => setOpen(false)}>
          <UpdatePanel />
        </Drawer>
      )}
    </>
  );
}

/** The version, the check, the install (the settings; the bar's button). Desktop only. */
export function UpdatePanel() {
  const t = useT();
  const { state, checking, failed, installing, installFailed } = useUpdates();
  useEffect(() => {
    if (inTauri && !updates.state && !updates.checking) void check();
  }, []);
  if (!inTauri) return null;
  const percent = installing?.done && installing.total ? Math.min(100, Math.round((installing.done / installing.total) * 100)) : null;
  return (
    <div className="grid gap-2 text-sm">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="font-semibold">EquiEvents {state?.current}</span>
        {state?.on !== false && !installing && (
          <button onClick={() => void check()} disabled={checking} className="min-h-8 btn btn-sm btn-secondary disabled:opacity-50">
            {checking ? t('update_checking') : t('update_check')}
          </button>
        )}
      </div>
      {state && !state.on && <p className="text-xs text-ink-3">{t('update_off')}</p>}
      {state?.on && !state.version && !checking && !failed && <p className="text-xs text-ok">{t('update_latest')}</p>}
      {failed && (
        <p className="text-xs text-warn" title={failed}>
          {t('update_check_failed')}
        </p>
      )}
      {state?.version && (
        <div className="grid gap-2 rounded-lg border border-line bg-page p-3">
          <div className="font-semibold">
            {t('update_new')} {state.version}
          </div>
          <p className="text-xs text-ink-3">{t('update_install_hint')}</p>
          {installing ? (
            <div className="grid gap-1" role="status">
              <div className="text-xs">{installing.installing ? t('update_installing') : `${t('update_downloading')}${percent !== null ? ` ${percent}\u00a0%` : ''}`}</div>
              <div className="h-1.5 overflow-hidden rounded bg-line">
                <div className="h-full bg-accent transition-all" style={{ width: `${installing.installing ? 100 : (percent ?? 5)}%` }} />
              </div>
            </div>
          ) : (
            <button onClick={() => void install()} className="min-h-9 justify-self-start rounded-lg bg-accent px-4 py-2 font-semibold text-on-accent hover:bg-accent-dark">
              {t('update_install')}
            </button>
          )}
          {installFailed && (
            <p className="text-xs font-semibold text-accent-dark" title={installFailed}>
              {t('update_install_failed')}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
