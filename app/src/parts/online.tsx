// equievents.ch in the app (crates/office/src/sync.rs): the account, a show's badge and panel
// (putting it online with its key, asking its owner, the owner's requests and team), the shows
// waiting on the server, an event loaded from Swiss Equestrian with its key, and the sync in the
// background.

import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react';
import { call } from '../api';
import { refusal } from '../refusals';
import { formatMillis, useLanguage, useT, type Key } from '../i18n';
import { useChanges } from './changes';
import { useLoad } from './common';
import { Cloud, CloudDownload, CloudOff, HardDrive, RefreshCw, Trash2, TriangleAlert, UserRound } from 'lucide-react';
import { Button, Callout, Field, Modal, Switch, Tooltip, buttonClass } from '../ui';
import type { SyncLook } from '../shell/ShowShell';
import { ConflictsNotice } from './conflicts';
import { signOutPage } from '../site';

export interface Account {
  server: string;
  id: number;
  name: string;
  email: string;
  language: string;
}
export interface AccountView {
  server: string;
  signed_in: boolean;
  account?: Account;
  /** The online version: signed in and out on equievents.ch's site. */
  hosted?: boolean;
}
interface Note {
  status: 'local' | 'synced' | 'offline' | 'signed_out' | 'no_access' | 'clock' | 'error';
  last_ok: number | null;
  error: string | null;
  clock_ahead: number | null;
  role: 'owner' | 'collaborator' | null;
  owner: string | null;
  requests: number;
  pushed: number;
  pulled: number;
}
export interface SyncStatus {
  /** A training show: it stays on this computer. */
  training: boolean;
  /** The online version (on equievents.ch). */
  hosted?: boolean;
  server: string;
  signed_in: boolean;
  remote: string | null;
  pending: number;
  needs_key: boolean;
  note: Note | null;
}
interface Request {
  id: number;
  show: string;
  show_name: string;
  kind: 'access' | 'ownership';
  status: 'pending' | 'accepted' | 'refused' | 'withdrawn';
  person: { id: number; name: string; email: string };
  created_at: string;
  decided_at?: string | null;
}
interface Person {
  id: number;
  name: string;
  email: string;
  role: 'owner' | 'collaborator';
}
export interface RemoteShow {
  id: string;
  name: string;
  role: 'owner' | 'collaborator';
  owner: string;
  requests: number;
  local: string | null;
  /** In the archives (its responsable decided, for everyone). */
  archived: boolean;
}
type Refusal = { ok: false; code: string; error?: string };

const input = 'field';
const primary = buttonClass('primary');
const secondary = buttonClass('secondary');

/** A refusal in the office's words (`signed_out` signing in: a wrong e-mail or password). */
export function useRefusal() {
  const t = useT();
  return (code: string, signingIn = false): string => {
    if (code === 'signed_out') return signingIn ? t('online_err_wrong') : t('online_signed_out_long');
    if (code === 'no_access') return t('online_no_access_long');
    const known = [
      'offline',
      'too_many',
      'taken',
      'email',
      'password',
      'name',
      'key',
      'account',
      'member',
      'owner',
      'answered',
      'need_key',
      'empty',
      'not_owner',
      'training',
      'swiss_equestrian',
      'no_swiss_equestrian',
    ];
    return known.includes(code) ? t(`online_err_${code}` as Key) : `${t('online_err_other')} (${code}).`;
  };
}

/** What used to open as a panel: a dialog (P. Burkhard, 2026-11-14). */
export function Drawer({ title, close, children }: { title: string; close: () => void; children: ReactNode }) {
  return (
    <Modal open onClose={close} title={title} size="md">
      {children}
    </Modal>
  );
}

