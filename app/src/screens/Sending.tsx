// Swiss Equestrian: where the show's requests go (a show of equievents.ch sends from there; else
// the simulation here) and with which access key; the event's update from Swiss Equestrian (a
// show of equievents.ch); the registry of riders and horses; for every class, its start lists and
// results, when they were sent and whether they changed since; the event; and, with the
// simulation, exactly what it received.

import { useState, type ComponentType, type ReactNode } from 'react';
import { call, type Connection, type Received, type RegistryAbout, type SendingView, type SendRow, type SentView, type Server } from '../api';
import { formatMillis, useLanguage, useT, type Key } from '../i18n';
import { useChanges } from '../parts/changes';
import { ErrorNote, useLoad, useNarrow } from '../parts/common';
import { useRefusal } from '../parts/online';
import { Check, CloudUpload, Database, Ellipsis, FlaskConical, Globe, KeyRound, Radio, RefreshCw, Send, TestTube, Undo2, UserRound } from 'lucide-react';
import { Button, Callout, Card, CardHeader, IconButton, Menu, MenuItem, Modal, PageHeader, Pill, Stat } from '../ui';
import { Horseshoe } from '../ui/icons';
import { ClassNumber } from './Classes';

type Send = (key: string, args: Record<string, unknown>) => void;

