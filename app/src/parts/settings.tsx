// Settings of this computer (Rosson's settings screen), in a dialog: the language, the theme, the
// zoom; the version and its updates (desktop); developer mode, the error log;
// and, in developer mode, a show's reference data — a dressage test's or a scoring table's
// definition, as Swiss Equestrian publishes it, changed when it is wrong (checked as the app
// reads them; undone with ⌘Z).

import { useState, type ReactNode } from 'react';
import { Car, Check, CloudDownload, Copy, Download, GraduationCap, Monitor, Moon, Plus, Settings, SlidersHorizontal, Sun, Trophy, Wrench } from 'lucide-react';
import { call } from '../api';
import { formatMillis, useT, type Key, type Language } from '../i18n';
import { LanguagePicker } from '../shell/ShowShell';
import { ACCENTS, Button, IconButton, Modal, Segmented, Switch, Tabs, Tooltip, buttonClass, useTheme } from '../ui';
import { useChanges } from './changes';
import { useLoad } from './common';
import { UpdatePanel } from './update';
import { ZoomControl } from './zoom';

function developer(): boolean {
  try {
    return localStorage.getItem('developer') === '1';
  } catch {
    return false;
  }
}

/** Whether developer mode is on (this computer). */
export function useDeveloper(): boolean {
  const [on] = useState(developer);
  return on;
}

interface LoggedError {
  millis: number;
  command: string;
  show: string | null;
  error: string;
}

type SettingsTab = 'general' | 'training' | 'updates' | 'advanced';

/** The accent of the app on this computer: coral, or one of the others; its name beside the
 *  swatches (a touch screen has no tooltips). */
function AccentPicker() {
  const t = useT();
  const { accent, chooseAccent } = useTheme();
  const name = (a: string) => t(`accent_${a}` as Key);
  return (
    <div className="flex items-center gap-3">
      <span className="text-xs font-semibold text-ink-2">{name(accent)}</span>
      <div role="radiogroup" aria-label={t('accent')} className="flex gap-2">
        {ACCENTS.map(a => (
          <Tooltip key={a.value} label={name(a.value)}>
            <button
              type="button"
              role="radio"
              aria-checked={accent === a.value}
              aria-label={name(a.value)}
              onClick={() => chooseAccent(a.value)}
              style={{ background: a.color }}
              className={`grid size-7 place-items-center rounded-full ring-offset-2 ring-offset-card transition-shadow outline-none focus-visible:ring-2 focus-visible:ring-info pointer-coarse:size-10 ${accent === a.value ? 'ring-2 ring-ink' : 'hover:ring-2 hover:ring-line-strong'}`}
            >
              {accent === a.value && <Check className="size-3.5 text-white" aria-hidden />}
            </button>
          </Tooltip>
        ))}
      </div>
    </div>
  );
}

/** The settings of this computer, in a dialog: language, theme, zoom; updates; developer mode
 *  and the error log. */