/** Signing in, making an account, a forgotten password. */
export function AccountForms({ done }: { done: () => void }) {
  const t = useT();
  const language = useLanguage();
  const refusal = useRefusal();
  const [mode, setMode] = useState<'in' | 'up' | 'forgot'>('in');
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [note, setNote] = useState<{ text: string; ok?: boolean } | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setNote(null);
    try {
      if (mode === 'forgot') {
        const answer = await call<{ ok: true } | Refusal>('forgot_password', { email, language });
        setNote(answer.ok ? { text: t('online_link_sent'), ok: true } : { text: refusal(answer.code) });
      } else {
        const answer = await call<{ ok: true } | Refusal>(mode === 'in' ? 'sign_in' : 'sign_up', mode === 'in' ? { email, password } : { name, email, password, language });
        if (answer.ok) done();
        else setNote({ text: refusal(answer.code, mode === 'in') });
      }
    } catch (err) {
      setNote({ text: String((err as Error)?.message ?? err) });
    } finally {
      setBusy(false);
    }
  };

  const tab = (m: typeof mode, label: string) => (
    <button
      type="button"
      onClick={() => {
        setMode(m);
        setNote(null);
      }}
      className={`flex-1 rounded-md px-3 py-1.5 text-sm font-semibold ${mode === m ? 'bg-card shadow-sm' : 'text-ink-3 hover:text-ink'}`}
    >
      {label}
    </button>
  );

  return (
    <form onSubmit={submit} className="grid gap-3">
      <p className="text-sm text-ink-2">{t('online_why')}</p>
      <div className="flex gap-1 rounded-lg bg-hover p-1">
        {tab('in', t('online_sign_in'))}
        {tab('up', t('online_sign_up'))}
      </div>
      {mode === 'up' && (
        <label className="grid gap-1 text-sm font-semibold">
          {t('online_name')}
          <input className={input} value={name} onChange={e => setName(e.target.value)} autoComplete="name" required />
        </label>
      )}
      <label className="grid gap-1 text-sm font-semibold">
        {t('online_email')}
        <input className={input} type="email" value={email} onChange={e => setEmail(e.target.value)} autoComplete="email" required />
      </label>
      {mode !== 'forgot' && (
        <label className="grid gap-1 text-sm font-semibold">
          {t('online_password')}
          <input
            className={input}
            type="password"
            value={password}
            onChange={e => setPassword(e.target.value)}
            autoComplete={mode === 'up' ? 'new-password' : 'current-password'}
            minLength={mode === 'up' ? 10 : undefined}
            required
          />
          {mode === 'up' && <span className="text-xs font-normal text-ink-3">{t('online_password_hint')}</span>}
        </label>
      )}
      {note && <p className={`text-sm ${note.ok ? 'text-ok' : 'font-semibold text-danger'}`}>{note.text}</p>}
      <button disabled={busy} className={primary}>
        {mode === 'in' ? t('online_sign_in') : mode === 'up' ? t('online_sign_up') : t('online_send_link')}
      </button>
      {mode === 'in' && (
        <button type="button" onClick={() => setMode('forgot')} className="justify-self-start text-sm text-ink-3 hover:text-ink">
          {t('online_forgot')}
        </button>
      )}
    </form>
  );
}

/** The account, in the library's bar: who is signed in, or signing in. */
export function AccountButton({ onChange }: { onChange: () => void }) {
  const t = useT();
  const language = useLanguage();
  const [open, setOpen] = useState(false);
  const account = useLoad(() => call<AccountView>('account'), [open]);
  const signedIn = account.data?.signed_in && account.data.account;
  return (
    <>
      {/* A phone: the initial alone, the name from a small tablet up. */}
      <button
        onClick={() => setOpen(true)}
        aria-label={signedIn ? account.data!.account!.name : t('online_sign_in')}
        className="inline-flex h-9 max-w-52 min-w-0 items-center gap-2 rounded-lg border border-white/15 bg-white/[0.06] px-1.5 text-[13px] font-semibold text-white/90 hover:bg-white/[0.12] sm:px-3"
      >
        {signedIn ? (
          <span className="grid size-6 shrink-0 place-items-center rounded-full bg-accent text-[11px] font-bold text-on-accent">{account.data!.account!.name.slice(0, 1).toUpperCase()}</span>
        ) : (
          <UserRound className="size-4 shrink-0" />
        )}
        <span className="hidden truncate sm:inline">{signedIn ? account.data!.account!.name : t('online_sign_in')}</span>
      </button>
      {open && (
        <Drawer title={t('online_account')} close={() => setOpen(false)}>
          {signedIn ? (
            <div className="grid gap-3 text-sm">
              <div>
                <div className="font-semibold">{account.data!.account!.name}</div>
                <div className="text-ink-2">{account.data!.account!.email}</div>
                {!account.data!.hosted && (
                  <div className="text-xs text-ink-3">
                    {t('online_server')}
                    {t('colon')}
                    {account.data!.server}
                  </div>
                )}
              </div>
              <ArchiveSetting />
              <MyRequests />
              <button
                className={`${secondary} justify-self-start`}
                onClick={async () => {
                  // The online version: signed out on the site.
                  if (account.data!.hosted) {
                    window.location.href = signOutPage(language);
                    return;
                  }
                  await call('sign_out');
                  account.reload();
                  onChange();
                }}
              >
                {t('online_sign_out')}
              </button>
            </div>
          ) : (
            <AccountForms
              done={() => {
                account.reload();
                onChange();
              }}
            />
          )}
        </Drawer>
      )}
    </>
  );
}

/** The account archives the shows it is responsable for on its own, some weeks after their
 * last day (on equievents.ch: for the whole team); off, by hand only. */
