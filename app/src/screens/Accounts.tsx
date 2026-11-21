// « Comptes » (the user, 2026-11-21): equievents.ch's accounts, for its administrators and
// support, in the online version only (#/accounts). Support finds an account and acts as it
// (to help it: its shows, as it). Administrators also give a new password (made and shown once,
// or typed), send a reset link, give the roles (never their own) and delete an account with all
// its data. « Concours »: the shows on equievents.ch and who works on each; administrators give
// a show another owner (the previous one stays or leaves). The server decides what each may do
// (crates/server/src/admin.rs); this screen only shows what that is.

import { useEffect, useState, type FormEvent } from 'react';
import { Archive, ArrowLeft, Check, Copy, Crown, KeyRound, Mail, Search, ShieldCheck, Trash2, Trophy, UserRoundCog, Users } from 'lucide-react';
import { staff } from '../api';
import { usePlural, useT, type Key, type Language } from '../i18n';
import { ErrorNote, useLoad } from '../parts/common';
import { reloadAs, useStaffRefusal, type StaffAccount, type StaffDetails, type StaffMe, type StaffRole, type StaffShow, type StaffShowDetails } from '../parts/staff';
import { Avatar, Button, Callout, Card, CheckRow, Choice, EmptyState, Field, Modal, Pill, Segmented, Tabs, useToast } from '../ui';

const roleKey: Record<StaffRole, Key> = { admin: 'staff_role_admin', support: 'staff_role_support', '': 'staff_role_none' };

function RolePill({ role }: { role: StaffRole }) {
  const t = useT();
  if (!role) return null;
  return (
    <Pill tone={role === 'admin' ? 'brand' : 'info'} icon={role === 'admin' ? ShieldCheck : UserRoundCog}>
      {t(roleKey[role])}
    </Pill>
  );
}

/** A day as Swiss people write it: 21.11.2026. */
function day(iso: string, language: Language): string {
  return new Date(iso).toLocaleDateString(`${language}-CH`, { day: 'numeric', month: 'numeric', year: 'numeric' });
}

/** The first letter up (a phrase that opens a line). */
const opening = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

/** « Responsable de 2 concours · collabore à 1 »: a counted phrase, singular for one. */
function useCounted() {
  const t = useT();
  return (n: number, key: 'staff_owns' | 'staff_works_on' | 'staff_collaborators') => (n === 1 ? t(`${key}_one` as Key) : t(key).replace('{n}', String(n)));
}

/** Searched a moment after typing stops. */
function useSearch() {
  const [query, setQuery] = useState('');
  const [search, setSearch] = useState('');
  useEffect(() => {
    const timer = setTimeout(() => setSearch(query.trim()), 250);
    return () => clearTimeout(timer);
  }, [query]);
  return { query, setQuery, search };
}

function SearchField({ id, value, onChange, label }: { id: string; value: string; onChange: (v: string) => void; label: string }) {
  return (
    <div className="border-b border-line p-3 sm:px-5">
      <label className="relative block">
        <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-ink-3" aria-hidden />
        <input
          id={id}
          className="field w-full pl-9"
          type="search"
          value={value}
          onChange={e => onChange(e.target.value)}
          placeholder={label}
          aria-label={label}
          autoComplete="off"
          spellCheck={false}
        />
      </label>
    </div>
  );
}

const rowClass = 'flex w-full flex-wrap items-center gap-x-3 gap-y-1 px-4 py-3 text-left hover:bg-hover focus-visible:bg-hover focus-visible:outline-none sm:px-5 pointer-coarse:py-4';

