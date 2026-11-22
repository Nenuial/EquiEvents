// The shows on this computer, as cards; a show from Swiss Equestrian (with the event's key,
// through equievents.ch), importing (a Rosson export, an EquiEvents file) and exporting; the
// account, the settings. The first screen of the app. The archived shows have a
// page of their own (#/archives), by year: archived is only how they are listed.

import { useState } from 'react';
import {
  Archive,
  ArchiveRestore,
  ArrowLeft,
  CalendarDays,
  CloudDownload,
  Download,
  Ellipsis,
  FileDown,
  FileInput,
  FolderOpen,
  HardDrive,
  Import,
  MapPin,
  Plus,
  Trash2,
  Trophy,
  UserPlus,
  Users,
} from 'lucide-react';
import { call, chooseFile, saveFile, type EventInFile, type ShowSummary } from '../api';
import { formatDate, usePlural, useT, type Language } from '../i18n';
import { ErrorNote, useLoad } from '../parts/common';
import {
  AccountButton,
  DeleteShow,
  ImportDialog,
  OnlineShows,
  OpenWithKey,
  RequestsDialog,
  importOnline,
  imports,
  useLibrarySync,
  useRefusal,
  type AccountView,
  type ImportAnswer,
  type ImportFile,
  type RemoteShow,
} from '../parts/online';
import { SettingsButton } from '../parts/settings';
import { downloadPage } from '../site';
import { UpdateButton } from '../parts/update';
import { LanguageButton, LanguagePicker, ThemeButton } from '../shell/ShowShell';
import { Button, Card, CheckRow, EmptyState, IconButton, Logo, Menu, MenuItem, MenuSeparator, Modal, Pill, useToast } from '../ui';