function ArchiveSetting() {
  const t = useT();
  const refusal = useRefusal();
  const settings = useLoad(() => call<{ ok: true; account: { archive_after_weeks: number | null } } | Refusal>('account_settings'), []);
  const current = settings.data?.ok ? settings.data.account.archive_after_weeks : null;
  const [weeks, setWeeks] = useState('');
  const [error, setError] = useState<string | null>(null);
  useEffect(() => setWeeks(current ? String(current) : ''), [current]);
  if (!settings.data) return null;
  const save = async (value: number | null) => {
    setError(null);
    const answer = await call<{ ok: true } | Refusal>('set_account_settings', { archive_after_weeks: value });
    if (answer.ok) settings.reload();
    else setError(refusal(answer.code));
  };
  // A whole number of weeks, 1 to 104; anything else goes back to what is set.
  const commit = () => {
    const n = Number(weeks);
    if (Number.isInteger(n) && n >= 1 && n <= 104) {
      if (n !== current) save(n);
    } else setWeeks(current ? String(current) : '');
  };
  return (
    <div className="grid gap-2 border-y border-line py-3">
      {settings.data.ok ? (
        <>
          <Switch checked={current !== null} onChange={on => save(on ? 4 : null)} label={t('archive_auto')} hint={t('archive_auto_hint')} />
          {current !== null && (
            <label className="flex items-center gap-2 text-[13px] text-ink-2">
              <input
                type="number"
                min={1}
                max={104}
                value={weeks}
                onChange={e => setWeeks(e.target.value)}
                onBlur={commit}
                onKeyDown={e => e.key === 'Enter' && commit()}
                className="field field-sm w-20 text-right"
              />
              {t('archive_weeks_after')}
            </label>
          )}
        </>
      ) : (
        <p className="text-xs text-ink-3">
          {t('archive_auto')} · {refusal(settings.data.code)}
        </p>
      )}
      {error && <p className="text-xs font-semibold text-danger">{error}</p>}
    </div>
  );
}

/** Runs `f` every `ms` while mounted (and once now). */
function useEvery(f: () => void, ms: number, active = true) {
  const latest = useRef(f);
  latest.current = f;
  useEffect(() => {
    if (!active) return;
    latest.current();
    const id = setInterval(() => latest.current(), ms);
    return () => clearInterval(id);
  }, [ms, active]);
}

/** The library keeps the shows on equievents.ch in sync while it is open (`synced`: then). */
export function useLibrarySync(signedIn: boolean, synced: () => void) {
  useEvery(() => void call('sync_all').then(synced, () => {}), 60_000, signedIn);
}

/** The show's state on equievents.ch, in the show's bar; it syncs in the background (every 20 s,
 * and shortly after each change here) and opens the show's panel. */
export function SyncBadge({ show, openShow, look = 'side' }: { show: string; openShow: (id: string) => void; look?: SyncLook }) {
  const t = useT();
  const { changed, refresh } = useChanges();
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [open, setOpen] = useState(false);
  const linked = !!status?.remote;

  const load = useCallback(() => call<SyncStatus>('sync_status', { show }).then(setStatus, () => {}), [show]);
  const sync = useCallback(async () => {
    try {
      const s = await call<SyncStatus>('sync', { show });
      setStatus(s);
      if (s.note?.pulled) refresh();
    } catch {
      /* the badge shows the last state */
    }
  }, [show, refresh]);

  useEffect(() => {
    load();
  }, [load]);
  useEvery(sync, 20_000, linked);
  useEffect(() => {
    if (!linked || changed === 0) return;
    const id = setTimeout(sync, 1500);
    return () => clearTimeout(id);
  }, [changed, linked, sync]);

  const note = status?.note;
  const state = !status ? null : !linked ? 'local' : (note?.status ?? 'synced');
  const label: Record<string, string> = {
    local: status?.hosted ? t('online_local_hosted') : t('online_local'),
    synced: status?.pending ? `${status.pending} ${t('online_pending')}` : t('online_synced'),
    offline: `${t('online_offline')}${status?.pending ? ` · ${status.pending}` : ''}`,
    signed_out: t('online_signed_out'),
    no_access: t('online_no_access'),
    clock: t('online_clock'),
    error: t('online_error'),
  };
  const Icon = !state ? Cloud : state === 'local' ? HardDrive : state === 'synced' ? (status?.pending ? RefreshCw : Cloud) : state === 'offline' ? CloudOff : TriangleAlert;
  const tone = !state
    ? 'text-white/50'
    : state === 'synced'
      ? 'bg-white/[0.08] text-white hover:bg-white/[0.12]'
      : state === 'local'
        ? 'text-white/60 hover:bg-white/[0.08] hover:text-white'
        : state === 'no_access' || state === 'error'
          ? 'bg-danger text-white'
          : 'bg-warn text-white';
  return (
    <>
      {look === 'icon' ? (
        <Tooltip label={`equievents.ch · ${state ? label[state] : '…'}`} side="right">
          <button onClick={() => setOpen(true)} aria-label={`equievents.ch · ${state ? label[state] : '…'}`} className={`relative grid size-9 place-items-center rounded-lg transition-colors ${tone}`}>
            <Icon className={`size-4 ${state === 'synced' && status?.pending ? 'animate-spin' : ''}`} />
            {!!note?.requests && note.role === 'owner' && (
              <span className="absolute -top-1 -right-1 min-w-4 rounded-full bg-accent px-1 text-center text-[10px] leading-4 font-semibold text-on-accent">{note.requests}</span>
            )}
          </button>
        </Tooltip>
      ) : (
        <button
          onClick={() => setOpen(true)}
          title={`equievents.ch · ${state ? label[state] : ''}`}
          className={`flex h-9 items-center gap-2.5 rounded-lg text-[13px] font-semibold whitespace-nowrap transition-colors ${look === 'bar' ? 'px-2.5' : 'w-full px-2.5'} ${tone}`}
        >
          <Icon className={`size-4 shrink-0 ${state === 'synced' && status?.pending ? 'animate-spin' : ''}`} />
          <span className={`truncate ${look === 'bar' ? 'hidden sm:inline' : 'flex-1 text-left'}`}>{state ? label[state] : '…'}</span>
          {!!note?.requests && note.role === 'owner' && <span className="rounded-full bg-accent px-1.5 text-[10px] text-on-accent">{note.requests}</span>}
        </button>
      )}
      <ConflictsNotice show={show} look={look} />
      {open && status && (
        <Drawer title="equievents.ch" close={() => setOpen(false)}>
          <OnlinePanel show={show} status={status} reload={load} sync={sync} openShow={openShow} />
        </Drawer>
      )}
    </>
  );
}