export default function Accounts({ me, language }: { me: StaffMe; language: Language }) {
  const t = useT();
  const [tab, setTab] = useState<'accounts' | 'shows'>('accounts');
  const [round, setRound] = useState(0);
  const [account, setAccount] = useState<number | null>(null);
  const [show, setShow] = useState<string | null>(null);
  const changed = () => setRound(r => r + 1);

  return (
    <div className="h-full overflow-auto">
      <div className="relative overflow-hidden bg-nav pb-20 text-white">
        <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(80%_120%_at_100%_0%,rgb(31_111_196/0.22),transparent_55%)]" aria-hidden />
        <div className="relative mx-auto max-w-6xl px-4 pt-[max(1.25rem,env(safe-area-inset-top))] sm:px-6 sm:pt-8">
          <a href="#/" className="inline-flex items-center gap-1.5 text-[13px] font-semibold text-accent-nav hover:text-white">
            <ArrowLeft className="size-4" />
            {t('library')}
          </a>
          <div className="mt-6 flex flex-wrap items-end gap-3 sm:mt-10">
            <div className="min-w-0 flex-1">
              <div className="text-[13px] font-semibold text-accent-nav">{t('staff_eyebrow')}</div>
              <h1 className="mt-1 text-[28px] leading-tight font-bold tracking-tight sm:text-[34px]">{tab === 'accounts' ? t('staff_accounts') : t('staff_shows')}</h1>
              <p className="mt-1 text-sm text-white/60">{t(roleKey[me.role])}</p>
            </div>
          </div>
        </div>
      </div>

      <main className="relative mx-auto -mt-12 max-w-6xl space-y-4 px-4 pb-10 sm:px-6">
        <Card>
          <Tabs
            value={tab}
            onChange={setTab}
            items={[
              { value: 'accounts', label: t('staff_accounts'), icon: Users },
              { value: 'shows', label: t('staff_shows'), icon: Trophy },
            ]}
            className="border-b border-line px-2 sm:px-3"
          />
          {tab === 'accounts' ? <AccountList me={me} language={language} round={round} open={setAccount} /> : <ShowList language={language} round={round} open={setShow} />}
        </Card>
      </main>

      {account !== null && (
        <AccountDialog
          id={account}
          me={me}
          language={language}
          close={() => setAccount(null)}
          changed={changed}
          deleted={() => {
            setAccount(null);
            changed();
          }}
        />
      )}
      {show !== null && <ShowDialog id={show} me={me} language={language} close={() => setShow(null)} changed={changed} />}
    </div>
  );
}

function AccountList({ me, language, round, open }: { me: StaffMe; language: Language; round: number; open: (id: number) => void }) {
  const t = useT();
  const plural = usePlural();
  const counted = useCounted();
  const { query, setQuery, search } = useSearch();
  const accounts = useLoad(() => staff<StaffAccount[]>(`accounts?q=${encodeURIComponent(search)}`), [search, round]);
  const list = accounts.data ?? [];
  return (
    <>
      <SearchField id="staff-search" value={query} onChange={setQuery} label={t('staff_search')} />
      <ErrorNote error={accounts.error} />
      {accounts.data && list.length === 0 ? (
        <EmptyState icon={Users} title={t('staff_none_found')} />
      ) : (
        <ul className="divide-y divide-line">
          {list.map(a => (
            <li key={a.id}>
              <button onClick={() => open(a.id)} className={rowClass}>
                <Avatar name={a.name} />
                <div className="min-w-0 flex-1 basis-48">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="truncate text-[14px] font-semibold text-ink">{a.name}</span>
                    <RolePill role={a.role} />
                    {a.id === me.id && <Pill>{t('staff_you')}</Pill>}
                  </div>
                  <div className="truncate text-[13px] text-ink-3">{a.email}</div>
                </div>
                <div className="grid basis-full gap-0.5 pl-12 text-xs text-ink-3 sm:basis-auto sm:pl-0 sm:text-right">
                  <span>
                    {a.owns || a.works_on ? opening([a.owns && counted(a.owns, 'staff_owns'), a.works_on && counted(a.works_on, 'staff_works_on')].filter(Boolean).join(' · ')) : t('staff_no_shows')}
                  </span>
                  <span className="tabular">{a.last_seen ? t('staff_last_seen').replace('{date}', day(a.last_seen, language)) : t('staff_never_seen')}</span>
                </div>
              </button>
            </li>
          ))}
        </ul>
      )}
      {accounts.data && list.length > 0 && (
        <p className="border-t border-line px-4 py-2.5 text-xs text-ink-3 sm:px-5">
          {list.length}
          {list.length === 500 ? '+' : ''} {plural(list.length, 'staff_count')}
        </p>
      )}
    </>
  );
}