export default function Sending({ show }: { show: string }) {
  const t = useT();
  const language = useLanguage();
  const { version, change } = useChanges();
  const [round, setRound] = useState(0);
  const connection = useLoad(() => call<Connection>('connection', { show }), [show, round]);
  const view = useLoad(() => call<SendingView>('sending', { show }), [show, version, round]);
  const log = useLoad(() => call<Received[]>('simulation_log', { show }), [show, version, round]);
  const registry = useLoad(() => call<RegistryAbout>('registry'), [show, round]);
  const narrow = useNarrow();
  const [warnings, setWarnings] = useState<string[]>([]);
  const [busy, setBusy] = useState<string | null>(null);

  const send: Send = async (key, args) => {
    setBusy(key);
    setWarnings([]);
    const answer = await change('send', { language, ...args });
    setBusy(null);
    if (answer) setWarnings((answer.warnings as string[]) ?? []);
  };
  const again = () => setRound(r => r + 1);

  return (
    <div className="h-full overflow-auto">
      <div className="mx-auto max-w-6xl space-y-5 px-3 py-5 sm:px-6 sm:py-7">
        <PageHeader icon={CloudUpload} title="Swiss Equestrian" subtitle={t('sending_hint')} />
        <ErrorNote error={connection.error ?? view.error} />
        <div className="grid items-stretch gap-5 lg:grid-cols-[3fr_2fr]">
          {connection.data && <ConnectionCard show={show} connection={connection.data} changed={again} />}
          {registry.data && <RegistryCard show={show} about={registry.data} downloaded={again} />}
        </div>

        {warnings.length > 0 && (
          <Callout tone="warn" title={t('sent_with_warnings')}>
            <ul className="mt-1 list-disc pl-5 text-xs">
              {warnings.map((w, i) => (
                <li key={i}>{w}</li>
              ))}
            </ul>
          </Callout>
        )}

        {view.data && (
          <Card className="overflow-hidden">
            <div className="flex flex-wrap items-center gap-x-4 gap-y-3 border-b border-line px-4 py-4 sm:px-5">
              <div className="grid size-10 shrink-0 place-items-center rounded-xl bg-brand text-white shadow-card">
                <Radio className="size-[18px]" />
              </div>
              <div className="min-w-0 flex-1">
                <h2 className="text-[15px] font-semibold tracking-tight text-ink">{t('send_event')}</h2>
                <div className="text-xs text-ink-3">{t('event_hint')}</div>
              </div>
              <div className="flex items-center gap-3">
                <SendState sent={view.data.event_sent} changed={false} />
                <SendAction id="event" sent={view.data.event_sent} changed={false} busy={busy} onSend={() => send('event', { what: 'event' })} />
              </div>
            </div>
            {narrow ? (
              <ul>
                {view.data.rows.map(r => (
                  <li key={r.class.id} className="space-y-2.5 border-t border-line px-4 py-3 first:border-t-0">
                    <ClassCell row={r} />
                    {r.lists_here && <Line label={t('start_lists_col')} cell={<ListsCell row={r} busy={busy} send={send} />} />}
                    <Line label={t('results_col')} cell={<ResultsCell row={r} busy={busy} send={send} />} />
                  </li>
                ))}
              </ul>
            ) : (
              <table className="w-full text-sm">
                <thead>
                  <tr className="bg-raised text-left text-xs font-semibold text-ink-3">
                    <th className="border-b border-line px-5 py-2.5">{t('class')}</th>
                    <th className="w-[30%] border-b border-line px-4 py-2.5">{t('start_lists_col')}</th>
                    <th className="w-[30%] border-b border-line px-4 py-2.5">{t('results_col')}</th>
                  </tr>
                </thead>
                <tbody>
                  {view.data.rows.map(r => (
                    <tr key={r.class.id} className="transition-colors hover:bg-hover/60">
                      <td className="max-w-0 border-b border-line px-5 py-3">
                        <ClassCell row={r} />
                      </td>
                      <td className="border-b border-line px-4 py-3">{r.lists_here ? <ListsCell row={r} busy={busy} send={send} /> : <span className="text-ink-4">—</span>}</td>
                      <td className="border-b border-line px-4 py-3">
                        <ResultsCell row={r} busy={busy} send={send} />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Card>
        )}

        {connection.data?.server === 'simulation' && <SimulationLog log={log.data ?? []} />}
      </div>
    </div>
  );
}

function ClassCell({ row }: { row: SendRow }) {
  const t = useT();
  return (
    <div className="flex min-w-0 items-center gap-3">
      <ClassNumber number={row.class.number} />
      <div className="min-w-0">
        <div className="truncate font-semibold text-ink">{row.class.title}</div>
        <div className="truncate text-xs text-ink-3">
          {row.class.discipline} · {row.class.category} · {row.class.results} {t('results')}
        </div>
      </div>
    </div>
  );
}

/** A label and its cell, on phones. */
function Line({ label, cell }: { label: string; cell: ReactNode }) {
  return (
    <div className="rounded-xl bg-raised px-3 py-2">
      <div className="mb-1 text-[11px] font-semibold text-ink-3">{label}</div>
      {cell}
    </div>
  );
}

function ListsCell({ row, busy, send }: { row: SendRow; busy: string | null; send: Send }) {
  const id = `l${row.class.published}`;
  return (
    <div className="flex items-center justify-between gap-3">
      <SendState sent={row.start_lists_sent} changed={row.start_lists_changed} />
      <SendAction
        id={id}
        sent={row.start_lists_sent}
        changed={row.start_lists_changed}
        busy={busy}
        onSend={() => send(id, { what: 'start_lists', published: row.class.published })}
        onWithdraw={() => send(id, { what: 'withdraw_start_lists', published: row.class.published })}
      />
    </div>
  );
}

function ResultsCell({ row, busy, send }: { row: SendRow; busy: string | null; send: Send }) {
  const t = useT();
  const id = `r${row.class.id}`;
  if (row.class.results === 0 && !row.results_sent) return <span className="text-xs text-ink-4">{t('no_results_yet')}</span>;
  return (
    <div className="flex items-center justify-between gap-3">
      <SendState sent={row.results_sent} changed={row.results_changed} />
      {row.class.results > 0 && <SendAction id={id} sent={row.results_sent} changed={row.results_changed} busy={busy} onSend={() => send(id, { what: 'results', class: row.class.id })} />}
    </div>
  );
}

/** Not sent; sent (when, by whom); changed since. */
function SendState({ sent, changed }: { sent: SentView | null; changed: boolean }) {
  const t = useT();
  const language = useLanguage();
  if (!sent) return <Pill dot>{t('not_sent')}</Pill>;
  return (
    <span className="flex min-w-0 flex-col items-start gap-0.5">
      {changed ? (
        <Pill tone="warn" dot>
          {t('changed_since')}
        </Pill>
      ) : (
        <Pill tone="ok" icon={Check}>
          {t('sent')}
        </Pill>
      )}
      <span className="max-w-full truncate text-[11px] text-ink-3 tabular">
        {formatMillis(sent.millis, language)} · {sent.author}
      </span>
    </span>
  );
}

/** Send (not sent yet), send again (changed since: the one to do), or, when up to date, a menu
 *  with sending again and withdrawing. */
function SendAction({ id, sent, changed, busy, onSend, onWithdraw }: { id: string; sent: SentView | null; changed: boolean; busy: string | null; onSend: () => void; onWithdraw?: () => void }) {
  const t = useT();
  if (!sent || changed)
    return (
      <Button size="sm" variant={sent ? 'primary' : 'secondary'} icon={Send} loading={busy === id} disabled={busy !== null} onClick={onSend}>
        {sent ? t('resend') : t('send_now')}
      </Button>
    );
  if (busy === id) return <Button size="sm" variant="ghost" loading disabled aria-label={t('send_now')} />;
  return (
    <Menu width="w-56" trigger={<IconButton icon={Ellipsis} label={t('actions_more')} size="sm" disabled={busy !== null} />}>
      <MenuItem icon={Send} onSelect={onSend}>
        {t('resend')}
      </MenuItem>
      {onWithdraw && (
        <MenuItem icon={Undo2} onSelect={onWithdraw}>
          {t('withdraw')}
        </MenuItem>
      )}
    </Menu>
  );
}

const serverIcons: Record<Server, ComponentType<{ className?: string }>> = { simulation: FlaskConical, stage: TestTube, production: Globe };

/** The event's access key, and where the show sends (said, not chosen here): a show of
 *  equievents.ch from there (the simulation until the pilot), else the simulation here. A show
 *  of equievents.ch is also updated from Swiss Equestrian there. */
function ConnectionCard({ show, connection, changed }: { show: string; connection: Connection; changed: () => void }) {
  const t = useT();
  const refusal = useRefusal();
  const { refresh } = useChanges();
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [updating, setUpdating] = useState(false);
  const [updated, setUpdated] = useState<string | null>(null);
  const update = async () => {
    setUpdating(true);
    setError(null);
    setUpdated(null);
    try {
      const answer = await call<{ ok: boolean; code?: string; added?: number }>('update_from_se', { show });
      if (!answer.ok) return setError(answer.code === 'key' ? t('online_err_key_unknown') : refusal(answer.code ?? 'error'));
      setUpdated(answer.added ? t('se_update_done').replace('{n}', String(answer.added)) : t('se_update_none'));
      refresh();
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
    } finally {
      setUpdating(false);
    }
  };

  const set = async (args: Record<string, unknown>) => {
    try {
      setError(null);
      await call('set_connection', { show, ...args });
      changed();
    } catch (e) {
      setError(String((e as Error).message ?? e));
      throw e;
    }
  };
  const ServerIcon = connection.server ? serverIcons[connection.server] : Globe;
  const where = connection.rehearsal ? t('sends_rehearsal') : connection.online ? t('sends_online') : t('sends_here');
  if (connection.training)
    return (
      <Card className="flex flex-col overflow-hidden">
        <CardHeader icon={KeyRound} title={t('access_key')} />
        <div className="p-4 sm:p-5">
          <Callout tone="info" title={t('srv_simulation')}>
            {t('training_sending')}
          </Callout>
        </div>
      </Card>
    );
  return (
    <Card className="flex flex-col overflow-hidden">
      <CardHeader icon={KeyRound} title={t('access_key')} />
      <div className="flex flex-1 flex-col gap-4 p-4 sm:p-5">
        {error && <Callout tone="danger">{error}</Callout>}
        <p className="flex items-start gap-2 text-xs text-ink-3">
          <ServerIcon className="mt-px size-4 shrink-0" />
          <span>
            {where} {connection.server ? t(`sends_${connection.server}` as Key) : t('online_offline_long')}
          </span>
        </p>
        {connection.online && (
          <div className="flex flex-wrap items-center gap-3 rounded-xl border border-line bg-raised px-4 py-3">
            <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-card text-ink-2 ring-1 ring-line">
              <RefreshCw className="size-4" />
            </div>
            <div className="min-w-0 flex-1 basis-44">
              <div className="text-[13px] font-semibold text-ink">{t('se_update')}</div>
              <div className={`text-xs ${updated ? 'text-ok' : 'text-ink-3'}`}>{updated ?? t('se_update_hint')}</div>
            </div>
            <Button icon={RefreshCw} loading={updating} onClick={update}>
              {t('se_update_go')}
            </Button>
          </div>
        )}
        <div className="mt-auto flex flex-wrap items-center gap-3 rounded-xl border border-line bg-raised px-4 py-3">
          <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-card text-ink-2 ring-1 ring-line">
            <KeyRound className="size-4" />
          </div>
          <div className="min-w-0 flex-1 basis-44">
            <div className="text-[13px] font-semibold text-ink">{t('access_key')}</div>
            <div className="text-xs text-ink-3">
              {connection.has_key ? (
                <span className="inline-flex items-center gap-1.5">
                  <span className="font-mono">•••{connection.key_end ?? ''}</span>
                  <Pill tone="ok" icon={Check}>
                    {t('key_saved')}
                  </Pill>
                </span>
              ) : (
                <Pill tone="warn" dot>
                  {t('no_key')}
                </Pill>
              )}
            </div>
          </div>
          <Button variant={connection.has_key ? 'secondary' : 'primary'} icon={KeyRound} onClick={() => setEditing(true)}>
            {connection.has_key ? t('key_change') : t('key_enter')}
          </Button>
        </div>
      </div>
      <KeyModal open={editing} onClose={() => setEditing(false)} connection={connection} set={set} />
    </Card>
  );
}

/** Entering, changing or forgetting the event's access key. Not a secret (Rosson's exports
 *  carry it), so shown as typed; still no login for a password manager to save. */
function KeyModal({ open, onClose, connection, set }: { open: boolean; onClose: () => void; connection: Connection; set: (args: Record<string, unknown>) => Promise<void> }) {
  const t = useT();
  const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);
  const close = () => {
    setKey('');
    onClose();
  };
  const save = async (value: string | null) => {
    setBusy(true);
    try {
      await set({ access_key: value });
      close();
    } catch {
      // Shown on the card.
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      open={open}
      onClose={close}
      title={t('access_key')}
      description={t('access_key_hint')}
      icon={KeyRound}
      size="sm"
      footer={
        <>
          {connection.has_key && (
            <Button variant="danger" className="mr-auto" disabled={busy} onClick={() => save(null)}>
              {t('key_forget')}
            </Button>
          )}
          <Button variant="ghost" onClick={close}>
            {t('cancel')}
          </Button>
          <Button variant="primary" loading={busy} disabled={!key.trim()} onClick={() => save(key)}>
            {t('save')}
          </Button>
        </>
      }
    >
      <input
        value={key}
        autoComplete="off"
        spellCheck={false}
        data-1p-ignore
        data-lpignore="true"
        data-bwignore
        placeholder={connection.has_key ? `•••${connection.key_end ?? ''}` : ''}
        onChange={e => setKey(e.target.value)}
        onKeyDown={e => e.key === 'Enter' && key.trim() && save(key)}
        className="field font-mono"
      />
    </Modal>
  );
}

/** The registry of riders and horses this computer has: how many, of when; updating it. */
function RegistryCard({ show, about, downloaded }: { show: string; about: RegistryAbout; downloaded: () => void }) {
  const t = useT();
  const language = useLanguage();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const empty = about.persons === 0 && about.horses === 0;
  const at = about.persons_at ?? about.horses_at;
  const download = async () => {
    setBusy(true);
    setError(null);
    try {
      await call('registry_download', { show });
      downloaded();
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Card className="flex flex-col overflow-hidden">
      <CardHeader icon={Database} title={t('registry')} />
      <div className="flex flex-1 flex-col gap-4 p-4 sm:p-5">
        <p className="text-xs text-ink-3">{t('registry_hint')}</p>
        {error && <Callout tone="danger">{error}</Callout>}
        {empty ? (
          <Callout tone="info">{t('registry_none')}</Callout>
        ) : (
          <div className="grid grid-cols-2 gap-3">
            <Stat icon={UserRound} label={t('riders')} value={about.persons.toLocaleString(`${language}-CH`)} />
            <Stat icon={Horseshoe} label={t('horses')} value={about.horses.toLocaleString(`${language}-CH`)} />
          </div>
        )}
        <div className="mt-auto flex flex-wrap items-center gap-x-3 gap-y-2">
          <div className="min-w-0 flex-1 text-xs text-ink-3">
            {at && <div>{t('registry_at').replace('{d}', new Date(at).toLocaleDateString(`${language}-CH`))}</div>}
            {!empty && about.simulated && <div className="text-ink-4">{t('registry_simulated')}</div>}
          </div>
          <Button variant={empty ? 'primary' : 'secondary'} icon={RefreshCw} loading={busy} onClick={download}>
            {busy ? t('registry_loading') : empty ? t('registry_download') : t('registry_update')}
          </Button>
        </div>
      </div>
    </Card>
  );
}

/** What the simulation received, the latest first; a request opens in a dialog. */
function SimulationLog({ log }: { log: Received[] }) {
  const t = useT();
  const language = useLanguage();
  const [open, setOpen] = useState<number | null>(null);
  const r = open === null ? null : log[open];
  return (
    <Card className="overflow-hidden">
      <CardHeader icon={FlaskConical} title={t('received')} subtitle={t('simulation_hint')} />
      {log.length === 0 && <p className="px-5 py-4 text-sm text-ink-3">{t('nothing_received')}</p>}
      <ul>
        {log.map((x, i) => (
          <li key={`${x.millis}-${i}`} className="border-t border-line first:border-t-0">
            <button onClick={() => setOpen(i)} className="flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors hover:bg-hover sm:px-5">
              <span className="w-14 shrink-0 rounded-md bg-sunken py-0.5 text-center font-mono text-[11px] font-semibold text-ink-2">{x.method}</span>
              <span className="min-w-0 flex-1 truncate font-mono text-xs text-ink">/{x.path}</span>
              <span className="hidden shrink-0 text-xs text-ink-3 tabular sm:inline">{formatMillis(x.millis, language)}</span>
              <Pill tone={x.status < 300 ? 'ok' : 'danger'}>{x.status}</Pill>
            </button>
          </li>
        ))}
      </ul>
      <Modal open={r !== null} onClose={() => setOpen(null)} title={r ? `${r.method} /${r.path}` : ''} description={r ? formatMillis(r.millis, language) : undefined} icon={Radio} size="lg">
        {r && (
          <pre className="overflow-auto rounded-xl bg-sunken p-4 text-[11px] leading-snug">
            {r.query.length > 0 && `?${r.query.map(([k, v]) => `${k}=${v}`).join('&')}\n\n`}
            {r.body ? JSON.stringify(r.body, null, 2) : ''}
            {`\n\n→ ${r.status} ${JSON.stringify(r.answer, null, 2)}`}
          </pre>
        )}
      </Modal>
    </Card>
  );
}
