// Rankings over several classes (U8; Rosson's finals): listed on the classes page, set up in a
// dialog, each with its own page. Penalties added (jumping; a tie: ex aequo, or the penalties or
// the result in a class picked for it), ranks added (jumping, dressage or both; a tie: the rank in a class picked for it, then the
// others) or the mean percentage (dressage; a tie: the percentage in a class picked for it, then
// the others).

import { useState, type ReactNode } from 'react';
import { ArrowLeft, ChevronRight, CloudUpload, Ellipsis, Pencil, Plus, Printer, Sigma, Star, Trash2, Trophy } from 'lucide-react';
import { call, type ClassRow, type CombinedRow, type CombinedView, type PenaltiesTie, type ShowView } from '../api';
import { href, type Place } from '../App';
import { formatDate, formatMillis, useLanguage, useReason, useT } from '../i18n';
import { useChanges } from '../parts/changes';
import { ErrorNote, useLoad } from '../parts/common';
import { usePrint } from '../parts/print';
import { Button, Card, ConfirmModal, EmptyState, Field, IconButton, Menu, MenuItem, MenuSeparator, Modal, PageHeader, Pill } from '../ui';

type Go = (place: Place) => void;

/** The show's jumping and dressage classes, in the order they are ridden. */
const rankableClasses = (show: ShowView): ClassRow[] => show.days.flatMap(d => d.classes).filter(c => c.discipline === 'CS' || c.discipline === 'CD');

/** How a ranking adds up and what breaks its ties, in words. */
function useMethodName() {
  const t = useT();
  return (r: CombinedRow) => {
    const tie = r.classes.find(c => c.id === r.tie_break)?.number ?? '';
    const ties: Record<string, string> = {
      none: t('tie_none'),
      faults: `${t('tie_faults_before')} ${tie} ${t('tie_faults_after')}`,
      result: `${t('tie_result_before')} ${tie} ${t('tie_result_after')}`,
      rank: `${t('tie_rank_before')} ${tie} ${t('tie_rank_after')}`,
      percentage: `${t('tie_percent_before')} ${tie} ${t('tie_percent_after')}`,
    };
    const method = r.method === 'penalties' ? 'method_penalties' : r.method === 'percentages' ? 'method_percentages' : 'method_ranks';
    return `${t(method)} · ${t('tie_label')} ${ties[r.tie] ?? ''}`;
  };
}

/** The rankings over several classes (rows, in a card of the classes page). */
export function CombinedList({ show, go, onNew }: { show: ShowView; go: Go; onNew?: () => void }) {
  const t = useT();
  const methodName = useMethodName();
  const possible = rankableClasses(show).length >= 2;
  if (show.combined.length === 0)
    return (
      <EmptyState
        icon={Sigma}
        title={t('combined_none')}
        action={
          possible && onNew ? (
            <Button variant="primary" icon={Plus} onClick={onNew}>
              {t('new_combined')}
            </Button>
          ) : undefined
        }
      >
        {possible ? t('combined_rule') : t('combined_needs_two')}
      </EmptyState>
    );
  return (
    <div>
      {show.combined.map(r => (
        <button
          key={r.id}
          onClick={() => go({ show: show.id, view: 'combined', class: r.id })}
          className="group flex w-full items-center gap-3 border-t border-line px-4 py-3 text-left first:border-t-0 hover:bg-hover"
        >
          <span className="grid size-10 shrink-0 place-items-center rounded-xl bg-accent-soft text-accent">
            <Trophy className="size-5" />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[14px] font-semibold">{r.name}</span>
            <span className="block truncate text-xs text-ink-3">
              {r.classes.map(c => c.number).join(' + ')} · {methodName(r)}
            </span>
          </span>
          <ChevronRight className="size-4 text-ink-4 group-hover:text-ink-2" />
        </button>
      ))}
    </div>
  );
}

/** Setting up a new ranking over several classes. */
export function NewCombined({ show, open, onClose, go }: { show: ShowView; open: boolean; onClose: () => void; go: Go }) {
  const t = useT();
  return (
    <Modal open={open} onClose={onClose} title={t('new_combined')} icon={Sigma} size="lg">
      {open && (
        <CombinedForm
          show={show}
          done={id => {
            onClose();
            if (id) go({ show: show.id, view: 'combined', class: id });
          }}
        />
      )}
    </Modal>
  );
}