/** The shows on equievents.ch: found by name, number or owner, the latest first. */
function ShowList({ language, round, open }: { language: Language; round: number; open: (id: string) => void }) {
  const t = useT();
  const plural = usePlural();
  const counted = useCounted();
  const { query, setQuery, search } = useSearch();
  const shows = useLoad(() => staff<StaffShow[]>(`shows?q=${encodeURIComponent(search)}`), [search, round]);
  const list = shows.data ?? [];
  return (
    <>
      <SearchField id="staff-show-search" value={query} onChange={setQuery} label={t('staff_shows_search')} />
      <ErrorNote error={shows.error} />
      {shows.data && list.length === 0 ? (
        <EmptyState icon={Trophy} title={t('staff_shows_none')} />
      ) : (
        <ul className="divide-y divide-line">
          {list.map(s => (
            <li key={s.id}>
              <button onClick={() => open(s.id)} className={rowClass}>
                <div className="min-w-0 flex-1 basis-56">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-[14px] font-semibold text-ink">{s.name}</span>
                    {s.archived && <Pill icon={Archive}>{t('staff_archived')}</Pill>}
                  </div>
                  <div className="text-[13px] text-ink-3">
                    {s.owner ? (
                      <>
                        {t('staff_owner').replace('{name}', s.owner.name)} <span className="text-ink-4">· {s.owner.email}</span>
                      </>
                    ) : (
                      t('staff_no_owner')
                    )}
                  </div>
                </div>
                <div className="grid basis-full gap-0.5 text-xs text-ink-3 sm:basis-auto sm:text-right">
                  <span>{s.collaborators > 0 ? opening(counted(s.collaborators, 'staff_collaborators')) : t('staff_no_collaborators')}</span>
                  <span className="tabular">
                    {s.last_day ? t('staff_last_day').replace('{date}', day(s.last_day, language)) : s.event_id ? t('staff_event').replace('{id}', String(s.event_id)) : s.id}
                  </span>
                </div>
              </button>
            </li>
          ))}
        </ul>
      )}
      {shows.data && list.length > 0 && (
        <p className="border-t border-line px-4 py-2.5 text-xs text-ink-3 sm:px-5">
          {list.length}
          {list.length === 500 ? '+' : ''} {plural(list.length, 'staff_shows_count')}
        </p>
      )}
    </>
  );
}

/** One show: who works on it; for administrators, another owner. */
function ShowDialog({ id, me, language, close, changed }: { id: string; me: StaffMe; language: Language; close: () => void; changed: () => void }) {
  const t = useT();
  const details = useLoad(() => staff<StaffShowDetails>(`shows/${encodeURIComponent(id)}`), [id]);
  const [owner, setOwner] = useState(false);
  const show = details.data?.show;
  const facts = show
    ? [show.event_id ? t('staff_event').replace('{id}', String(show.event_id)) : null, show.last_day ? t('staff_last_day').replace('{date}', day(show.last_day, language)) : null]
        .filter(Boolean)
        .join(' · ')
    : '';
  return (
    <>
      <Modal
        open={!owner}
        onClose={close}
        title={show?.name ?? t('loading')}
        description={facts || undefined}
        icon={Trophy}
        size="md"
        footer={
          <>
            <Button variant={me.role === 'admin' ? 'ghost' : 'primary'} onClick={close}>
              {t('close')}
            </Button>
            {me.role === 'admin' && details.data && (
              <Button variant="primary" icon={Crown} onClick={() => setOwner(true)}>
                {t('staff_owner_change')}
              </Button>
            )}
          </>
        }
      >
        <ErrorNote error={details.error} />
        {details.data && (
          <section className="grid gap-2 text-sm">
            <h3 className="text-xs font-semibold text-ink-2">{t('staff_people')}</h3>
            <ul className="grid gap-1.5">
              {details.data.people.map(p => (
                <li key={p.id} className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-lg bg-sunken px-3 py-2">
                  <Avatar name={p.name} size="sm" />
                  <div className="min-w-0 flex-1 basis-40">
                    <div className="font-semibold text-ink">{p.name}</div>
                    <div className="truncate text-xs text-ink-3">{p.email}</div>
                  </div>
                  {p.role === 'owner' ? (
                    <Pill tone="accent" icon={Crown}>
                      {t('online_role_owner')}
                    </Pill>
                  ) : (
                    <span className="text-xs text-ink-3">{t('online_role_collaborator')}</span>
                  )}
                </li>
              ))}
            </ul>
          </section>
        )}
      </Modal>
      {owner && details.data && (
        <OwnerDialog
          details={details.data}
          close={() => setOwner(false)}
          done={() => {
            setOwner(false);
            details.reload();
            changed();
          }}
        />
      )}
    </>
  );
}