/** The show and equievents.ch: putting it online, its state, the owner's requests and team. */
function OnlinePanel({ show, status, reload, sync, openShow }: { show: string; status: SyncStatus; reload: () => void; sync: () => Promise<void>; openShow: (id: string) => void }) {
  const t = useT();
  if (status.training) return <p className="text-sm text-ink-2">{t('training_sync_long')}</p>;
  if (!status.signed_in)
    return (
      <div className="grid gap-4">
        {status.remote && <p className="text-sm text-ink-2">{t('online_signed_out_long')}</p>}
        <AccountForms
          done={() => {
            reload();
            if (status.remote) sync();
          }}
        />
      </div>
    );
  if (!status.remote) return <PutOnline show={show} status={status} done={() => (reload(), sync())} openShow={openShow} />;
  return <Linked show={show} status={status} reload={reload} sync={sync} />;
}

/** A Rosson import goes on equievents.ch at once when signed in (the user, 2026-11-21: the
 *  first to put an event up owns it); when it did not simply go up (someone else's, ours
 *  already, a key to type, no connection), what came of it and what to do. */
export function AfterImport({ show, answer, close, openShow }: { show: string; answer: PutAnswer; close: () => void; openShow: (id: string) => void }) {
  const t = useT();
  const status = useLoad(() => call<SyncStatus>('sync_status', { show }), [show]);
  return (
    <Modal
      open
      onClose={close}
      title={t('import_online_title')}
      description={t('import_online_intro')}
      icon={Cloud}
      size="md"
      footer={
        <Button variant="ghost" onClick={close}>
          {t('import_online_later')}
        </Button>
      }
    >
      {status.data && <PutOnline show={show} status={status.data} initial={answer} done={close} openShow={openShow} />}
    </Modal>
  );
}

/** What putting a show on equievents.ch gave (`put_online`). */
export type PutAnswer = { outcome: string; remote?: string; owner?: string; request?: Request | null };

/** Putting a show on equievents.ch (`initial`: what an attempt already gave, after an import). */
function PutOnline({ show, status, done, openShow, initial }: { show: string; status: SyncStatus; done: () => void; openShow: (id: string) => void; initial?: PutAnswer }) {
  const t = useT();
  const refusal = useRefusal();
  const chosen = initial && (initial.outcome === 'member' || initial.outcome === 'owned') ? initial : null;
  const [key, setKey] = useState('');
  const [askKey, setAskKey] = useState(initial?.outcome === 'need_key' || initial?.outcome === 'key');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(initial && !chosen && initial.outcome !== 'need_key' ? refusal(initial.outcome) : null);
  const [outcome, setOutcome] = useState<PutAnswer | null>(chosen);

  const run = async (f: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
    } finally {
      setBusy(false);
    }
  };
  const put = () =>
    run(async () => {
      const answer = await call<PutAnswer>('put_online', { show, access_key: key || null });
      if (answer.outcome === 'made' || answer.outcome === 'linked') return done();
      if (answer.outcome === 'need_key') return setAskKey(true);
      if (answer.outcome === 'key' || answer.outcome === 'offline' || answer.outcome === 'too_many' || answer.outcome === 'signed_out' || answer.outcome === 'error') {
        if (answer.outcome === 'key') setAskKey(true);
        return setError(refusal(answer.outcome));
      }
      setOutcome(answer);
    });
  const ask = (kind: 'access' | 'ownership') =>
    run(async () => {
      const answer = await call<{ ok: true; request: Request } | Refusal>('ask_owner', { show, remote: outcome!.remote, kind });
      if (!answer.ok) return setError(refusal(answer.code));
      setOutcome({ ...outcome!, request: answer.request });
    });
  const withdraw = () =>
    run(async () => {
      await call('withdraw_request', { id: outcome!.request!.id });
      setOutcome({ ...outcome!, request: null });
    });
  const download = () =>
    run(async () => {
      const answer = await call<{ ok: true; show: string } | Refusal>('download_show', { remote: outcome!.remote });
      if (!answer.ok) return setError(refusal(answer.code));
      openShow(answer.show);
    });

  return (
    <div className="grid gap-4 text-sm">
      {!outcome && (
        <>
          <p className="text-ink-2">{status.needs_key ? t('online_put_hint') : t('online_rehearsal_hint')}</p>
          {askKey && (
            <label className="grid gap-1 font-semibold">
              {t('online_key')}
              <input className={input} value={key} onChange={e => setKey(e.target.value)} autoFocus spellCheck={false} autoComplete="off" />
            </label>
          )}
          <button disabled={busy || (askKey && !key.trim())} onClick={put} className={`${primary} justify-self-start`}>
            {t('online_put')}
          </button>
        </>
      )}
      {outcome?.outcome === 'member' && (
        <>
          <p className="text-ink-2">{t('online_member')}</p>
          <button disabled={busy} onClick={download} className={`${primary} justify-self-start`}>
            {t('online_download')}
          </button>
        </>
      )}
      {outcome?.outcome === 'owned' && (
        <>
          <p>
            <span className="font-semibold">{outcome.owner}</span> {t('online_owned')}
          </p>
          {outcome.request?.status === 'pending' ? (
            <div className="grid gap-2 rounded-lg border border-line p-3">
              <span>
                {t('online_waiting')} ({outcome.request.kind === 'ownership' ? t('online_ask_ownership') : t('online_ask_access')})
              </span>
              <button disabled={busy} onClick={withdraw} className={`${secondary} justify-self-start`}>
                {t('online_withdraw')}
              </button>
            </div>
          ) : (
            <div className="flex flex-wrap gap-2">
              <button disabled={busy} onClick={() => ask('access')} className={primary}>
                {t('online_ask_access')}
              </button>
              <button disabled={busy} onClick={() => ask('ownership')} className={secondary}>
                {t('online_ask_ownership')}
              </button>
            </div>
          )}
        </>
      )}
      {error && <p className="font-semibold text-danger">{error}</p>}
    </div>
  );
}