/** Set up or change a ranking over several classes. `done`: with the ranking's id once saved. */
function CombinedForm({ show, ranking, done }: { show: ShowView; ranking?: CombinedRow; done: (id?: string) => void }) {
  const t = useT();
  const language = useLanguage();
  const { change } = useChanges();
  const all = rankableClasses(show);
  const [name, setName] = useState(ranking?.name ?? '');
  const [chosen, setChosen] = useState<Set<string>>(new Set(ranking?.classes.map(c => c.id) ?? []));
  const [picked, setMethod] = useState<CombinedRow['method']>(ranking?.method ?? 'penalties');
  const [tieBreak, setTieBreak] = useState(ranking?.tie_break ?? '');
  // Penalties added: what breaks a tie (nothing, ex aequo, unless chosen).
  const [tieBy, setTieBy] = useState<PenaltiesTie>(ranking?.method === 'penalties' ? (ranking.tie as PenaltiesTie) : 'none');
  const classes = all.filter(c => chosen.has(c.id));
  // Penalties for jumping classes, percentages for dressage ones, ranks for any.
  const jumping = classes.every(c => c.discipline === 'CS');
  const dressage = classes.every(c => c.discipline === 'CD');
  const allowed = (m: CombinedRow['method']) => m === 'ranks' || (m === 'penalties' ? jumping : dressage);
  const method = allowed(picked) ? picked : dressage ? 'percentages' : 'ranks';
  const tie = classes.some(c => c.id === tieBreak) ? tieBreak : (classes[classes.length - 1]?.id ?? '');
  // A class breaks ties: ranks, or penalties by its penalties or its result.
  const byClass = method === 'ranks' || method === 'percentages' || (method === 'penalties' && (tieBy === 'faults' || tieBy === 'result'));
  const ready = name.trim() !== '' && classes.length >= 2 && (!byClass || tie !== '');
  // Its number, never typed: the highest class's with ".1" (".2" when taken…), kept while its
  // classes stay.
  const ids = classes.map(c => c.id);
  const number = useLoad(() => (ids.length ? call<string>('combined_number', { show: show.id, id: ranking?.id, classes: ids }) : Promise.resolve('')), [show.id, ranking?.id, ids.join(',')]);

  const save = async () => {
    const answer = await change('set_combined', {
      id: ranking?.id,
      name,
      classes: classes.map(c => c.id),
      method,
      tie: method === 'penalties' ? tieBy : undefined,
      tie_break: byClass ? tie : undefined,
    });
    if (answer) done(String(answer.id ?? ranking?.id));
  };
  const toggle = (id: string) =>
    setChosen(s => {
      const next = new Set(s);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  // The class that breaks ties, by its number and category; picking one picks its choice.
  const tieClass = (pick: () => void) => (
    <select
      value={tie}
      disabled={classes.length === 0}
      onChange={e => {
        setTieBreak(e.target.value);
        pick();
      }}
      className="field field-sm w-auto max-w-full"
    >
      {classes.length === 0 && <option value="">{t('tie_pick')}</option>}
      {classes.map(c => (
        <option key={c.id} value={c.id}>
          {c.number} · {c.category}
        </option>
      ))}
    </select>
  );
  // A choice: a radio and its sentence, the sentence's words and a class picker on one line
  // (wrapping on a phone), the radio level with it.
  const choice = (name: string, checked: boolean, disabled: boolean, onPick: () => void, words: ReactNode) => (
    <label className={`flex min-h-8 items-center gap-3 ${disabled ? '' : 'cursor-pointer'}`}>
      <input type="radio" name={name} className="check shrink-0" disabled={disabled} checked={checked} onChange={onPick} />
      <span className="flex min-w-0 flex-1 flex-wrap items-center gap-x-1.5 gap-y-1 text-[13px]">{words}</span>
    </label>
  );
  const card = (m: CombinedRow['method'], children: ReactNode) => (
    <div
      className={`rounded-xl border px-3 py-2 transition-colors ${method === m ? 'border-accent bg-accent-soft' : 'border-line hover:bg-hover'} ${allowed(m) ? '' : 'pointer-events-none opacity-40'}`}
    >
      {children}
    </div>
  );
  const tieChoice = (by: PenaltiesTie, words: ReactNode) => choice('tie', tieBy === by, false, () => setTieBy(by), words);

  return (
    <div className="grid gap-5">
      <div className="grid gap-3 sm:grid-cols-[1fr_auto]">
        <Field label={t('combined_name')}>
          <input data-autofocus value={name} onChange={e => setName(e.target.value)} className="field" />
        </Field>
        <Field label={t('combined_number')}>
          <span className="flex h-9 min-w-20 items-center rounded-lg bg-sunken px-3 text-[13px] font-semibold tabular text-ink-2 pointer-coarse:h-11">{number.data || '–'}</span>
        </Field>
      </div>
      <Field label={t('combined_classes')}>
        <div className="flex flex-wrap gap-1.5">
          {all.map(c => (
            <button
              key={c.id}
              type="button"
              onClick={() => toggle(c.id)}
              title={`${formatDate(c.date, language)} · ${c.category} · ${c.scoring}`}
              className={`rounded-lg border px-2.5 py-1.5 text-xs transition-colors pointer-coarse:py-2.5 ${chosen.has(c.id) ? 'border-accent bg-accent text-on-accent shadow-xs' : 'border-line-strong bg-card hover:bg-hover'}`}
            >
              <span className="font-bold">{c.number}</span> {c.category}
            </button>
          ))}
        </div>
      </Field>
      <div className="grid gap-2">
        {card(
          'penalties',
          <>
            {choice('method', method === 'penalties', !allowed('penalties'), () => setMethod('penalties'), <span className="font-semibold">{t('method_penalties')}</span>)}
            {/* Its tie-break, once chosen. */}
            {method === 'penalties' && (
              <div className="grid gap-0.5 pb-1 pl-7">
                <span className="text-xs font-semibold text-ink-3">{t('tie_label')}</span>
                {tieChoice('none', t('tie_none'))}
                {tieChoice(
                  'faults',
                  <>
                    <span>{t('tie_faults_before')}</span>
                    {tieClass(() => setTieBy('faults'))}
                    <span>{t('tie_faults_after')}</span>
                  </>,
                )}
                {tieChoice(
                  'result',
                  <>
                    <span>{t('tie_result_before')}</span>
                    {tieClass(() => setTieBy('result'))}
                    <span>{t('tie_result_after')}</span>
                  </>,
                )}
              </div>
            )}
          </>,
        )}
        {card(
          'percentages',
          choice(
            'method',
            method === 'percentages',
            !allowed('percentages'),
            () => setMethod('percentages'),
            <>
              <span>
                <span className="font-semibold">{t('method_percentages')}</span> · {t('tie_label')} {t('tie_percent_before')}
              </span>
              {tieClass(() => setMethod('percentages'))}
              <span>{t('tie_percent_after')}</span>
            </>,
          ),
        )}
        {card(
          'ranks',
          choice(
            'method',
            method === 'ranks',
            !allowed('ranks'),
            () => setMethod('ranks'),
            <>
              <span>
                <span className="font-semibold">{t('method_ranks')}</span> · {t('tie_label')} {t('tie_rank_before')}
              </span>
              {tieClass(() => setMethod('ranks'))}
              <span>{t('tie_rank_after')}</span>
            </>,
          ),
        )}
      </div>
      <p className="text-xs text-ink-3">{t('combined_rule')}</p>
      <div className="flex justify-end gap-2 border-t border-line pt-4">
        <Button variant="ghost" onClick={() => done()}>
          {t('cancel')}
        </Button>
        <Button variant="primary" disabled={!ready} onClick={save}>
          {t('save')}
        </Button>
      </div>
    </div>
  );
}

/** A ranking over several classes: each class's result, the total, what broke ties. */
export function CombinedScreen({ show, id, go }: { show: ShowView; id: string; go: Go }) {
  const t = useT();
  const reason = useReason();
  const language = useLanguage();
  const print = usePrint(show.id);
  const { version, change } = useChanges();
  const view = useLoad(() => call<CombinedView>('combined', { show: show.id, id }), [show.id, id, version]);
  const [editing, setEditing] = useState(false);
  const [removing, setRemoving] = useState(false);
  const methodName = useMethodName();
  const r = view.data?.ranking;
  const penalties = r?.method === 'penalties';
  const percentages = r?.method === 'percentages';
  // The tie-break apart: the rank in a class (the class picked for penalties shows its own,
  // starred; ex aequo has none).
  const tieColumn = r?.tie === 'rank';
  const percent = (p: string | null) => (p === null ? '' : `${Number(p).toFixed(2)} %`);
  const th = 'border-b border-line bg-raised px-3 py-2.5 text-xs font-semibold text-ink-3';
  const td = 'border-b border-line px-3 py-2 align-top';
  const ranked = view.data?.lines.some(l => l.rank !== null) ?? false;

  return (
    <div className="pane">
      <div className="mx-auto max-w-[1180px] space-y-5 px-3 py-5 sm:px-6 sm:py-7">
        <a href={href({ show: show.id, view: 'classes' })} className="inline-flex items-center gap-1.5 text-[13px] font-semibold text-ink-3 hover:text-ink">
          <ArrowLeft className="size-4" />
          {t('classes')}
        </a>
        {r && (
          <PageHeader
            icon={Trophy}
            title={r.name}
            subtitle={`${r.classes.map(c => c.number).join(' + ')} · ${methodName(r)} · ${r.number}`}
            actions={
              <>
                <Button icon={Printer} onClick={() => print({ what: 'combined', id })}>
                  {t('print')}
                </Button>
                {/* Its results to Swiss Equestrian, numbered as the ranking (Rosson's finals). */}
                {ranked && view.data && (
                  <Button
                    variant={!view.data.sent || view.data.changed ? 'primary' : 'secondary'}
                    icon={CloudUpload}
                    onClick={() => change('send', { what: 'combined', id, language })}
                    title={view.data.changed ? t('changed_since') : view.data.sent ? `${t('sent')} ${formatMillis(view.data.sent, language)}` : undefined}
                  >
                    {view.data.sent ? t('resend') : t('send_results')}
                  </Button>
                )}
                <Menu trigger={<IconButton icon={Ellipsis} label={t('actions_more')} variant="secondary" />}>
                  <MenuItem icon={Pencil} onSelect={() => setEditing(true)}>
                    {t('edit')}
                  </MenuItem>
                  <MenuSeparator />
                  <MenuItem icon={Trash2} danger onSelect={() => setRemoving(true)}>
                    {t('delete')}
                  </MenuItem>
                </Menu>
              </>
            }
          />
        )}
        <ErrorNote error={view.error} />
        {view.data && r && (
          <Card className="overflow-hidden">
            <div className="overflow-x-auto">
              <table className="w-full text-[13px]">
                <thead>
                  <tr>
                    <th className={`${th} w-14 text-right`}>{t('rank')}</th>
                    <th className={`${th} text-left`}>{t('rider')}</th>
                    {r.classes.map(c => (
                      <th key={c.id} className={`${th} border-l text-right`} title={c.id === r.tie_break ? t('tie_break') : undefined}>
                        <span className="inline-flex items-center gap-1">
                          {c.id === r.tie_break && <Star className="size-3 text-warn" />}
                          {c.number}
                        </span>
                      </th>
                    ))}
                    <th className={`${th} border-l text-right`}>{t('total')}</th>
                    {tieColumn && <th className={`${th} text-right`}>{t('tie_break')}</th>}
                  </tr>
                </thead>
                <tbody className="tabular">
                  {view.data.lines.map((l, i) => (
                    <tr key={i} className="hover:bg-hover">
                      <td className={`${td} text-right text-[15px] font-bold ${l.placed ? 'text-accent' : ''}`} title={l.placed ? t('placed') : undefined}>
                        {l.rank ?? '–'}
                      </td>
                      <td className={`${td} max-w-0 min-w-44`}>
                        <div className="truncate font-semibold">{l.rider}</div>
                        <div className="truncate text-xs text-ink-3">{l.horse}</div>
                      </td>
                      {l.cells.map((c, j) => (
                        <td key={j} className={`${td} border-l text-right whitespace-nowrap`}>
                          {c.missing ? (
                            <span className="text-xs text-ink-4">–</span>
                          ) : c.rank === null ? (
                            <Pill tone="warn">{c.out ? reason(c.out) : t('out')}</Pill>
                          ) : percentages ? (
                            <>
                              <span className="font-semibold">{percent(c.percentage)}</span>
                              <span className="block text-xs text-ink-3">{c.rank}.</span>
                            </>
                          ) : penalties ? (
                            <>
                              <span className="font-semibold">{c.faults}</span>
                              <span className="block text-xs text-ink-3">{c.time === null ? '' : Number(c.time).toFixed(2)}</span>
                            </>
                          ) : (
                            <>
                              <span className="font-semibold">{c.rank}.</span>
                              <span className="block text-xs text-ink-3">{c.percentage !== null ? percent(c.percentage) : c.faults}</span>
                            </>
                          )}
                        </td>
                      ))}
                      <td className={`${td} border-l text-right text-[14px] font-bold`}>{percentages ? percent(l.total) : (l.total ?? '')}</td>
                      {tieColumn && <td className={`${td} text-right text-ink-2`}>{l.tie_break === null ? '' : penalties ? Number(l.tie_break).toFixed(2) : `${l.tie_break}.`}</td>}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {view.data.lines.length === 0 && <EmptyState icon={Sigma} title={t('combined_rule')} />}
          </Card>
        )}
      </div>
      {r && (
        <Modal open={editing} onClose={() => setEditing(false)} title={r.name} icon={Pencil} size="lg">
          {editing && <CombinedForm show={show} ranking={r} done={() => setEditing(false)} />}
        </Modal>
      )}
      <ConfirmModal
        open={removing}
        onClose={() => setRemoving(false)}
        title={`${t('delete')} · ${r?.name ?? ''}`}
        message={t('combined_delete_ask')}
        confirm={t('delete')}
        danger
        icon={Trash2}
        onConfirm={async () => {
          setRemoving(false);
          if (await change('remove_combined', { id })) go({ show: show.id, view: 'classes' });
        }}
      />
    </div>
  );
}