/** Another owner for a show (administrators): one who works on it or any account; the previous
 *  owner stays as a collaborator or leaves. Both are told by e-mail. */
function OwnerDialog({ details, close, done }: { details: StaffShowDetails; close: () => void; done: () => void }) {
  const t = useT();
  const toast = useToast();
  const refusal = useStaffRefusal();
  const previous = details.people.find(p => p.role === 'owner');
  const collaborators = details.people.filter(p => p.role === 'collaborator');
  const [to, setTo] = useState<{ id: number; name: string } | null>(collaborators.length === 1 ? collaborators[0] : null);
  const [other, setOther] = useState(collaborators.length === 0);
  const [stays, setStays] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { query, setQuery, search } = useSearch();
  const found = useLoad(() => (other && search ? staff<StaffAccount[]>(`accounts?q=${encodeURIComponent(search)}`) : Promise.resolve([] as StaffAccount[])), [other, search]);
  const show = details.show;

  const submit = async () => {
    if (!to) return;
    setBusy(true);
    setError(null);
    try {
      await staff(`shows/${encodeURIComponent(show.id)}/owner`, 'POST', { account: to.id, previous: stays ? 'stay' : 'leave' });
      toast.show({ text: t('staff_owner_done').replace('{name}', to.name).replace('{show}', show.name) });
      done();
    } catch (e) {
      setError(refusal(e));
      setBusy(false);
    }
  };

  return (
    <Modal
      open
      onClose={close}
      title={t('staff_owner_title').replace('{show}', show.name)}
      description={t('staff_owner_intro')}
      icon={Crown}
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={close}>
            {t('cancel')}
          </Button>
          <Button variant="primary" icon={Crown} loading={busy} disabled={!to} onClick={submit}>
            {to ? t('staff_owner_go').replace('{name}', to.name) : t('staff_owner_change')}
          </Button>
        </>
      }
    >
      <div className="grid gap-5 text-sm">
        <Field label={t('staff_owner_who')} group>
          <div className="grid gap-2" role="radiogroup">
            {collaborators.map(c => (
              <Choice key={c.id} selected={!other && to?.id === c.id} onSelect={() => (setOther(false), setTo(c))} title={c.name} hint={`${c.email} · ${t('online_role_collaborator')}`} />
            ))}
            <Choice selected={other} onSelect={() => (setOther(true), setTo(null))} title={t('staff_owner_other')}>
              {other && (
                <div className="grid gap-2">
                  <input
                    id="staff-owner-search"
                    className="field w-full"
                    type="search"
                    value={query}
                    onChange={e => setQuery(e.target.value)}
                    placeholder={t('staff_search')}
                    aria-label={t('staff_search')}
                    autoComplete="off"
                    spellCheck={false}
                    autoFocus
                  />
                  {(found.data ?? [])
                    .filter(a => a.id !== previous?.id)
                    .slice(0, 8)
                    .map(a => (
                      <button
                        key={a.id}
                        type="button"
                        onClick={() => setTo(a)}
                        aria-pressed={to?.id === a.id}
                        className={`flex items-center gap-2.5 rounded-lg border px-3 py-2 text-left ${to?.id === a.id ? 'border-accent bg-accent-soft/40' : 'border-line hover:bg-hover'}`}
                      >
                        <Avatar name={a.name} size="sm" />
                        <span className="min-w-0 flex-1">
                          <span className="block font-semibold text-ink">{a.name}</span>
                          <span className="block truncate text-xs text-ink-3">{a.email}</span>
                        </span>
                        {to?.id === a.id && <Check className="size-4 text-accent" aria-hidden />}
                      </button>
                    ))}
                </div>
              )}
            </Choice>
          </div>
        </Field>
        {previous && (
          <Field label={t('staff_owner_previous').replace('{name}', previous.name)} group>
            <div className="grid gap-2" role="radiogroup">
              <Choice selected={stays} onSelect={() => setStays(true)} title={t('staff_owner_stays')} />
              <Choice selected={!stays} onSelect={() => setStays(false)} title={t('staff_owner_leaves')} hint={t('staff_owner_leaves_hint')} />
            </div>
          </Field>
        )}
        {error && <p className="font-semibold text-danger">{error}</p>}
      </div>
    </Modal>
  );
}