export default function Library({
  open,
  archives,
  language,
  setLanguage,
  accountChanged,
  staff = false,
}: {
  open: (show: string) => void;
  archives: boolean;
  language: Language;
  setLanguage: (l: Language) => void;
  /** Signed out (or in as someone else): the app checks again (nothing without an account). */
  accountChanged: () => void;
  /** Staff of equievents.ch (the online version): « Comptes ». */
  staff?: boolean;
}) {
  const t = useT();
  const plural = usePlural();
  const refusal = useRefusal();
  const toast = useToast();
  const shows = useLoad(() => call<ShowSummary[]>('library'), []);
  // equievents.ch: the account (the bar), shows waiting there, the sync while the library is open.
  const [online, setOnline] = useState(0);
  const account = useLoad(() => call<AccountView>('account'), [online]);
  const signedIn = !!account.data?.signed_in;
  // The account's shows on equievents.ch: those not here yet (the desktop app), the requests
  // waiting for their owner (a counter on each card and in the band: the user, 2026-11-22).
  const remote = useLoad(() => (signedIn ? call<{ ok: true; shows: RemoteShow[] } | { ok: false }>('online_shows').catch(() => null) : Promise.resolve(null)), [online, signedIn]);
  const remoteShows = remote.data?.ok ? remote.data.shows : [];
  const waitingFor = (s: ShowSummary) => remoteShows.find(r => r.local === s.id && r.role === 'owner')?.requests ?? 0;
  const waitingAll = remoteShows.filter(r => r.role === 'owner').reduce((n, r) => n + r.requests, 0);
  const [answering, setAnswering] = useState<ShowSummary | null>(null);
  useLibrarySync(signedIn, () => {
    shows.reload();
    remote.reload();
  });
  const [error, setError] = useState<string | null>(null);
  const [picking, setPicking] = useState<{ path: string; events: EventInFile[]; chosen: string[] } | null>(null);
  const [deleting, setDeleting] = useState<ShowSummary | null>(null);
  const [withKey, setWithKey] = useState(false);
  const [busy, setBusy] = useState(false);

  const run = async (f: () => Promise<void>) => {
    setError(null);
    setBusy(true);
    try {
      await f();
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
    } finally {
      setBusy(false);
    }
  };

  // An event already on this computer (by its number).
  const here = (e: EventInFile) => !!shows.data?.some(s => s.event?.number === e.number);

  // A Rosson export (the real show, as it ended, with all its data and the event's key) or an
  // EquiEvents show file: imported through equievents.ch (the user, 2026-11-22). The first to
  // bring an event owns it, past events too; one already works on: equievents.ch's show, not the
  // file; someone else's: never imported, only asked for; without equievents.ch: no import.
  const [refused, setRefused] = useState<{ file: ImportFile; answer: ImportAnswer }[] | null>(null);
  const bring = async (files: ImportFile[]) => {
    toast.show({ text: t('import_online_busy'), tone: 'busy' });
    const answers = [];
    for (const file of files) answers.push({ file, answer: await importOnline(file) });
    toast.clear();
    setOnline(v => v + 1);
    shows.reload();
    const came = answers.filter(a => imports(a.answer.outcome));
    if (came.length < answers.length) {
      if (came.length) toast.show({ text: t('import_online_some').replace('{n}', String(came.length)), tone: 'info' });
      return setRefused(answers.filter(a => !imports(a.answer.outcome)));
    }
    if (answers.length > 1) return toast.show({ text: t('import_online_some').replace('{n}', String(came.length)) });
    const { outcome, show } = answers[0].answer;
    if (outcome === 'made') toast.show({ text: t('import_online_made') });
    if (outcome === 'member') toast.show({ text: t('import_online_member') });
    open(show!);
  };

  const importRosson = () =>
    run(async () => {
      const path = await chooseFile(['txt'], t('rosson_file'));
      if (!path) return;
      const events = await call<EventInFile[]>('rosson_events', { path });
      if (events.length === 1) await bring([{ path, event: events[0].id }]);
      else setPicking({ path, events, chosen: events.filter(e => !here(e)).map(e => e.id) });
    });

  const importChosen = () =>
    run(async () => {
      if (!picking) return;
      const files = picking.chosen.map(event => ({ path: picking.path, event }));
      setPicking(null);
      await bring(files);
    });

  const importShow = () =>
    run(async () => {
      const path = await chooseFile(['equievents'], t('show_file'));
      if (!path) return;
      await bring([{ path }]);
    });

  const exportShow = (s: ShowSummary, withPrivate: boolean) =>
    run(async () => {
      await saveFile(`${s.id}.equievents`, 'equievents', path => call('export_show', { show: s.id, path, private: withPrivate }));
    });

  // Archived for the whole team when on equievents.ch (its responsable only), here otherwise.
  const archive = (s: ShowSummary, archived: boolean) =>
    run(async () => {
      const answer = await call<{ ok: boolean; code?: string }>('archive_show', { show: s.id, archived });
      if (!answer.ok) throw new Error(refusal(answer.code ?? 'error'));
      shows.reload();
    });

  // In Rosson's format: a copy that also opens in Rosson 3.3.15.
  const exportRosson = (s: ShowSummary) =>
    run(async () => {
      const name = await call<string>('rosson_file_name', { show: s.id });
      await saveFile(name, 'txt', path => call('export_rosson', { show: s.id, path, private: true }));
    });

  // A show, as a card (both pages).
  const card = (s: ShowSummary) => (
    <Card key={s.id} className="group flex flex-col overflow-hidden transition-shadow hover:shadow-lift">
      <button onClick={() => open(s.id)} className="flex flex-1 flex-col p-5 text-left outline-none focus-visible:bg-hover">
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="mr-1 text-xs font-bold text-accent tabular">{s.event?.number ?? s.id}</span>
          {s.event?.disciplines.map(d => (
            <Pill key={d}>{d}</Pill>
          ))}
          {s.rehearsal && <Pill tone="warn">{t('rehearsal')}</Pill>}
          {s.training && <Pill tone="info">{t('training_pill')}</Pill>}
        </div>
        <div className="mt-3 line-clamp-2 min-h-[2.6em] text-[16px] leading-snug font-semibold tracking-tight text-ink">{s.event?.title[0] ?? s.id}</div>
        <div className="mt-3 grid gap-1 text-[13px] text-ink-3">
          {s.event?.place && (
            <span className="flex min-w-0 items-center gap-2">
              <MapPin className="size-3.5 shrink-0 text-ink-4" />
              <span className="truncate">{s.event.place}</span>
            </span>
          )}
          {s.event && (
            <span className="flex min-w-0 items-center gap-2">
              <CalendarDays className="size-3.5 shrink-0 text-ink-4" />
              <span className="truncate">
                {formatDate(s.event.from, language)}
                {s.event.to !== s.event.from && ` – ${formatDate(s.event.to, language)}`}
              </span>
            </span>
          )}
          {/* A real event not on equievents.ch: a copy apart (an import from before 0.2.5). */}
          {signedIn && s.event && !s.online && !s.training && !s.rehearsal && (
            <span className="flex min-w-0 items-center gap-2 text-warn">
              <HardDrive className="size-3.5 shrink-0" />
              <span className="truncate">{account.data?.hosted ? t('online_local_hosted') : t('library_local_only')}</span>
            </span>
          )}
        </div>
      </button>
      {/* Requests waiting for the owner: answered from here. */}
      {waitingFor(s) > 0 && (
        <button
          onClick={() => setAnswering(s)}
          className="flex items-center gap-2 border-t border-accent/25 bg-accent-soft/50 px-4 py-2.5 text-left text-[13px] font-semibold text-accent hover:bg-accent-soft pointer-coarse:py-3"
        >
          <UserPlus className="size-4 shrink-0" aria-hidden />
          <span className="flex-1">{waitingFor(s) === 1 ? t('online_requests_waiting_one') : t('online_requests_waiting').replace('{n}', String(waitingFor(s)))}</span>
          <span className="rounded-full bg-accent px-2 py-0.5 text-[11px] text-on-accent">{t('online_requests_answer')}</span>
        </button>
      )}
      <div className="flex items-center gap-3 border-t border-line bg-raised px-4 py-2.5">
        <div className="flex min-w-0 flex-1 flex-wrap gap-x-4 gap-y-0.5 text-xs whitespace-nowrap text-ink-3 tabular">
          <span>
            <span className="font-semibold text-ink">{s.entries}</span> {plural(s.entries, 'entries')}
          </span>
          <span>
            <span className="font-semibold text-ink">{s.classes}</span> {plural(s.classes, 'classes_selected')}
          </span>
          <span className="hidden sm:inline">
            <span className="font-semibold text-ink">{s.results}</span> {plural(s.results, 'results')}
          </span>
        </div>
        <Menu trigger={<IconButton icon={Ellipsis} label={t('actions_more')} size="sm" />}>
          <MenuItem icon={FileDown} onSelect={() => exportShow(s, true)} hint={t('export_private')}>
            {t('export')}
          </MenuItem>
          <MenuItem icon={FileInput} onSelect={() => exportRosson(s)} hint={t('export_rosson_hint')}>
            {t('export_rosson')}
          </MenuItem>
          <MenuSeparator />
          {!s.online || s.role === 'owner' ? (
            <MenuItem icon={s.archived ? ArchiveRestore : Archive} onSelect={() => archive(s, !s.archived)} hint={s.archived ? undefined : s.online ? t('archive_hint_team') : t('archive_hint_here')}>
              {s.archived ? t('unarchive') : t('archive')}
            </MenuItem>
          ) : (
            <MenuItem icon={s.archived ? ArchiveRestore : Archive} disabled onSelect={() => {}} hint={t('archive_owner_only')}>
              {s.archived ? t('unarchive') : t('archive')}
            </MenuItem>
          )}
          <MenuItem icon={Trash2} danger onSelect={() => setDeleting(s)}>
            {t('delete')}…
          </MenuItem>
        </Menu>
        <Button size="sm" onClick={() => open(s.id)}>
          {t('open')}
        </Button>
      </div>
    </Card>
  );
  const current = shows.data?.filter(s => !s.archived) ?? [];
  const archived = shows.data?.filter(s => s.archived) ?? [];
  // The archives by year, the latest first.
  const years = [...new Set(archived.map(s => s.event?.from.slice(0, 4) ?? ''))].sort().reverse();

  const importMenu = (
    <Menu
      trigger={
        <Button variant="primary" icon={Plus} loading={busy}>
          {t('import')}
        </Button>
      }
    >
      <MenuItem icon={CloudDownload} onSelect={() => setWithKey(true)} hint={t('open_key_hint')}>
        {t('open_key')}
      </MenuItem>
      <MenuSeparator />
      <MenuItem icon={Import} onSelect={importRosson} hint={t('import_rosson_hint')}>
        {t('import_rosson')}
      </MenuItem>
      <MenuItem icon={FolderOpen} onSelect={importShow} hint={t('import_show_hint')}>
        {t('import_show')}
      </MenuItem>
    </Menu>
  );

  return (
    <div className="h-full overflow-auto">
      {/* The band on top: the app, the account, the settings; the screen's title and action. */}
      <div className={`relative overflow-hidden bg-nav text-white ${archives ? 'pb-8' : 'pb-20'}`}>
        <div
          className="pointer-events-none absolute inset-0 bg-[radial-gradient(80%_120%_at_100%_0%,rgb(242_90_93/0.22),transparent_55%),radial-gradient(60%_90%_at_0%_100%,rgb(31_111_196/0.18),transparent_60%)]"
          aria-hidden
        />
        <div className="relative mx-auto flex max-w-6xl items-center gap-2 px-4 pt-[max(1rem,env(safe-area-inset-top))] pb-2 sm:px-6">
          <Logo dark compact />
          <div className="flex-1" />
          <UpdateButton />
          {/* The online version: the desktop apps, beside the account (the user, 2026-11-21); not on
              a phone or a tablet, which cannot run them (the user, 2026-11-22). */}
          {account.data?.hosted && (
            <a
              href={downloadPage(language)}
              aria-label={t('online_download_app')}
              title={t('online_download_app')}
              className="inline-flex h-9 shrink-0 items-center gap-2 rounded-lg bg-white px-3 text-[13px] font-semibold text-nav shadow-xs hover:bg-white/90 max-sm:hidden pointer-coarse:hidden"
            >
              <Download className="size-4 shrink-0" aria-hidden />
              {t('download_app_short')}
            </a>
          )}
          <AccountButton
            onChange={() => {
              setOnline(v => v + 1);
              accountChanged();
            }}
          />
          <SettingsButton language={language} setLanguage={setLanguage} className="shrink-0 text-white/70 hover:bg-white/10 hover:text-white" />
          {/* A phone: all of them still there, smaller (the user, 2026-11-22: they disappeared). */}
          <ThemeButton className="shrink-0 text-white/70 hover:bg-white/10 hover:text-white" />
          <span className="hidden sm:inline-flex">
            <LanguagePicker language={language} setLanguage={setLanguage} />
          </span>
          <span className="sm:hidden">
            <LanguageButton language={language} setLanguage={setLanguage} side="bottom" className="text-white/70 hover:bg-white/10 hover:text-white" />
          </span>
        </div>
        <div className="relative mx-auto flex max-w-6xl flex-wrap items-end gap-4 px-4 pt-8 sm:px-6 sm:pt-12">
          {/* Room for the title; on a phone the actions go under it. */}
          <div className="min-w-[14rem] flex-1">
            {archives ? (
              <a href="#/" className="inline-flex items-center gap-1.5 text-[13px] font-semibold text-accent-nav hover:text-white">
                <ArrowLeft className="size-4" />
                {t('library')}
              </a>
            ) : (
              <div className="text-[13px] font-semibold text-accent-nav">{t('library_eyebrow')}</div>
            )}
            <h1 className="mt-1 text-[28px] leading-tight font-bold tracking-tight sm:text-[34px]">{archives ? t('archives') : t('library')}</h1>
            <p className="mt-1 text-sm text-white/60">
              {!shows.data
                ? t('loading')
                : archives
                  ? `${archived.length} ${plural(archived.length, 'archived_count')}`
                  : `${current.length} ${plural(current.length, account.data?.hosted ? 'shows_online' : 'shows_here')}`}
              {!archives && waitingAll > 0 && (
                <span className="ml-2 inline-flex items-center gap-1 rounded-full bg-accent px-2 py-0.5 align-middle text-[11px] font-semibold text-on-accent">
                  <UserPlus className="size-3" aria-hidden />
                  {waitingAll === 1 ? t('online_requests_waiting_one') : t('online_requests_waiting').replace('{n}', String(waitingAll))}
                </span>
              )}
            </p>
          </div>
          {!archives && (
            <div className="flex flex-wrap items-center gap-2">
              {staff && (
                <a
                  href="#/accounts"
                  className="inline-flex h-10 items-center gap-2 rounded-lg border border-white/15 bg-white/[0.06] px-3.5 text-[13px] font-semibold text-white/90 hover:bg-white/[0.12] pointer-coarse:h-11"
                >
                  <Users className="size-4" />
                  {t('staff_accounts')}
                </a>
              )}
              {archived.length > 0 && (
                <a
                  href="#/archives"
                  className="inline-flex h-10 items-center gap-2 rounded-lg border border-white/15 bg-white/[0.06] px-3.5 text-[13px] font-semibold text-white/90 hover:bg-white/[0.12] pointer-coarse:h-11"
                >
                  <Archive className="size-4" />
                  {t('archives')}
                  <span className="rounded-full bg-white/15 px-1.5 text-[11px] tabular">{archived.length}</span>
                </a>
              )}
              {importMenu}
            </div>
          )}
        </div>
      </div>

      <main className={`relative mx-auto max-w-6xl space-y-6 px-4 pb-10 sm:px-6 ${archives ? 'mt-6' : '-mt-12'}`}>
        <ErrorNote error={error ?? shows.error} />
        {!account.data?.hosted && <OnlineShows shows={remoteShows} open={open} archived={archives} />}

        {archives ? (
          shows.data &&
          (archived.length === 0 ? (
            <Card>
              <EmptyState icon={Archive} title={t('archives_empty')}>
                {t('archives_empty_hint')}
              </EmptyState>
            </Card>
          ) : (
            years.map(y => (
              <section key={y} className="space-y-3">
                <h2 className="text-[15px] font-bold tracking-tight text-ink-2 tabular">{y || '—'}</h2>
                <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">{archived.filter(s => (s.event?.from.slice(0, 4) ?? '') === y).map(card)}</div>
              </section>
            ))
          ))
        ) : shows.data && current.length === 0 ? (
          <Card>
            <EmptyState icon={Trophy} title={archived.length ? t('library_all_archived') : account.data?.hosted ? t('library_empty_title_online') : t('library_empty_title')} action={importMenu}>
              {account.data?.hosted ? t('library_empty_online') : t('library_empty')}
            </EmptyState>
          </Card>
        ) : (
          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">{current.map(card)}</div>
        )}
      </main>

      {/* A Rosson export with several events: which to import (those not here yet, ticked). */}
      <Modal
        open={picking !== null}
        onClose={() => setPicking(null)}
        title={t('which_events')}
        description={t('which_events_hint')}
        icon={Import}
        size="lg"
        footer={
          <>
            <Button onClick={() => setPicking(null)}>{t('cancel')}</Button>
            <Button variant="primary" icon={Import} loading={busy} disabled={!picking?.chosen.length} onClick={importChosen}>
              {t('import')}
              {picking && picking.chosen.length > 1 ? ` (${picking.chosen.length})` : ''}
            </Button>
          </>
        }
      >
        <div className="overflow-hidden rounded-xl border border-line">
          {picking?.events.map(e => (
            <CheckRow key={e.id} checked={picking.chosen.includes(e.id)} onChange={on => setPicking(p => p && { ...p, chosen: on ? [...p.chosen, e.id] : p.chosen.filter(x => x !== e.id) })}>
              <span className="min-w-0 flex-1">
                <span className="flex flex-wrap items-baseline gap-x-2">
                  <span className="text-xs font-bold text-accent tabular">{e.number}</span>
                  <span className="truncate text-[13px] font-semibold">{e.title}</span>
                </span>
                <span className="block text-xs text-ink-3">
                  {formatDate(e.from, language)}
                  {e.to !== e.from && ` – ${formatDate(e.to, language)}`} · {e.entries} {plural(e.entries, 'entries')} · {e.results} {plural(e.results, 'results')}
                  {here(e) && ` · ${t('already_here')}`}
                </span>
              </span>
            </CheckRow>
          ))}
        </div>
      </Modal>
      {withKey && <OpenWithKey close={() => setWithKey(false)} open={open} signedIn={() => setOnline(v => v + 1)} />}
      {deleting && (
        <DeleteShow
          show={deleting.id}
          title={deleting.event?.number ?? deleting.id}
          online={deleting.online}
          owner={deleting.role === 'owner'}
          hosted={!!account.data?.hosted}
          me={account.data?.account?.id}
          close={() => setDeleting(null)}
          done={() => {
            setDeleting(null);
            shows.reload();
            setOnline(v => v + 1);
          }}
        />
      )}
      {answering && (
        <RequestsDialog
          show={answering.id}
          name={answering.event?.title[0] ?? answering.id}
          close={() => {
            setAnswering(null);
            remote.reload();
          }}
        />
      )}
      {refused && (
        <ImportDialog
          items={refused}
          openShow={id => {
            setRefused(null);
            open(id);
          }}
          close={() => {
            setRefused(null);
            setOnline(v => v + 1);
            shows.reload();
          }}
        />
      )}
    </div>
  );
}