function Linked({ show, status, reload, sync }: { show: string; status: SyncStatus; reload: () => void; sync: () => Promise<void> }) {
  const t = useT();
  const language = useLanguage();
  const note = status.note;
  const [busy, setBusy] = useState(false);
  const now = async () => {
    setBusy(true);
    await sync();
    setBusy(false);
  };
  const long: Record<string, Key> = { offline: 'online_offline_long', signed_out: 'online_signed_out_long', no_access: 'online_no_access_long', clock: 'online_clock_long' };
  return (
    <div className="grid gap-5 text-sm">
      <section className="grid gap-2">
        {note?.role && (
          <div>
            <span className="font-semibold">{note.role === 'owner' ? t('online_role_owner') : t('online_role_collaborator')}</span>
            {note.role !== 'owner' && note.owner && (
              <span className="text-ink-2">
                {' '}
                · {t('online_role_owner')}
                {t('colon')}
                {note.owner}
              </span>
            )}
          </div>
        )}
        {note && long[note.status] && <p className={note.status === 'no_access' ? 'font-semibold text-danger' : 'text-warn'}>{t(long[note.status])}</p>}
        {note?.status === 'error' && note.error && <p className="text-danger">{refusal(note.error)}</p>}
        {status.pending > 0 && (
          <p>
            <span className="font-semibold">{status.pending}</span> {t('online_pending_long')}
          </p>
        )}
        {note?.last_ok && (
          <p className="text-ink-3">
            {t('online_last_sync')}
            {t('colon')}
            {formatMillis(note.last_ok, language)}
          </p>
        )}
        <div className="flex flex-wrap gap-2">
          <button disabled={busy} onClick={now} className={secondary}>
            {t('online_sync_now')}
          </button>
          {note?.status === 'no_access' && (
            <button
              onClick={async () => {
                await call('unlink_show', { show });
                reload();
              }}
              className={secondary}
            >
              {t('online_unlink')}
            </button>
          )}
        </div>
      </section>
      {note?.role === 'owner' && <Requests show={show} sync={sync} />}
      {note?.role && note.status !== 'no_access' && <Team show={show} owner={note.role === 'owner'} sync={sync} reload={reload} />}
    </div>
  );
}

/** The requests waiting for the owner. Accepting a transfer: staying on as a collaborator or
 * leaving the show. */
function Requests({ show, sync, empty = null, heading = true }: { show: string; sync: () => Promise<void>; empty?: ReactNode; heading?: boolean }) {
  const t = useT();
  const refusal = useRefusal();
  const requests = useLoad(() => call<{ ok: true; requests: Request[] } | Refusal>('show_requests', { show }), [show]);
  const [error, setError] = useState<string | null>(null);
  const answer = async (r: Request, accept: boolean, stay?: boolean) => {
    setError(null);
    const a = await call<{ ok: true } | Refusal>('answer_request', { show, id: r.id, accept, stay: stay ?? null });
    if (!a.ok) setError(refusal(a.code));
    requests.reload();
    await sync();
  };
  const list = requests.data?.ok ? requests.data.requests : [];
  if (list.length === 0) return requests.data ? <>{empty}</> : null;
  return (
    <section className="grid gap-2">
      {heading && <h3 className="text-xs font-semibold uppercase tracking-wide text-ink-3">{t('online_requests')}</h3>}
      {list.map(r => (
        <div key={r.id} className="grid gap-2 rounded-lg border border-line p-3">
          <div>
            <span className="font-semibold">{r.person.name}</span> <span className="text-ink-3">{r.person.email}</span>
            <div>{r.kind === 'ownership' ? t('online_wants_ownership') : t('online_wants_access')}</div>
          </div>
          <div className="flex flex-wrap gap-2">
            {r.kind === 'ownership' ? (
              <>
                <button onClick={() => answer(r, true, true)} className={primary}>
                  {t('online_accept_stay')}
                </button>
                <button onClick={() => answer(r, true, false)} className={secondary}>
                  {t('online_accept_leave')}
                </button>
              </>
            ) : (
              <button onClick={() => answer(r, true)} className={primary}>
                {t('online_accept')}
              </button>
            )}
            <button onClick={() => answer(r, false)} className={secondary}>
              {t('online_refuse')}
            </button>
          </div>
        </div>
      ))}
      {error && <p className="font-semibold text-danger">{error}</p>}
    </section>
  );
}

