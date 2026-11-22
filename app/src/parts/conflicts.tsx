// What was changed on two computers apart (the show's conflicts, docs/architecture/
// reconciliation.md R2, R4, R7, R11, R12). After a sync the office is told: what was kept and what it
// replaced, by whom and when; the other version can be put back. What was looked at ("Compris")
// is this computer's own: each person sees what concerns them.

import { useCallback, useEffect, useRef, useState } from 'react';
import { GitMerge } from 'lucide-react';
import { call } from '../api';
import { useLanguage, useT, type Key } from '../i18n';
import { Button, Modal, Tooltip, useToast } from '../ui';
import { useChanges } from './changes';

export interface ChangeBy {
  author: string;
  /** Made on this computer. */
  here: boolean;
  millis: number;
}

export interface ConflictItem {
  field: string;
  rider: string;
  kept: string;
  replaced: string;
}

export interface ConflictView {
  id: string;
  kind: 'class' | 'order' | 'numbers' | 'duplicates' | 'result' | 'late_final' | 'class_gone' | 'change' | 'change_gone' | 'basis' | 'late_upload' | 'delivered' | 'stranded';
  class: string;
  class_label: string;
  kept: ChangeBy | null;
  replaced: ChangeBy | null;
  items: ConflictItem[];
  count: number;
  /** R13: what the change made without seeing (as `items`' fields name it). */
  done: string;
  restorable: boolean;
  seen: boolean;
}

/** A class's field, as the settings name it. */
const FIELDS: Record<string, Key> = {
  name: 'class_name',
  date: 'class_day',
  time_text: 'time',
  scoring_table: 'scoring_label',
  programme: 'programme',
  sponsor: 'sponsor_labels',
  position: 'conflict_field_position',
  teams: 'conflict_field_teams',
  'options.placed_count': 'placed_count',
  'options.max_starters': 'max_starters',
  'options.ideal_time': 'ideal_time',
};

/** Anything else changed on two computers (R12): what it is. */
const THINGS: Record<string, Key> = {
  horse: 'horse',
  rider: 'rider',
  status: 'conflict_thing_status',
  hors_concours: 'hors_concours',
  published: 'conflict_thing_published',
  stage: 'conflict_thing_stage',
  comment: 'conflict_thing_comment',
  horse_phases: 'conflict_thing_horse_phases',
  entry: 'conflict_thing_entry',
  assignment: 'official',
  address: 'conflict_thing_address',
  team: 'team',
  event_numbers: 'conflict_thing_event_numbers',
  prizes: 'conflict_thing_prizes',
  category_text: 'conflict_thing_category_text',
  levels: 'conflict_thing_levels',
  programme: 'programme',
  scoring_table: 'scoring_label',
  combined: 'conflict_thing_combined',
  class: 'conflict_thing_class',
  class_status: 'conflict_thing_class_status',
  rank: 'rank',
  jury_penalty: 'conflict_thing_jury_penalty',
  card: 'conflict_thing_card',
  sheet: 'conflict_thing_sheet',
  penalties: 'penalties',
  total: 'total',
  // R13: what a change relied on.
  class_setup: 'conflict_thing_class_setup',
  judge: 'conflict_thing_judge',
  entry_results: 'conflict_thing_entry_results',
  judge_sheets: 'conflict_thing_judge_sheets',
  class_results: 'conflict_thing_class_results',
  // R8: what only Swiss Equestrian's deliveries set.
  fee_paid: 'conflict_thing_fee_paid',
  winning_points: 'conflict_thing_winning_points',
  entered_by: 'conflict_thing_entered_by',
};