export function SettingsModal({ open, onClose, language, setLanguage }: { open: boolean; onClose: () => void; language: Language; setLanguage: (l: Language) => void }) {
  const t = useT();
  const { choice, choose } = useTheme();
  const [tab, setTab] = useState<SettingsTab>('general');
  const [dev, setDev] = useState(developer);
  const [copied, setCopied] = useState(false);
  const errors = useLoad(() => (open && tab === 'advanced' ? call<LoggedError[]>('error_log') : Promise.resolve([])), [open, tab]);
  const toggle = (on: boolean) => {
    setDev(on);
    try {
      localStorage.setItem('developer', on ? '1' : '0');
    } catch {
      /* a preference only */
    }
  };
  const copy = async () => {
    const text = (errors.data ?? []).map(e => `${new Date(e.millis).toISOString()} ${e.command}${e.show ? ` (${e.show})` : ''}: ${e.error}`).join('\n');
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };
  const row = (label: string, hint: string | null, control: ReactNode) => (
    <div className="flex flex-wrap items-center justify-between gap-3 border-t border-line py-4 first:border-t-0 first:pt-0">
      <div className="min-w-0">
        <div className="text-[13px] font-semibold">{label}</div>
        {hint && <div className="text-xs text-ink-3">{hint}</div>}
      </div>
      {control}
    </div>
  );
  return (
    <Modal open={open} onClose={onClose} title={t('settings')} icon={Settings} size="lg" bodyClass="pt-0">
      <div className="sticky top-0 z-10 -mx-5 mb-5 border-b border-line bg-card px-3 sm:-mx-6 sm:px-4">
        <Tabs
          value={tab}
          onChange={setTab}
          compact
          items={[
            { value: 'general', label: t('settings_general'), icon: SlidersHorizontal },
            { value: 'training', label: t('training_tab'), icon: GraduationCap },
            { value: 'updates', label: t('updates'), icon: Download },
            { value: 'advanced', label: t('settings_advanced'), icon: Wrench },
          ]}
        />
      </div>
      {tab === 'general' && (
        <div>
          {row(t('language'), null, <LanguagePicker language={language} setLanguage={setLanguage} dark={false} />)}
          {row(
            t('theme'),
            t('theme_hint'),
            <Segmented
              value={choice}
              onChange={choose}
              options={[
                { value: 'system', label: t('theme_system'), icon: Monitor },
                { value: 'light', label: t('theme_light'), icon: Sun },
                { value: 'dark', label: t('theme_dark'), icon: Moon },
              ]}
            />,
          )}
          {row(t('accent'), t('accent_hint'), <AccentPicker />)}
          {row(t('zoom'), '⌘+ ⌘− ⌘0', <ZoomControl />)}
        </div>
      )}
      {tab === 'training' && <TrainingPanel language={language} done={onClose} />}
      {tab === 'updates' && (
        <div className="grid gap-4">
          <UpdatePanel />
          <p className="text-xs text-ink-3">{t('updates_browser')}</p>
        </div>
      )}
      {tab === 'advanced' && (
        <div className="grid gap-6">
          <Switch checked={dev} onChange={toggle} label={t('developer_mode')} hint={t('developer_mode_hint')} />
          <div className="grid gap-2">
            <div className="flex items-center justify-between gap-2">
              <span className="text-[13px] font-semibold">{t('error_log')}</span>
              {!!errors.data?.length && (
                <Button size="sm" icon={copied ? Check : Copy} onClick={copy}>
                  {t('copy')}
                </Button>
              )}
            </div>
            {errors.data?.length === 0 && <p className="text-xs text-ink-3">{t('error_log_empty')}</p>}
            <ol className="grid gap-2">
              {errors.data?.slice(0, 50).map((e, i) => (
                <li key={i} className="rounded-xl border border-line bg-raised p-3 text-xs">
                  <div className="text-ink-3">
                    {formatMillis(e.millis, language)} · {e.command}
                    {e.show && ` · ${e.show}`}
                  </div>
                  <div className="mt-0.5 break-words text-ink">{e.error}</div>
                </li>
              ))}
            </ol>
          </div>
        </div>
      )}
    </Modal>
  );
}

/** ⚙ (the library): the settings. */
export function SettingsButton({ language, setLanguage, className = '' }: { language: Language; setLanguage: (l: Language) => void; className?: string }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  return (
    <>
      <IconButton icon={Settings} label={t('settings')} onClick={() => setOpen(true)} className={className} />
      <SettingsModal open={open} onClose={() => setOpen(false)} language={language} setLanguage={setLanguage} />
    </>
  );
}

/** Training shows, as Rosson's « Manifestation test »: EquiEvents' practice event, or Swiss
 * Equestrian's demo events (downloaded). They stay on this computer, never synced; made, the show
 * opens. */