/** Who works on the show. The owner adds people (with an account) and takes them off; a
 * collaborator can leave. */
function Team({ show, owner, sync, reload }: { show: string; owner: boolean; sync: () => Promise<void>; reload: () => void }) {
  const t = useT();
  const refusal = useRefusal();
  const people = useLoad(() => call<{ ok: true; people: Person[] } | Refusal>('show_people', { show }), [show]);
  const me = useLoad(() => call<AccountView>('account'), []);
  const [email, setEmail] = useState('');
  const [error, setError] = useState<string | null>(null);
  const add = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    const a = await call<{ ok: true } | Refusal>('add_person', { show, email });
    if (!a.ok) return setError(refusal(a.code));
    setEmail('');
    people.reload();
  };
  const remove = async (p: Person) => {
    setError(null);
    const a = await call<{ ok: true } | Refusal>('remove_person', { show, user: p.id });
    if (!a.ok) return setError(refusal(a.code));
    if (p.id === me.data?.account?.id) {
      await call('unlink_show', { show });
      reload();
    } else {
      people.reload();
      await sync();
    }
  };
  const list = people.data?.ok ? people.data.people : [];
  return (
    <section className="grid gap-2">
      <h3 className="text-xs font-semibold uppercase tracking-wide text-ink-3">{t('online_people')}</h3>
      {list.map(p => (
        <div key={p.id} className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <div className="min-w-0 flex-1">
            <span className="font-semibold">{p.name}</span> <span className="text-ink-3">{p.email}</span>
          </div>
          <span className="text-xs text-ink-3">{p.role === 'owner' ? t('online_role_owner') : t('online_role_collaborator')}</span>
          {p.role !== 'owner' && (owner || p.id === me.data?.account?.id) && (
            <button onClick={() => remove(p)} className="text-xs text-ink-3 hover:text-accent pointer-coarse:px-2 pointer-coarse:py-1">
              {p.id === me.data?.account?.id ? t('online_leave') : t('online_remove')}
            </button>
          )}
        </div>
      ))}
      {owner && (
        <form onSubmit={add} className="mt-1 grid gap-1">
          <label className="text-xs text-ink-3" htmlFor="add-person">
            {t('online_add_person')}
          </label>
          <div className="flex gap-2">
            <input id="add-person" className={input} type="email" value={email} onChange={e => setEmail(e.target.value)} required />
            <button className={secondary}>{t('online_add')}</button>
          </div>
        </form>
      )}
      {error && <p className="font-semibold text-danger">{error}</p>}
    </section>
  );
}

/** In the library: the shows one works on at equievents.ch that are not on this computer yet,
 * and one's requests; in the archives, those archived. */
/** The account's events on equievents.ch not on this computer yet, to download (the desktop app
 *  only: the online version has every one by itself). One's own requests are in the account's
 *  dialog (the user, 2026-11-22: less in one's face). */
export function OnlineShows({ shows, open, archived = false }: { shows: RemoteShow[]; open: (show: string) => void; archived?: boolean }) {
  const t = useT();
  const refusal = useRefusal();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const waiting = shows.filter(s => !s.local && !!s.archived === archived);
  if (waiting.length === 0) return null;
  const download = async (remote: string) => {
    setBusy(remote);
    setError(null);
    const a = await call<{ ok: true; show: string } | Refusal>('download_show', { remote });
    setBusy(null);
    if (a.ok) open(a.show);
    else setError(refusal(a.code));
  };
  return (
    <section className="grid gap-3 rounded-2xl border border-line bg-card p-5 shadow-card">
      <h2 className="flex items-center gap-2 text-[15px] font-semibold">
        <Cloud className="size-4 text-info" />
        {t('online_on_server')}
      </h2>
      {waiting.map(s => (
        <div key={s.id} className="flex flex-wrap items-center gap-3">
          <div className="min-w-0 flex-1">
            <div className="truncate font-semibold">{s.name}</div>
            <div className="text-xs text-ink-3">
              {t('online_not_here')} · {s.role === 'owner' ? t('online_role_owner') : `${t('online_role_collaborator')} · ${s.owner}`}
            </div>
          </div>
          <button disabled={busy === s.id} onClick={() => download(s.id)} className={`${primary} btn-sm`}>
            {t('online_download')}
          </button>
        </div>
      ))}
      {error && <p className="text-sm font-semibold text-danger">{error}</p>}
    </section>
  );
}

/** One's own requests (the account's dialog): those still waiting (to withdraw), and those
 *  refused lately. An accepted one shows as the event itself. */