/** One account: its shows; acting as it (staff); for administrators, its role, a new password,
 *  a reset link, deleting it. */
function AccountDialog({ id, me, language, close, changed, deleted }: { id: number; me: StaffMe; language: Language; close: () => void; changed: () => void; deleted: () => void }) {
  const t = useT();
  const toast = useToast();
  const refusal = useStaffRefusal();
  const counted = useCounted();
  const details = useLoad(() => staff<StaffDetails>(`accounts/${id}`), [id]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dialog, setDialog] = useState<'password' | 'delete' | null>(null);
  const [show, setShow] = useState<string | null>(null);
  const a = details.data?.account;
  const self = id === me.id;
  const admin = me.role === 'admin';
  // Support never acts as staff (the server refuses it too).
  const mayAct = !self && a && (admin || !a.role);

  const run = async (what: string, action: () => Promise<void>) => {
    setBusy(what);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(refusal(e));
    } finally {
      setBusy(null);
    }
  };
  const act = () =>
    run('act', async () => {
      await staff(`accounts/${id}/act`, 'POST');
      reloadAs();
    });
  const setRole = (role: StaffRole) =>
    run('role', async () => {
      await staff(`accounts/${id}/role`, 'POST', { role: role || 'none' });
      details.reload();
      changed();
      toast.show({ text: t('staff_role_saved').replace('{name}', a!.name).replace('{role}', t(roleKey[role])) });
    });
  const resetLink = () =>
    run('reset', async () => {
      const sent = await staff<{ mail: 'off' | 'redirect' | 'on' }>(`accounts/${id}/reset-link`, 'POST');
      toast.show(
        sent.mail === 'off' ? { text: t('staff_reset_off'), tone: 'info' } : { text: (sent.mail === 'redirect' ? t('staff_reset_redirect') : t('staff_reset_sent')).replace('{email}', a!.email) },
      );
    });

  return (
    <>
      <Modal
        open={dialog === null && show === null}
        onClose={close}
        title={a?.name ?? t('loading')}
        description={a?.email}
        icon={UserRoundCog}
        size="md"
        footer={
          <>
            <Button variant="ghost" onClick={close}>
              {t('close')}
            </Button>
            {mayAct && (
              <Button variant="primary" icon={UserRoundCog} loading={busy === 'act'} onClick={act}>
                {t('staff_act').replace('{name}', a!.name)}
              </Button>
            )}
          </>
        }
      >
        <ErrorNote error={details.error} />
        {a && details.data && (
          <div className="grid gap-5 text-sm">
            <div className="flex flex-wrap items-center gap-2 text-xs text-ink-3">
              <RolePill role={a.role} />
              {self && <Pill>{t('staff_you')}</Pill>}
              <span>{t('staff_created').replace('{date}', day(a.created_at, language))}</span>
              <span>·</span>
              <span>{a.last_seen ? t('staff_last_seen').replace('{date}', day(a.last_seen, language)) : t('staff_never_seen')}</span>
            </div>

            {!self && (mayAct ? <p className="text-[13px] text-ink-2">{t('staff_act_hint').replace('{name}', a.name)}</p> : <Callout>{t('staff_act_staff')}</Callout>)}

            <section className="grid gap-2">
              <h3 className="text-xs font-semibold text-ink-2">{t('staff_shows')}</h3>
              {details.data.owns.length + details.data.works_on.length === 0 ? (
                <p className="text-[13px] text-ink-3">{t('staff_no_shows')}</p>
              ) : (
                <ul className="grid gap-1.5">
                  {details.data.owns.map(s => (
                    <li key={s.id}>
                      <button onClick={() => setShow(s.id)} className="flex w-full flex-wrap items-baseline gap-x-2 rounded-lg bg-sunken px-3 py-2 text-left hover:bg-hover">
                        <span className="min-w-0 flex-1 basis-40 font-semibold text-ink">{s.name}</span>
                        <span className="text-xs text-ink-3">
                          {t('online_role_owner')}
                          {s.collaborators > 0 && ` · ${counted(s.collaborators, 'staff_collaborators')}`}
                        </span>
                      </button>
                    </li>
                  ))}
                  {details.data.works_on.map(s => (
                    <li key={s.id}>
                      <button onClick={() => setShow(s.id)} className="flex w-full flex-wrap items-baseline gap-x-2 rounded-lg bg-sunken px-3 py-2 text-left hover:bg-hover">
                        <span className="min-w-0 flex-1 basis-40 font-semibold text-ink">{s.name}</span>
                        <span className="text-xs text-ink-3">{t('staff_owner').replace('{name}', s.owner)}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </section>

            {admin && (
              <section className="grid gap-3 border-t border-line pt-4">
                <h3 className="text-xs font-semibold text-ink-2">{t('staff_tools')}</h3>
                <Field label={t('staff_role')} hint={self ? t('staff_role_self') : t('staff_role_hint')} group>
                  <Segmented<StaffRole>
                    value={a.role}
                    onChange={role => !self && busy === null && role !== a.role && setRole(role)}
                    options={(['', 'support', 'admin'] as StaffRole[]).map(r => ({ value: r, label: t(roleKey[r]) }))}
                    className={self ? 'pointer-events-none opacity-60' : ''}
                  />
                </Field>
                <div className="flex flex-wrap gap-2">
                  <Button icon={KeyRound} onClick={() => setDialog('password')}>
                    {t('staff_password')}
                  </Button>
                  <Button icon={Mail} loading={busy === 'reset'} onClick={resetLink} title={t('staff_reset_hint').replace('{name}', a.name)}>
                    {t('staff_reset')}
                  </Button>
                  {!self && (
                    <Button variant="danger" icon={Trash2} onClick={() => setDialog('delete')}>
                      {t('staff_delete')}
                    </Button>
                  )}
                </div>
              </section>
            )}
            {error && <p className="font-semibold text-danger">{error}</p>}
          </div>
        )}
      </Modal>
      {a && dialog === 'password' && <PasswordDialog account={a} close={() => setDialog(null)} />}
      {show !== null && (
        <ShowDialog
          id={show}
          me={me}
          language={language}
          close={() => setShow(null)}
          changed={() => {
            details.reload();
            changed();
          }}
        />
      )}
      {a && details.data && dialog === 'delete' && (
        <DeleteDialog
          details={details.data}
          close={() => setDialog(null)}
          done={() => {
            toast.show({ text: t('staff_deleted').replace('{name}', a.name) });
            deleted();
          }}
        />
      )}
    </>
  );
}

/** A new password: made by equievents.ch (shown once, to pass on) or typed. The account is
 *  signed out everywhere. */
function PasswordDialog({ account, close }: { account: StaffAccount; close: () => void }) {
  const t = useT();
  const refusal = useStaffRefusal();
  const [how, setHow] = useState<'make' | 'type'>('make');
  const [typed, setTyped] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<{ made: string | null } | null>(null);
  const [copied, setCopied] = useState(false);
  const name = account.name;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const answer = await staff<{ password: string | null }>(`accounts/${account.id}/password`, 'POST', how === 'type' ? { password: typed } : {});
      setDone({ made: answer.password });
    } catch (e) {
      setError(refusal(e));
    } finally {
      setBusy(false);
    }
  };
  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };

  return (
    <Modal
      open
      onClose={close}
      title={t('staff_password_title').replace('{name}', name)}
      description={done ? undefined : t('staff_password_intro').replace('{name}', name)}
      icon={KeyRound}
      size="sm"
      footer={
        done ? (
          <Button variant="primary" onClick={close}>
            {t('close')}
          </Button>
        ) : (
          <>
            <Button variant="ghost" onClick={close}>
              {t('cancel')}
            </Button>
            <Button variant="primary" type="submit" form="staff-password" icon={KeyRound} loading={busy} disabled={how === 'type' && typed.length < 10}>
              {t('staff_password_set')}
            </Button>
          </>
        )
      }
    >
      {done ? (
        <div className="grid gap-3 text-sm">
          {done.made ? (
            <>
              <p>{t('staff_password_made').replace('{name}', name)}</p>
              <div className="flex items-center gap-2 rounded-lg border border-line bg-sunken p-2 pl-3">
                <code className="min-w-0 flex-1 font-mono text-[15px] font-semibold tracking-wide break-all text-ink select-all">{done.made}</code>
                <Button size="sm" icon={copied ? Check : Copy} onClick={() => copy(done.made!)}>
                  {copied ? t('staff_copied') : t('staff_copy')}
                </Button>
              </div>
            </>
          ) : null}
          <Callout>{t('staff_password_done').replace('{name}', name)}</Callout>
        </div>
      ) : (
        <form id="staff-password" onSubmit={submit} className="grid gap-2.5 text-sm" role="radiogroup">
          <Choice selected={how === 'make'} onSelect={() => setHow('make')} title={t('staff_password_make')} hint={t('staff_password_make_hint').replace('{name}', name)} />
          <Choice selected={how === 'type'} onSelect={() => setHow('type')} title={t('staff_password_type')} hint={t('staff_password_type_hint')}>
            {how === 'type' && (
              <input
                id="staff-password-typed"
                className="field w-full"
                type="text"
                value={typed}
                onChange={e => setTyped(e.target.value)}
                autoFocus
                autoComplete="new-password"
                spellCheck={false}
                autoCapitalize="off"
                aria-label={t('staff_password_type')}
              />
            )}
          </Choice>
          {error && <p className="font-semibold text-danger">{error}</p>}
        </form>
      )}
    </Modal>
  );
}

/** Deleting an account and all its data: what goes with it said first, a tick to be sure. */
function DeleteDialog({ details, close, done }: { details: StaffDetails; close: () => void; done: () => void }) {
  const t = useT();
  const refusal = useStaffRefusal();
  const counted = useCounted();
  const [sure, setSure] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const a = details.account;

  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      await staff(`accounts/${a.id}`, 'DELETE');
      done();
    } catch (e) {
      setError(refusal(e));
      setBusy(false);
    }
  };

  return (
    <Modal
      open
      onClose={close}
      title={t('staff_delete_title').replace('{name}', a.name)}
      description={a.email}
      icon={Trash2}
      tone="danger"
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={close}>
            {t('cancel')}
          </Button>
          <Button variant="danger-solid" icon={Trash2} loading={busy} disabled={!sure} onClick={remove}>
            {t('staff_delete_go')}
          </Button>
        </>
      }
    >
      <div className="grid gap-4 text-sm">
        <p>{t('staff_delete_intro')}</p>
        {details.owns.length > 0 && (
          <div className="grid gap-1.5">
            <p className="font-semibold text-ink">{t('staff_delete_owned')}</p>
            <ul className="grid gap-1 pl-1">
              {details.owns.map(s => (
                <li key={s.id} className="flex flex-wrap gap-x-2">
                  <span className="font-semibold">{s.name}</span>
                  {s.collaborators > 0 && <span className="text-ink-3">{counted(s.collaborators, 'staff_collaborators')}</span>}
                </li>
              ))}
            </ul>
          </div>
        )}
        {details.works_on.length > 0 && (
          <div className="grid gap-1.5">
            <p className="font-semibold text-ink">{t('staff_delete_works_on')}</p>
            <ul className="grid gap-1 pl-1">
              {details.works_on.map(s => (
                <li key={s.id}>
                  <span className="font-semibold">{s.name}</span> <span className="text-ink-3">{t('staff_owner').replace('{name}', s.owner)}</span>
                </li>
              ))}
            </ul>
          </div>
        )}
        <p className="text-[13px] text-ink-3">{t('staff_delete_history')}</p>
        <CheckRow checked={sure} onChange={setSure}>
          {t('staff_delete_sure')}
        </CheckRow>
        {error && <p className="font-semibold text-danger">{error}</p>}
      </div>
    </Modal>
  );
}