/** In the show's bar (beside its sync state): changes made on two computers, not looked at yet. */
export function ConflictsNotice({ show, look }: { show: string; look: 'side' | 'bar' | 'icon' }) {
  const t = useT();
  const { version } = useChanges();
  const { show: note } = useToast();
  const [list, setList] = useState<ConflictView[]>([]);
  const [open, setOpen] = useState(false);
  const known = useRef<number | null>(null);

  const load = useCallback(
    () =>
      call<{ unseen: number; conflicts: ConflictView[] }>('conflicts', { show }).then(
        a => setList(a.conflicts.filter(c => !c.seen)),
        () => {},
      ),
    [show],
  );
  useEffect(() => {
    load();
  }, [load, version]);
  // New ones (a sync brought them): said once.
  useEffect(() => {
    const n = list.length;
    if (known.current !== null && n > known.current) {
      note({ text: n === 1 ? t('conflicts_one') : t('conflicts_many').replace('{n}', String(n)), icon: GitMerge, tone: 'info', action: { label: t('conflicts_see'), onClick: () => setOpen(true) } });
    }
    known.current = n;
  }, [list.length, note, t]);

  if (list.length === 0) return null;
  const label = list.length === 1 ? t('conflicts_one') : t('conflicts_many').replace('{n}', String(list.length));
  const short = t('conflicts_short').replace('{n}', String(list.length));
  return (
    <>
      {look === 'side' ? (
        <button
          onClick={() => setOpen(true)}
          title={label}
          className="flex h-9 w-full items-center gap-2.5 rounded-lg bg-warn px-2.5 text-[13px] font-semibold whitespace-nowrap text-white hover:brightness-110"
        >
          <GitMerge className="size-4 shrink-0" />
          <span className="flex-1 truncate text-left">{short}</span>
        </button>
      ) : (
        <Tooltip label={label} side={look === 'icon' ? 'right' : 'bottom'}>
          <button onClick={() => setOpen(true)} aria-label={label} className="relative grid size-9 place-items-center rounded-lg bg-warn text-white hover:brightness-110">
            <GitMerge className="size-4" />
            <span className="absolute -top-1 -right-1 min-w-4 rounded-full bg-card px-1 text-center text-[10px] leading-4 font-semibold text-warn tabular">{list.length}</span>
          </button>
        </Tooltip>
      )}
      <ConflictsDialog show={show} list={list} open={open} close={() => setOpen(false)} reload={load} />
    </>
  );
}

function ConflictsDialog({ show, list, open, close, reload }: { show: string; list: ConflictView[]; open: boolean; close: () => void; reload: () => Promise<void> }) {
  const t = useT();
  const done = async () => {
    try {
      await call('acknowledge_conflicts', { show, ids: list.map(c => c.id) });
    } finally {
      await reload();
      close();
    }
  };
  return (
    <Modal
      open={open && list.length > 0}
      onClose={close}
      title={t('conflicts_title')}
      description={t('conflicts_intro')}
      icon={GitMerge}
      tone="warn"
      size="lg"
      footer={
        <>
          <Button variant="ghost" onClick={close}>
            {t('later')}
          </Button>
          <Button variant="primary" onClick={done}>
            {t('conflicts_ok')}
          </Button>
        </>
      }
    >
      <ul className="grid gap-3">
        {list.map(c => (
          <ConflictCard key={c.id} conflict={c} reload={reload} />
        ))}
      </ul>
    </Modal>
  );
}

