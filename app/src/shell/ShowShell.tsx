// An open show's frame: on a computer a sidebar (the show, its parts, sync, settings) that folds
// to its icons (⌘B, kept), on a phone a bar on top and the main parts at the bottom (U10). The
// part itself fills the rest; on a phone the page scrolls as one, the top bar with it.

import { useEffect, useState, type ComponentType, type ReactNode } from 'react';
import { ArrowLeft, CloudUpload, Ellipsis, Gavel, History, LayoutList, Moon, Monitor, PanelLeftClose, PanelLeftOpen, Printer, Settings, Sun, Users } from 'lucide-react';
import { href, type Place } from '../App';
import { formatDate, useT, type Key, type Language } from '../i18n';
import { LogoMark, Menu, MenuCheck, Modal, Tooltip, overlayOpen, useTheme, type ThemeChoice } from '../ui';
import type { EventView } from '../api';
import { useMedia } from '../parts/common';

/** How the sync state shows: in the sidebar, on a phone's bar, as an icon in the folded sidebar. */
export type SyncLook = 'side' | 'bar' | 'icon';

export type ViewKey = 'classes' | 'participants' | 'officials' | 'prints' | 'sending' | 'history';
type Icon = ComponentType<{ className?: string }>;

export const NAV: { view: ViewKey; icon: Icon; label: Key; kbd: string }[] = [
  { view: 'classes', icon: LayoutList, label: 'classes', kbd: '1' },
  { view: 'participants', icon: Users, label: 'participants', kbd: '2' },
  { view: 'officials', icon: Gavel, label: 'officials', kbd: '3' },
  { view: 'prints', icon: Printer, label: 'prints_tab', kbd: '4' },
  { view: 'sending', icon: CloudUpload, label: 'sending_tab', kbd: '5' },
  { view: 'history', icon: History, label: 'history', kbd: '6' },
];

const THEMES: { value: ThemeChoice; icon: Icon; label: Key }[] = [
  { value: 'system', icon: Monitor, label: 'theme_system' },
  { value: 'light', icon: Sun, label: 'theme_light' },
  { value: 'dark', icon: Moon, label: 'theme_dark' },
];

/** Light, dark or the system's: one click to the next. */
export function ThemeButton({ className = '', side }: { className?: string; side?: 'top' | 'right' }) {
  const t = useT();
  const { choice, choose } = useTheme();
  const i = THEMES.findIndex(x => x.value === choice);
  const now = THEMES[i];
  const next = THEMES[(i + 1) % THEMES.length];
  return (
    <Tooltip label={`${t('theme')} : ${t(now.label)}`} side={side}>
      <button onClick={() => choose(next.value)} aria-label={`${t('theme')} : ${t(now.label)}`} className={`grid size-9 place-items-center rounded-lg transition-colors ${className}`}>
        <now.icon className="size-[18px]" />
      </button>
    </Tooltip>
  );
}

/** The language as one button (the folded sidebar, a phone's bar): FR, DE or IT in a menu. */
export function LanguageButton({
  language,
  setLanguage,
  side = 'right',
  className = 'text-white/60 hover:bg-white/10 hover:text-white',
}: {
  language: Language;
  setLanguage: (l: Language) => void;
  side?: 'right' | 'bottom';
  className?: string;
}) {
  const t = useT();
  const names: Record<Language, string> = { fr: 'Français', de: 'Deutsch', it: 'Italiano' };
  return (
    <Menu
      align={side === 'right' ? 'start' : 'end'}
      side={side}
      offset={side === 'right' ? 22 : 6}
      width="w-44"
      trigger={
        <button aria-label={t('language')} title={t('language')} className={`grid size-9 shrink-0 place-items-center rounded-lg text-[11px] font-bold uppercase ${className}`}>
          {language}
        </button>
      }
    >
      {(['fr', 'de', 'it'] as Language[]).map(l => (
        <MenuCheck key={l} checked={l === language} onChange={() => setLanguage(l)}>
          {names[l]}
        </MenuCheck>
      ))}
    </Menu>
  );
}