function MyRequests() {
  const t = useT();
  const online = useLoad(() => call<{ ok: true; requests: Request[] } | Refusal>('online_shows'), []);
  if (!online.data?.ok) return null;
  const lately = Date.now() - 14 * 24 * 3600 * 1000;
  const shown = online.data.requests.filter(r => r.status === 'pending' || (r.status === 'refused' && !!r.decided_at && Date.parse(r.decided_at) > lately));
  if (shown.length === 0) return null;
  return (
    <div className="grid gap-1.5 border-t border-line pt-3 text-sm">
      <div className="text-xs font-semibold text-ink-2">{t('online_my_requests')}</div>
      {shown.map(r => (
        <div key={r.id} className="flex flex-wrap items-center gap-x-2">
          <span className="font-semibold">{r.show_name}</span>
          <span className="text-ink-3">{r.kind === 'ownership' ? t('online_ask_ownership') : t('online_ask_access')}</span>
          <span className={r.status === 'refused' ? 'text-danger' : 'text-warn'}>· {t(r.status === 'refused' ? 'online_status_refused' : 'online_status_pending')}</span>
          {r.status === 'pending' && (
            <button
              onClick={async () => {
                await call('withdraw_request', { id: r.id });
                online.reload();
              }}
              className="text-xs text-ink-3 hover:text-accent"
            >
              {t('online_withdraw')}
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

/** The requests waiting for the owner of a show, from its card in the library (the user,
 *  2026-11-22: a counter on the dashboard). */
export function RequestsDialog({ show, name, close }: { show: string; name: string; close: () => void }) {
  const t = useT();
  return (
    <Modal open onClose={close} title={t('online_requests_for').replace('{show}', name)} icon={UserRound} size="md" footer={<Button onClick={close}>{t('close')}</Button>}>
      <Requests show={show} sync={async () => void (await call('sync', { show }).catch(() => null))} empty={<p className="text-sm text-ink-3">{t('online_requests_none')}</p>} heading={false} />
    </Modal>
  );
}

/** An event from Swiss Equestrian, by its key (the user, 2026-11-21): equievents.ch downloads it,
 * the first to enter the key is its responsable, and the show comes to this computer; someone
 * else's: ask them. Signed out: sign in first (the server is the one that downloads). */
export function OpenWithKey({ close, open, signedIn }: { close: () => void; open: (show: string) => void; signedIn: () => void }) {
  const t = useT();
  const refusal = useRefusal();
  const account = useLoad(() => call<AccountView>('account'), []);
  const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [owned, setOwned] = useState<{ remote: string; owner: string; request?: Request | null } | null>(null);

  const run = async (f: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
    } finally {
      setBusy(false);
    }
  };
  const load = (e?: FormEvent) => {
    e?.preventDefault();
    if (!key.trim() || busy) return;
    run(async () => {
      const answer = await call<{ outcome: string; show?: string; remote?: string; owner?: string; request?: Request | null }>('open_with_key', { access_key: key });
      if ((answer.outcome === 'made' || answer.outcome === 'member') && answer.show) return open(answer.show);
      if (answer.outcome === 'owned') return setOwned({ remote: answer.remote ?? '', owner: answer.owner ?? '', request: answer.request });
      if (answer.outcome === 'signed_out') account.reload();
      setError(answer.outcome === 'key' ? t('online_err_key_unknown') : refusal(answer.outcome));
    });
  };
  const ask = (kind: 'access' | 'ownership') =>
    run(async () => {
      const answer = await call<{ ok: true; request: Request } | Refusal>('ask_owner', { remote: owned!.remote, kind, access_key: key });
      if (!answer.ok) return setError(refusal(answer.code));
      setOwned({ ...owned!, request: answer.request });
    });
  const withdraw = () =>
    run(async () => {
      await call('withdraw_request', { id: owned!.request!.id });
      setOwned({ ...owned!, request: null });
    });

  const signedOut = account.data && !account.data.signed_in;
  return (
    <Modal
      open
      onClose={close}
      title={t('open_key_title')}
      description={signedOut ? t('open_key_sign_in') : t('open_key_intro')}
      icon={CloudDownload}
      size="md"
      footer={
        account.data?.signed_in && !owned ? (
          <>
            <Button variant="ghost" onClick={close}>
              {t('cancel')}
            </Button>
            <Button variant="primary" type="submit" form="open-with-key" icon={CloudDownload} loading={busy} disabled={!key.trim()}>
              {t('open_key_go')}
            </Button>
          </>
        ) : undefined
      }
    >
      <div className="grid gap-4 text-sm">
        {signedOut && (
          <AccountForms
            done={() => {
              account.reload();
              signedIn();
            }}
          />
        )}
        {account.data?.signed_in && !owned && (
          <form id="open-with-key" onSubmit={load} className="grid gap-4">
            <Field label={t('online_key')}>
              <input className={input} value={key} onChange={e => setKey(e.target.value)} disabled={busy} autoFocus spellCheck={false} autoComplete="off" autoCapitalize="off" />
            </Field>
            {busy && <Callout tone="info">{t('open_key_waiting')}</Callout>}
          </form>
        )}
        {owned && (
          <>
            <p>
              <span className="font-semibold">{owned.owner}</span> {t('online_owned')}
            </p>
            {owned.request?.status === 'pending' ? (
              <div className="grid gap-2 rounded-lg border border-line p-3">
                <span>
                  {t('online_waiting')} ({owned.request.kind === 'ownership' ? t('online_ask_ownership') : t('online_ask_access')})
                </span>
                <button disabled={busy} onClick={withdraw} className={`${secondary} justify-self-start`}>
                  {t('online_withdraw')}
                </button>
              </div>
            ) : (
              <div className="flex flex-wrap gap-2">
                <button disabled={busy} onClick={() => ask('access')} className={primary}>
                  {t('online_ask_access')}
                </button>
                <button disabled={busy} onClick={() => ask('ownership')} className={secondary}>
                  {t('online_ask_ownership')}
                </button>
              </div>
            )}
          </>
        )}
        {error && <p className="font-semibold text-danger">{error}</p>}
      </div>
    </Modal>
  );
}

/** Deleting a show (the user, 2026-11-21): this copy, and for a show of equievents.ch what
 * becomes of it there. Its owner keeps the rights, gives the event to a collaborator (and leaves
 * it) or releases it (deleted for everyone: the next to enter the key downloads it fresh and owns
 * it); a collaborator keeps their access or leaves. Offline, only this copy goes. */
export function DeleteShow({
  show,
  title,
  online,
  owner,
  hosted,
  me,
  close,
  done,
}: {
  show: string;
  title: string;
  online: boolean;
  owner: boolean;
  hosted: boolean;
  me?: number;
  close: () => void;
  done: () => void;
}) {
  const t = useT();
  const refusal = useRefusal();
  const people = useLoad(() => (online ? call<{ ok: true; people: Person[] } | Refusal>('show_people', { show }) : Promise.resolve(null)), [show, online]);
  const others = people.data?.ok ? people.data.people.filter(p => p.id !== me) : [];
  const reachable = online && !!people.data?.ok;
  const [choice, setChoice] = useState<'keep' | 'give' | 'release' | 'leave'>('keep');
  const [to, setTo] = useState<number | null>(null);
  const [sure, setSure] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const given = to ?? others[0]?.id ?? null;

  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      const answer = await call<{ ok: boolean; code?: string }>('delete_show', { show, choice: reachable ? choice : 'keep', to: choice === 'give' ? given : undefined });
      if (!answer.ok) return setError(refusal(answer.code ?? 'error'));
      done();
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
    } finally {
      setBusy(false);
    }
  };

  const option = (value: typeof choice, label: string, hint: string, extra?: ReactNode, disabled = false) => (
    <label className={`grid gap-1 rounded-xl border p-3 ${choice === value ? 'border-accent bg-accent-soft/40' : 'border-line'} ${disabled ? 'opacity-50' : 'cursor-pointer'}`}>
      <span className="flex items-center gap-2 font-semibold">
        <input type="radio" name="delete-choice" checked={choice === value} disabled={disabled} onChange={() => setChoice(value)} />
        {label}
      </span>
      <span className="pl-6 text-xs text-ink-3">{hint}</span>
      {extra && choice === value && <span className="pl-6">{extra}</span>}
    </label>
  );

  const ready = !people.data && online ? false : choice === 'release' ? sure : choice === 'give' ? given !== null : true;
  return (
    <Modal
      open
      onClose={close}
      title={`${t('delete')} · ${title}`}
      icon={Trash2}
      tone="danger"
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={close}>
            {t('cancel')}
          </Button>
          <Button variant="danger-solid" icon={Trash2} loading={busy} disabled={!ready} onClick={remove}>
            {t('delete')}
          </Button>
        </>
      }
    >
      <div className="grid gap-3 text-sm">
        {!online && <p className="text-ink-2">{hosted ? t('delete_local_online') : t('delete_local')}</p>}
        {online && people.data && !people.data.ok && <Callout tone="warn">{t('delete_offline')}</Callout>}
        {reachable && owner && (
          <>
            {option('keep', t('delete_keep_owner'), t('delete_keep_owner_hint'))}
            {option(
              'give',
              t('delete_give'),
              t('delete_give_hint'),
              <select className="field field-sm" value={given ?? ''} onChange={e => setTo(Number(e.target.value))}>
                {others.map(p => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>,
              others.length === 0,
            )}
            {option(
              'release',
              t('delete_release'),
              t('delete_release_hint'),
              <span className="grid gap-2 text-xs">
                {others.length > 0 && (
                  <span className="text-danger">
                    {t('delete_release_loses')} {others.map(p => p.name).join(', ')}
                  </span>
                )}
                <Switch checked={sure} onChange={setSure} label={t('delete_release_sure')} />
              </span>,
            )}
          </>
        )}
        {reachable && !owner && (
          <>
            {option('keep', t('delete_keep_collab'), t('delete_keep_collab_hint'))}
            {option('leave', t('delete_leave'), t('delete_leave_hint'))}
          </>
        )}
        {error && <p className="font-semibold text-danger">{error}</p>}
      </div>
    </Modal>
  );
}