function ConflictCard({ conflict: c, reload }: { conflict: ConflictView; reload: () => Promise<void> }) {
  const t = useT();
  const language = useLanguage();
  const { change } = useChanges();
  const [busy, setBusy] = useState(false);
  const when = (millis: number) => new Date(millis).toLocaleString(`${language}-CH`, { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });
  const who = (b: ChangeBy) => `${b.author}${b.here ? ` (${t('conflict_here')})` : ''} · ${when(b.millis)}`;
  const text: Partial<Record<ConflictView['kind'], Key>> = {
    order: 'conflict_order_text',
    numbers: 'conflict_numbers_text',
    duplicates: 'conflict_duplicates_text',
    late_final: 'conflict_late_final_text',
    class_gone: 'conflict_class_gone_text',
    change_gone: 'conflict_change_gone_text',
    basis: 'conflict_basis_text',
    late_upload: 'conflict_late_upload_text',
    delivered: 'conflict_delivered_text',
  };
  const restore = async () => {
    setBusy(true);
    await change('restore_conflict', { id: c.id });
    setBusy(false);
    await reload();
  };
  const fieldName = (f: string) => t(FIELDS[f] ?? 'conflict_field_other');
  // R12: what it is (a mark: its number), and its value as the lists say it.
  const thingName = (f: string) => (f.startsWith('mark:') ? t('conflict_thing_mark').replace('{n}', f.slice(5)) : THINGS[f] ? t(THINGS[f]) : f);
  const VALUES: Record<string, Key> = {
    Entered: 'conflict_value_entered',
    NotStarting: 'conflict_value_not_starting',
    Final: 'conflict_value_final',
    true: 'hors_concours',
    false: 'conflict_value_in_competition',
  };
  const value = (v: string) => (VALUES[v] ? t(VALUES[v]) : v);
  const changed = c.kind === 'change' || c.kind === 'change_gone' || c.kind === 'basis' || c.kind === 'delivered';
  const table = c.kind === 'class' || c.kind === 'numbers' || c.kind === 'result' || c.kind === 'change' || c.kind === 'delivered';
  // What the two changes were to each other: kept and replaced, or not.
  const keptLabel: Partial<Record<ConflictView['kind'], Key>> = { late_final: 'conflict_set_aside', change_gone: 'conflict_set_aside', basis: 'conflict_done', late_upload: 'conflict_sent' };
  const replacedLabel: Partial<Record<ConflictView['kind'], Key>> = { change_gone: 'conflict_earlier', basis: 'conflict_unseen', late_upload: 'conflict_earlier' };
  const more = c.count - c.items.length;
  return (
    <li className="grid gap-2.5 rounded-xl border border-line bg-card px-4 py-3">
      <div className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
        <span className="text-[13px] font-semibold text-ink">{c.kind === 'change' && c.items.length === 1 ? thingName(c.items[0].field) : t(`conflict_${c.kind}` as Key)}</span>
        <span className="min-w-0 truncate text-[13px] text-ink-3">{c.class_label}</span>
      </div>
      {(c.kept || c.replaced) && (
        <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-xs">
          {c.kept && (
            <>
              <dt className="font-semibold text-ink-2">{t(keptLabel[c.kind] ?? 'conflict_kept')}</dt>
              <dd className="min-w-0 truncate text-ink-2">
                {c.done && <span className="font-semibold text-ink">{thingName(c.done)} · </span>}
                {who(c.kept)}
              </dd>
            </>
          )}
          {c.replaced && (
            <>
              <dt className="font-semibold text-ink-2">{t(replacedLabel[c.kind] ?? 'conflict_replaced')}</dt>
              <dd className="min-w-0 truncate text-ink-2">{who(c.replaced)}</dd>
            </>
          )}
        </dl>
      )}
      {text[c.kind] && <p className="text-xs text-ink-2">{t(text[c.kind]!)}</p>}
      {c.kind === 'stranded' && <p className="text-xs text-ink-2">{t('conflict_stranded_text').replace('{n}', String(c.count))}</p>}
      {c.kind === 'late_upload' && <p className="text-xs font-semibold text-ink">{c.count === 1 ? t('conflict_changes_one') : t('conflict_changes_many').replace('{n}', String(c.count))}</p>}
      {table && c.items.length > 0 && (
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-left text-ink-3">
                <th className="py-1 pr-3 font-semibold" />
                <th className="w-1/4 py-1 pr-3 font-semibold">{t('conflict_kept')}</th>
                <th className="w-1/4 py-1 font-semibold">{t('conflict_replaced')}</th>
              </tr>
            </thead>
            <tbody>
              {c.items.map((i, k) => (
                <tr key={k} className="border-t border-line">
                  <td className="py-1.5 pr-3 text-ink-2">
                    {c.kind === 'class' ? fieldName(i.field) : changed ? thingName(i.field) : i.rider}
                    {changed && i.rider && i.rider !== c.class_label && <span className="block text-ink-3">{i.rider}</span>}
                  </td>
                  <td className="py-1.5 pr-3 font-semibold text-ink tabular">{(changed ? value(i.kept) : i.kept) || '—'}</td>
                  <td className="py-1.5 text-ink-3 tabular line-through decoration-ink-4">{(changed ? value(i.replaced) : i.replaced) || '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {!table && c.items.length > 0 && (
        <ul className="grid gap-0.5 text-xs text-ink-2">
          {c.items.map((i, k) => (
            <li key={k} className="truncate">
              {c.kind === 'duplicates' && <span className="mr-1.5 font-semibold text-ink tabular">{i.kept}</span>}
              {c.kind === 'change_gone' || c.kind === 'basis' ? [thingName(i.field), i.rider !== c.class_label ? i.rider : ''].filter(Boolean).join(' · ') : i.rider}
              {c.kind === 'basis' && i.kept && (
                <span className="text-ink-3">
                  {' — '}
                  {t('conflict_now')}
                  {t('colon')}
                  <span className="font-semibold text-ink">{value(i.kept)}</span>
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
      {more > 0 && <p className="text-xs text-ink-3">{t('conflict_others').replace('{n}', String(more))}</p>}
      {c.restorable && c.replaced && (
        <div>
          <Button size="sm" onClick={restore} disabled={busy}>
            {c.replaced.here ? t('conflict_restore_mine') : t('conflict_restore_theirs').replace('{who}', c.replaced.author)}
          </Button>
        </div>
      )}
    </li>
  );
}