/** FR · DE · IT on the dark bar. */
export function LanguagePicker({ language, setLanguage, dark = true }: { language: Language; setLanguage: (l: Language) => void; dark?: boolean }) {
  return (
    <div className={`inline-flex rounded-lg p-0.5 ${dark ? 'bg-white/8' : 'border border-line bg-sunken'}`}>
      {(['fr', 'de', 'it'] as Language[]).map(l => (
        <button
          key={l}
          onClick={() => setLanguage(l)}
          aria-pressed={l === language}
          className={`h-7 rounded-md px-2 text-[11px] font-bold uppercase transition-colors pointer-coarse:h-9 pointer-coarse:px-2.5 ${
            l === language ? (dark ? 'bg-white text-brand' : 'bg-card text-ink shadow-card') : dark ? 'text-white/55 hover:text-white' : 'text-ink-3 hover:text-ink'
          }`}
        >
          {l}
        </button>
      ))}
    </div>
  );
}

export function ShowShell({
  show,
  event,
  place,
  go,
  language,
  setLanguage,
  sync,
  settings,
  extra,
  children,
}: {
  show: string;
  event?: EventView | null;
  place: Place & { show: string };
  go: (p: Place) => void;
  language: Language;
  setLanguage: (l: Language) => void;
  /** The show's state on equievents.ch (a button). */
  sync: (look: SyncLook) => ReactNode;
  /** Opens the settings. */
  settings: () => void;
  /** More in the sidebar's foot (developer mode); `icon` in the folded sidebar. */
  extra?: (icon: boolean) => ReactNode;
  children: ReactNode;
}) {
  const t = useT();
  const [more, setMore] = useState(false);
  // One layout or the other, never both mounted (the sync runs once).
  const wide = useMedia('(min-width: 1024px)');
  const active = (v: ViewKey) => place.view === v || (v === 'classes' && place.view === 'combined');
  const [folded, setFolded] = useState(() => {
    try {
      return localStorage.getItem('sidebar-folded') === '1';
    } catch {
      return false;
    }
  });
  useEffect(() => {
    try {
      localStorage.setItem('sidebar-folded', folded ? '1' : '0');
    } catch {
      /* kept for this session only */
    }
  }, [folded]);

  // ⌘1 … ⌘6: the parts; ⌘B folds or unfolds the sidebar.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey || overlayOpen()) return;
      if (e.key.toLowerCase() === 'b' && !e.shiftKey) {
        e.preventDefault();
        setFolded(f => !f);
        return;
      }
      const item = NAV.find(n => n.kbd === e.key);
      if (!item) return;
      e.preventDefault();
      go({ show, view: item.view });
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [go, show]);

  const title = event ? event.title[0] : show;
  const dates = event ? `${formatDate(event.from, language)}${event.to !== event.from ? ` – ${formatDate(event.to, language)}` : ''}` : '';

  return (
    <div className="flex h-full phone:h-auto phone:min-h-svh">
      {/* The sidebar (a computer), folded to its icons or not. */}
      {wide && (
        <aside className={`relative flex shrink-0 flex-col overflow-hidden bg-nav text-white transition-[width] duration-200 ${folded ? 'w-[68px]' : 'w-[264px]'}`}>
          <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(120%_60%_at_0%_0%,rgb(255_255_255/0.07),transparent_60%)]" aria-hidden />
          {folded ? (
            <>
              <Tooltip label={t('library')} side="right">
                <a href="#/" aria-label={t('library')} className="relative mt-5 mb-3 ml-4 grid size-9 place-items-center text-white/80 hover:text-white">
                  <LogoMark className="size-7" />
                </a>
              </Tooltip>
              <Tooltip label={[event?.number ?? show, title, event ? `${event.place} · ${dates}` : ''].filter(Boolean).join(' · ')} side="right">
                <a
                  href={href({ show, view: 'classes' })}
                  aria-label={title}
                  className="relative mb-4 ml-3 grid size-11 place-items-center rounded-xl border border-white/10 bg-white/[0.04] leading-none hover:bg-white/[0.07]"
                >
                  {event ? (
                    <span className="grid gap-0.5 text-center">
                      <span className="text-[15px] font-bold text-white tabular">{Number(event.from.slice(8, 10))}</span>
                      <span className="text-[9px] font-semibold tracking-wide text-accent-nav uppercase">{month(event.from, language)}</span>
                    </span>
                  ) : (
                    <span className="text-[11px] font-bold text-accent-nav">…</span>
                  )}
                </a>
              </Tooltip>
            </>
          ) : (
            <>
              <div className="relative flex min-w-[264px] items-center pt-5 pr-3 pb-4 pl-5">
                <a href="#/" className="flex min-w-0 flex-1 items-center gap-2.5 text-white/80 hover:text-white" title={t('library')}>
                  <LogoMark className="size-7 shrink-0" />
                  <span className="text-[15px] font-bold tracking-tight whitespace-nowrap">
                    Equi<span className="text-accent-nav">Events</span>
                  </span>
                </a>
              </div>
              <a href={href({ show, view: 'classes' })} className="relative mx-3 mb-4 block min-w-[240px] rounded-xl border border-white/10 bg-white/[0.04] px-3.5 py-3 hover:bg-white/[0.07]">
                <div className="text-[11px] font-semibold tracking-wide text-accent-nav tabular">{event?.number ?? show}</div>
                <div className="mt-0.5 line-clamp-2 text-[13px] leading-snug font-semibold text-white">{title}</div>
                {event && (
                  <div className="mt-1 truncate text-[11px] text-white/55">
                    {event.place} · {dates}
                  </div>
                )}
              </a>
            </>
          )}
          <nav className={`relative flex flex-col gap-0.5 ${folded ? 'items-start px-3.5' : 'px-3'}`}>
            {NAV.map(n =>
              folded ? (
                <Tooltip key={n.view} label={t(n.label)} kbd={`⌘${n.kbd}`} side="right">
                  <a
                    href={href({ show, view: n.view })}
                    aria-label={t(n.label)}
                    aria-current={active(n.view) ? 'page' : undefined}
                    className={`group grid size-10 place-items-center rounded-lg transition-colors ${active(n.view) ? 'bg-white/[0.12]' : 'hover:bg-white/[0.06]'}`}
                  >
                    <n.icon className={`size-[18px] ${active(n.view) ? 'text-accent-nav' : 'text-white/55 group-hover:text-white/85'}`} />
                  </a>
                </Tooltip>
              ) : (
                <a
                  key={n.view}
                  href={href({ show, view: n.view })}
                  aria-current={active(n.view) ? 'page' : undefined}
                  className={`group flex h-10 items-center gap-3 rounded-lg px-3 text-[13px] font-semibold whitespace-nowrap transition-colors ${active(n.view) ? 'bg-white/[0.12] text-white' : 'text-white/65 hover:bg-white/[0.06] hover:text-white'}`}
                >
                  <n.icon className={`size-[18px] shrink-0 ${active(n.view) ? 'text-accent-nav' : 'text-white/45 group-hover:text-white/80'}`} />
                  <span className="flex-1">{t(n.label)}</span>
                  <kbd className="font-sans text-[10px] font-semibold text-white/30 opacity-0 transition-opacity group-hover:opacity-100">⌘{n.kbd}</kbd>
                </a>
              ),
            )}
          </nav>
          <div className="flex-1" />
          {folded ? (
            <div className="relative flex flex-col items-start gap-1.5 border-t border-white/10 px-4 py-4">
              {sync('icon')}
              {extra?.(true)}
              <Tooltip label={t('settings')} side="right">
                <button onClick={settings} aria-label={t('settings')} className="grid size-9 place-items-center rounded-lg text-white/60 hover:bg-white/10 hover:text-white">
                  <Settings className="size-[18px]" />
                </button>
              </Tooltip>
              <ThemeButton side="right" className="text-white/60 hover:bg-white/10 hover:text-white" />
              <LanguageButton language={language} setLanguage={setLanguage} />
              <Tooltip label={t('sidebar_unfold')} kbd="⌘B" side="right">
                <button onClick={() => setFolded(false)} aria-label={t('sidebar_unfold')} className="mt-1 grid size-9 place-items-center rounded-lg text-white/45 hover:bg-white/10 hover:text-white">
                  <PanelLeftOpen className="size-[18px]" />
                </button>
              </Tooltip>
            </div>
          ) : (
            <div className="relative min-w-[264px] space-y-2 border-t border-white/10 px-4 py-4">
              {sync('side')}
              {extra?.(false)}
              <div className="flex items-center gap-1.5">
                <Tooltip label={t('settings')}>
                  <button onClick={settings} aria-label={t('settings')} className="grid size-9 place-items-center rounded-lg text-white/60 hover:bg-white/10 hover:text-white">
                    <Settings className="size-[18px]" />
                  </button>
                </Tooltip>
                <ThemeButton className="text-white/60 hover:bg-white/10 hover:text-white" />
                <div className="flex-1" />
                <LanguagePicker language={language} setLanguage={setLanguage} />
                {/* Folded and unfolded at the same place, the bottom. */}
                <Tooltip label={t('sidebar_fold')} kbd="⌘B">
                  <button
                    onClick={() => setFolded(true)}
                    aria-label={t('sidebar_fold')}
                    className="grid size-9 shrink-0 place-items-center rounded-lg text-white/45 hover:bg-white/10 hover:text-white"
                  >
                    <PanelLeftClose className="size-[18px]" />
                  </button>
                </Tooltip>
              </div>
            </div>
          )}
        </aside>
      )}

      <div className="flex min-w-0 flex-1 flex-col">
        {/* The bar on top (a phone, a narrow window). */}
        {!wide && (
          <header className="flex shrink-0 items-center gap-2 bg-nav px-2 pt-[max(0.5rem,env(safe-area-inset-top))] pb-2 text-white">
            <a href="#/" aria-label={t('library')} className="grid size-10 place-items-center rounded-lg text-white/75 hover:bg-white/10">
              <ArrowLeft className="size-5" />
            </a>
            <div className="min-w-0 flex-1">
              <div className="truncate text-[11px] font-semibold text-accent-nav tabular">{event?.number ?? show}</div>
              <div className="truncate text-[13px] font-semibold">{title}</div>
            </div>
            <div className="flex shrink-0 items-center gap-1.5">{sync('bar')}</div>
          </header>
        )}
        <main className="min-h-0 flex-1 overflow-hidden phone:overflow-visible">{children}</main>
        {/* The main parts at the bottom (a phone: kept there while the page scrolls, unless held
            sideways: the screen is too short to give them a band). */}
        {!wide && (
          <nav className="grid shrink-0 grid-cols-5 border-t border-line bg-card pb-[env(safe-area-inset-bottom)] max-md:sticky max-md:bottom-0 max-md:z-30">
            {NAV.slice(0, 4).map(n => (
              <a
                key={n.view}
                href={href({ show, view: n.view })}
                className={`flex h-14 flex-col items-center justify-center gap-0.5 text-[10px] font-semibold ${active(n.view) ? 'text-accent' : 'text-ink-3'}`}
              >
                <n.icon className="size-5" />
                <span className="max-w-full truncate px-1">{t(n.label)}</span>
              </a>
            ))}
            <button
              onClick={() => setMore(true)}
              className={`flex h-14 flex-col items-center justify-center gap-0.5 text-[10px] font-semibold ${active('sending') || active('history') ? 'text-accent' : 'text-ink-3'}`}
            >
              <Ellipsis className="size-5" />
              {t('more')}
            </button>
          </nav>
        )}
      </div>

      <Modal open={more} onClose={() => setMore(false)} title={event?.number ? `${event.number} · ${title}` : title} size="sm">
        <div className="grid gap-1">
          {NAV.slice(4).map(n => (
            <a key={n.view} href={href({ show, view: n.view })} onClick={() => setMore(false)} className="flex h-12 items-center gap-3 rounded-xl px-3 text-sm font-semibold hover:bg-hover">
              <n.icon className="size-5 text-ink-3" />
              {t(n.label)}
            </a>
          ))}
          <button
            onClick={() => {
              setMore(false);
              settings();
            }}
            className="flex h-12 items-center gap-3 rounded-xl px-3 text-left text-sm font-semibold hover:bg-hover"
          >
            <Settings className="size-5 text-ink-3" />
            {t('settings')}
          </button>
          {extra?.(false)}
          <div className="mt-3 flex items-center justify-between gap-3 border-t border-line pt-4">
            <ThemeButton className="border border-line text-ink-2 hover:bg-hover" />
            <LanguagePicker language={language} setLanguage={setLanguage} dark={false} />
          </div>
        </div>
      </Modal>
    </div>
  );
}

/** The month of a date, short (« nov. », « Nov. »), for the folded sidebar's show tile. */
function month(date: string, language: Language): string {
  const d = new Date(`${date}T12:00:00`);
  return new Intl.DateTimeFormat(language === 'de' ? 'de-CH' : language === 'it' ? 'it-CH' : 'fr-CH', { month: 'short' }).format(d).replace('.', '');
}