function TrainingPanel({ language, done }: { language: Language; done: () => void }) {
  const t = useT();
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const make = async (what: string, command: string, args: Record<string, unknown>) => {
    setBusy(what);
    setError(null);
    try {
      const { show } = await call<{ show: string }>(command, args);
      done();
      window.location.hash = `#/show/${encodeURIComponent(show)}/classes`;
    } catch (e) {
      setError(String((e as Error).message ?? e));
    } finally {
      setBusy(null);
    }
  };
  const choices = [
    {
      what: 'practice',
      icon: Trophy,
      title: t('practice_title'),
      hint: t('practice_hint'),
      action: t('practice_create'),
      actionIcon: Plus,
      run: () => make('practice', 'create_practice', { language }),
    },
    {
      what: 'demo',
      icon: CloudDownload,
      title: t('demo_title'),
      hint: t('demo_hint'),
      action: t('demo_download'),
      actionIcon: CloudDownload,
      run: () => make('demo', 'load_demo', { driving: false }),
    },
    {
      what: 'driving',
      icon: Car,
      title: t('demo_driving_title'),
      hint: t('demo_driving_hint'),
      action: t('demo_download'),
      actionIcon: CloudDownload,
      run: () => make('driving', 'load_demo', { driving: true }),
    },
  ];
  return (
    <div className="grid gap-4">
      <p className="text-[13px] text-ink-2">{t('training_intro')}</p>
      {error && <p className="text-[13px] font-semibold text-danger">{error}</p>}
      <div className="overflow-hidden rounded-xl border border-line">
        {choices.map(c => (
          <div key={c.what} className="flex flex-wrap items-center gap-3 border-t border-line px-4 py-3.5 first:border-t-0">
            <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-sunken text-ink-3">
              <c.icon className="size-[18px]" />
            </span>
            <span className="min-w-0 flex-1 basis-56">
              <span className="block text-[13px] font-semibold text-ink">{c.title}</span>
              <span className="block text-xs leading-snug text-ink-3">{c.hint}</span>
            </span>
            <Button size="sm" icon={c.actionIcon} loading={busy === c.what} disabled={busy !== null && busy !== c.what} onClick={c.run}>
              {c.action}
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}

interface Reference {
  programmes: { id: number; name: string; title: string }[];
  scoring_tables: { id: number; code: string; name: string }[];
}

/** In a show, developer mode: its tests' and scoring tables' definitions (in the sidebar). */
export function ReferenceButton({ show, icon = false }: { show: string; icon?: boolean }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  if (!useDeveloper()) return null;
  return (
    <>
      {icon ? (
        <Tooltip label={t('reference_data')} side="right">
          <button onClick={() => setOpen(true)} aria-label={t('reference_data')} className="grid size-9 place-items-center rounded-lg text-white/60 hover:bg-white/10 hover:text-white">
            <Wrench className="size-4" />
          </button>
        </Tooltip>
      ) : (
        <button
          onClick={() => setOpen(true)}
          className="flex h-9 w-full items-center gap-2.5 rounded-lg px-2.5 text-[13px] font-semibold whitespace-nowrap text-white/60 hover:bg-white/10 hover:text-white"
        >
          <Wrench className="size-4" />
          {t('reference_data')}
        </button>
      )}
      <Modal open={open} onClose={() => setOpen(false)} title={t('reference_data')} icon={Wrench} size="lg">
        {open && <ReferenceEditor show={show} />}
      </Modal>
    </>
  );
}

function ReferenceEditor({ show }: { show: string }) {
  const t = useT();
  const { change } = useChanges();
  const data = useLoad(() => call<Reference>('reference_data', { show }), [show]);
  const [query, setQuery] = useState('');
  const [picked, setPicked] = useState<{ kind: 'programme' | 'scoring_table'; id: number; label: string } | null>(null);
  const [text, setText] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  const open = async (kind: 'programme' | 'scoring_table', id: number, label: string) => {
    setError(null);
    setSaved(false);
    const d = await call<{ definition: string }>('definition', { show, kind, id });
    setPicked({ kind, id, label });
    setText(d.definition);
  };
  const save = async () => {
    if (!picked) return;
    setError(null);
    try {
      JSON.parse(text);
    } catch (e) {
      return setError(String((e as Error).message));
    }
    const answer = await change('set_definition', { kind: picked.kind, id: picked.id, definition: text });
    if (answer) setSaved(true);
  };
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const match = (s: string) => words.every(w => s.toLowerCase().includes(w));

  if (picked)
    return (
      <div className="grid gap-2 text-sm">
        <button onClick={() => setPicked(null)} className="justify-self-start text-xs text-ink-3 hover:text-ink">
          ← {t('reference_data')}
        </button>
        <div className="font-semibold">{picked.label}</div>
        <p className="text-xs text-warn">{t('reference_warning')}</p>
        <textarea value={text} onChange={e => (setText(e.target.value), setSaved(false))} spellCheck={false} className="field h-[55vh] font-mono text-xs" />
        {error && <p className="font-semibold text-danger">{error}</p>}
        <div className="flex items-center gap-2">
          <button onClick={save} className={buttonClass('primary')}>
            {t('save')}
          </button>
          {saved && <span className="text-xs text-ok">✓</span>}
        </div>
      </div>
    );
  return (
    <div className="grid gap-3 text-sm">
      <input value={query} onChange={e => setQuery(e.target.value)} placeholder={t('reference_search')} className="field" />
      <section>
        <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-ink-3">{t('reference_tests')}</h3>
        <div className="grid gap-0.5">
          {data.data?.programmes
            .filter(p => match(`${p.name} ${p.title}`))
            .map(p => (
              <button key={p.id} onClick={() => open('programme', p.id, `${p.name} · ${p.title}`)} className="truncate rounded px-2 py-1 text-left hover:bg-hover">
                <span className="font-semibold">{p.name}</span> <span className="text-ink-3">{p.title}</span>
              </button>
            ))}
        </div>
      </section>
      <section>
        <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-ink-3">{t('reference_tables')}</h3>
        <div className="grid gap-0.5">
          {data.data?.scoring_tables
            .filter(s => match(`${s.code} ${s.name}`))
            .map(s => (
              <button key={s.id} onClick={() => open('scoring_table', s.id, `${s.code} · ${s.name}`)} className="truncate rounded px-2 py-1 text-left hover:bg-hover">
                <span className="font-semibold">{s.code}</span> <span className="text-ink-3">{s.name}</span>
              </button>
            ))}
        </div>
      </section>
    </div>
  );
}
