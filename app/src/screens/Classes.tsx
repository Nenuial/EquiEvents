// The show's classes: the programme day by day (each class one row: what it is, how far it is,
// what was sent), the published classes still to prepare, and the rankings over several
// classes. A class opens on its own page; anything else opens in a dialog.

import { useState, type ReactNode } from 'react';
import { ArrowRight, CalendarDays, CheckCheck, ChevronRight, CircleCheck, ClipboardList, Copy, Hash, Layers, ListChecks, Lock, Plus, Printer, Sigma, TriangleAlert, Trophy, Users } from 'lucide-react';
import { openPrint, type ClassRow, type EventView, type ShowView } from '../api';
import type { Place } from '../App';
import { formatDate, formatMillis, useLanguage, usePlural, useT, type Language } from '../i18n';
import { useChanges } from '../parts/changes';
import { phaseName } from '../parts/overall';
import { SERIES, sizeWarning } from '../parts/sections';
import { DropMark, Grip, SortableGroup, SortableGroups, SortRow } from '../parts/sortable';
import { Button, Card, CardHeader, ConfirmModal, EmptyState, Field, Modal, PageHeader, Pill, Switch, Tabs, Tooltip, useToast } from '../ui';
import { CombinedList, NewCombined } from './Combined';

type Go = (place: Place) => void;
type Tab = 'programme' | 'prepare' | 'combined';

/** Where a class stands, as a pill. */
export function ClassStatus({ row }: { row: ClassRow }) {
  const t = useT();
  if (row.status === 'final')
    return (
      <Pill tone="ok" icon={Lock}>
        {t('status_final')}
      </Pill>
    );
  if (row.status === 'running' || row.results > 0)
    return (
      <Pill tone="warn" dot>
        {t('status_running')}
      </Pill>
    );
  return <Pill dot>{t('status_planned')}</Pill>;
}

/** What Swiss Equestrian has of a class: its start lists, its results (changed since?). */
export function SentMarks({ row, language }: { row: ClassRow; language: Language }) {
  const t = useT();
  return (
    <span className="inline-flex items-center gap-1">
      {row.start_lists_sent && (
        <Tooltip label={`${t('start_lists_sent')} ${formatMillis(row.start_lists_sent, language)}${row.start_lists_changed ? ` · ${t('changed_since')}` : ''}`}>
          <span className={`grid size-6 place-items-center rounded-md ${row.start_lists_changed ? 'bg-warn/12 text-warn' : 'bg-info/10 text-info'}`}>
            <ListChecks className="size-3.5" />
          </span>
        </Tooltip>
      )}
      {row.results_sent && (
        <Tooltip label={`${t('results_sent')} ${formatMillis(row.results_sent, language)}${row.results_changed ? ` · ${t('changed_since')}` : ''}`}>
          <span className={`grid size-6 place-items-center rounded-md ${row.results_changed ? 'bg-warn/12 text-warn' : 'bg-ok/10 text-ok'}`}>
            <CheckCheck className="size-3.5" />
          </span>
        </Tooltip>
      )}
    </span>
  );
}

/** Results entered against starters, as a bar. */
function Progress({ row }: { row: ClassRow }) {
  const starting = Math.max(0, row.starters - row.not_starting);
  if (!starting) return <span className="text-xs text-ink-4">—</span>;
  const share = Math.min(1, row.results / starting);
  return (
    <div className="flex items-center gap-2">
      <div className="h-1.5 w-16 overflow-hidden rounded-full bg-sunken">
        <div className={`h-full rounded-full ${share >= 1 ? 'bg-ok' : 'bg-accent'}`} style={{ width: `${share * 100}%` }} />
      </div>
      <span className="text-xs text-ink-3 tabular">
        {row.results}/{starting}
      </span>
    </div>
  );
}

/** The number of a class, as a tile. */
export function ClassNumber({ number, size = 'md' }: { number: string; size?: 'md' | 'lg' }) {
  return (
    <span
      className={`grid shrink-0 place-items-center rounded-xl bg-gradient-to-b from-raised to-sunken font-bold text-ink ring-1 ring-line tabular ${size === 'lg' ? 'size-12 text-lg' : 'size-10 text-[15px]'}`}
    >
      {number}
    </span>
  );
}

/** A class that is a part of another (a série, a phase), under it: its tile, its title, and
 *  whether it is the last part (the tree's line stops there). */
interface Part {
  tile: ReactNode;
  title: ReactNode;
  last: boolean;
}

/** The tree's line between a class and its parts: from the parent's tile down, then to the part. */
function Branch({ last }: { last: boolean }) {
  return (
    <span className="relative -my-3 ml-5 w-5 shrink-0 self-stretch" aria-hidden>
      <span className="absolute top-0 left-0 h-1/2 w-full rounded-bl-lg border-b-2 border-l-2 border-line-strong/70" />
      {!last && <span className="absolute top-1/2 bottom-0 left-0 border-l-2 border-line-strong/70" />}
    </span>
  );
}

/** A tile with the tree's line under it, to its parts. */
function Trunk({ children }: { children: ReactNode }) {
  return (
    <span className="relative shrink-0">
      {children}
      <span className="absolute top-full left-1/2 h-3 -translate-x-px border-l-2 border-line-strong/70" aria-hidden />
    </span>
  );
}

function ClassLine({
  row,
  show,
  go,
  language,
  grip,
  dragging,
  part,
  parts,
}: {
  row: ClassRow;
  show: string;
  go: Go;
  language: Language;
  grip?: ReactNode;
  dragging?: boolean;
  part?: Part;
  parts?: boolean;
}) {
  const t = useT();
  const open = () => go({ show, view: 'classes', class: row.id });
  return (
    <div
      role="link"
      tabIndex={0}
      onClick={open}
      onKeyDown={e => e.key === 'Enter' && open()}
      className={`group flex cursor-pointer items-center gap-3 border-t border-line px-3 py-3 transition-colors outline-none first:border-t-0 hover:bg-hover focus-visible:bg-hover sm:px-4 ${dragging ? 'relative z-10 rounded-xl bg-card shadow-pop ring-1 ring-accent' : ''}`}
    >
      <span className="hidden w-5 shrink-0 opacity-0 transition-opacity group-hover:opacity-100 sm:block" onClick={e => e.stopPropagation()}>
        {grip}
      </span>
      {part ? (
        <>
          <Branch last={part.last} />
          {part.tile}
        </>
      ) : parts ? (
        <Trunk>
          <ClassNumber number={row.number} />
        </Trunk>
      ) : (
        <ClassNumber number={row.number} />
      )}
      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-center gap-1.5">
          {part ? (
            <span className="truncate text-[14px] font-semibold text-ink">{part.title}</span>
          ) : (
            <>
              {row.phase && <Pill tone="info">{phaseName(t, row.phase)}</Pill>}
              {row.phases.length > 0 && <Pill tone="brand">{t('overall')}</Pill>}
              <span className="truncate text-[14px] font-semibold text-ink">{row.title}</span>
            </>
          )}
        </div>
        <div className="mt-0.5 truncate text-xs text-ink-3">
          <span className="md:hidden">{[row.time, `${row.discipline} · ${row.category}`].filter(Boolean).join(' · ')}</span>
          {/* The scoring, unless the title says it already. */}
          {row.scoring && !row.title.includes(row.scoring) && (
            <span>
              <span className="md:hidden"> · </span>
              {row.scoring}
            </span>
          )}
        </div>
      </div>
      <div className="hidden w-20 shrink-0 text-[13px] font-medium text-ink-2 tabular md:block">{row.time || <span className="text-ink-4">—</span>}</div>
      <div className="hidden w-36 shrink-0 truncate text-[13px] text-ink-2 lg:block">
        <span className="font-semibold">{row.discipline}</span> · {row.category}
      </div>
      <div className="hidden w-24 shrink-0 items-center gap-1.5 text-[13px] text-ink-2 sm:flex tabular">
        <Users className="size-3.5 text-ink-4" />
        {row.starters}
        {row.not_starting > 0 && <span className="text-xs text-ink-4">−{row.not_starting}</span>}
      </div>
      <div className="hidden w-28 shrink-0 md:block">
        <Progress row={row} />
      </div>
      <div className="flex w-auto shrink-0 items-center justify-end gap-1.5 sm:w-44">
        {row.divide_into !== null && (
          <Tooltip label={sizeWarning(row, t)}>
            <span>
              <Pill tone={row.must_divide ? 'warn' : 'neutral'} icon={TriangleAlert}>
                &gt;{row.over}
              </Pill>
            </span>
          </Tooltip>
        )}
        {row.unplaced > 0 && (
          <Tooltip label={`${row.unplaced} ${t('unplaced')}`}>
            <span>
              <Pill tone="warn">+{row.unplaced}</Pill>
            </span>
          </Tooltip>
        )}
        <span className="hidden sm:inline-flex">
          <SentMarks row={row} language={language} />
        </span>
        <ClassStatus row={row} />
      </div>
      <ChevronRight className="size-4 shrink-0 text-ink-4 transition-transform group-hover:translate-x-0.5 group-hover:text-ink-2" />
    </div>
  );
}

/** Start numbers fixed for the whole show (one per entry), and giving them out. Numbers given
 *  already (printed, handed out, maybe sent to Swiss Equestrian) are not replaced or erased
 *  without asking. */
function ShowNumbers({ event, numbered, sent, open, onClose }: { event: EventView; numbered: boolean; sent: boolean; open: boolean; onClose: () => void }) {
  const t = useT();
  const { change } = useChanges();
  const [order, setOrder] = useState('name');
  const [ask, setAsk] = useState<null | 'renumber' | { fixed: boolean }>(null);
  const sentNote = sent && <p className="mt-2 font-semibold text-danger">{t('renumber_show_sent')}</p>;
  return (
    <Modal open={open} onClose={onClose} title={t('show_numbers')} icon={Hash} description={t('show_numbers_hint')} footer={<Button onClick={onClose}>{t('close')}</Button>}>
      <div className="grid gap-5">
        <Switch checked={event.fixed_numbers} onChange={v => (numbered ? setAsk({ fixed: v }) : change('set_fixed_numbers', { fixed: v }))} label={t('fixed_numbers')} />
        {event.fixed_numbers && (
          <div className="grid gap-3 rounded-xl border border-line bg-raised p-4">
            <Field label={t('number_order')}>
              <select value={order} onChange={e => setOrder(e.target.value)} className="field">
                <option value="name">{t('number_by_name')}</option>
                <option value="horse">{t('number_by_horse')}</option>
                <option value="start_order">{t('number_by_order')}</option>
                <option value="winning_points">{t('order_points')}</option>
                <option value="random">{t('number_random')}</option>
              </select>
            </Field>
            <div className="flex flex-wrap items-center gap-3">
              <Button variant="primary" icon={Hash} onClick={() => (numbered ? setAsk('renumber') : change('number_show', { order }))}>
                {t('number_show')}
              </Button>
              {event.numbers_assigned && (
                <Pill tone="ok" icon={CircleCheck}>
                  {t('numbers_given')}
                </Pill>
              )}
            </div>
          </div>
        )}
      </div>
      <ConfirmModal
        open={ask === 'renumber'}
        onClose={() => setAsk(null)}
        title={t('renumber_title')}
        message={
          <>
            <p>{t('renumber_show')}</p>
            {sentNote}
          </>
        }
        confirm={t('renumber_confirm')}
        danger
        icon={Hash}
        onConfirm={async () => {
          setAsk(null);
          await change('number_show', { order });
        }}
      />
      <ConfirmModal
        open={!!ask && ask !== 'renumber'}
        onClose={() => setAsk(null)}
        title={t('clear_numbers_title')}
        message={
          <>
            <p>{t('clear_numbers_text')}</p>
            {sentNote}
          </>
        }
        confirm={t('clear_numbers_confirm')}
        danger
        icon={Hash}
        onConfirm={async () => {
          const fixed = ask !== null && ask !== 'renumber' && ask.fixed;
          setAsk(null);
          await change('set_fixed_numbers', { fixed });
        }}
      />
    </Modal>
  );
}

/** A published class set up as another one was (Rosson's "Folgeprüfung"): its sections and order. */
function Following({ show, published, models, onClose, go }: { show: ShowView; published: number | null; models: ClassRow[]; onClose: () => void; go: Go }) {
  const t = useT();
  const { change } = useChanges();
  const [model, setModel] = useState<number | null>(null);
  const p = show.not_set_up.find(x => x.id === published);
  const from = model ?? models[0]?.published;
  return (
    <Modal
      open={published !== null}
      onClose={onClose}
      title={`${t('following')} · ${p?.number ?? ''}`}
      icon={Copy}
      description={t('following_hint')}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button
            variant="primary"
            icon={ArrowRight}
            onClick={async () => {
              if (published === null || from === undefined) return;
              const answer = await change('set_up_following', { published, model: from });
              onClose();
              if (answer && typeof answer.class === 'string') go({ show: show.id, view: 'classes', class: answer.class });
            }}
          >
            {t('set_up')}
          </Button>
        </>
      }
    >
      <Field label={t('following_as')}>
        <select value={from} onChange={e => setModel(Number(e.target.value))} className="field">
          {models.map(c => (
            <option key={c.published} value={c.published}>
              {c.number.replace(/[a-z]$/, '')} · {c.title}
            </option>
          ))}
        </select>
      </Field>
    </Modal>
  );
}

/** A day's classes as they are shown: a class alone, or a class with its parts under it — its
 *  séries (1a, 1b…: the same published class, side by side) or its phases (under the class made
 *  of them, when they follow it). */
type Block = { kind: 'one'; row: ClassRow } | { kind: 'phases'; head: ClassRow; parts: ClassRow[] } | { kind: 'series'; parts: ClassRow[] };

function blocks(classes: ClassRow[]): Block[] {
  const out: Block[] = [];
  for (const c of classes) {
    const last = out[out.length - 1];
    const head = last?.kind === 'phases' ? last.head : last?.kind === 'one' ? last.row : null;
    if (c.phase_of && head && head.id === c.phase_of) {
      if (last.kind === 'one') out[out.length - 1] = { kind: 'phases', head: last.row, parts: [c] };
      else if (last.kind === 'phases') last.parts.push(c);
      continue;
    }
    const isSeries = (r: ClassRow) => /[a-z]$/.test(r.number) && !r.phase_of && r.phases.length === 0;
    if (isSeries(c)) {
      const prev = last?.kind === 'series' ? last.parts[last.parts.length - 1] : last?.kind === 'one' ? last.row : null;
      if (prev && isSeries(prev) && prev.published === c.published) {
        if (last.kind === 'one') out[out.length - 1] = { kind: 'series', parts: [last.row, c] };
        else if (last.kind === 'series') last.parts.push(c);
        continue;
      }
    }
    out.push({ kind: 'one', row: c });
  }
  return out;
}

/** The head of a class's séries: its number and name, how many séries, their starters. */
function SeriesHead({ parts }: { parts: ClassRow[] }) {
  const plural = usePlural();
  const first = parts[0];
  const inscrits = parts.reduce((n, p) => n + p.starters, 0);
  return (
    <div className="flex items-center gap-3 px-1 py-3">
      <span className="hidden w-5 shrink-0 sm:block" />
      <Trunk>
        <ClassNumber number={first.number.replace(/[a-z]$/, '')} />
      </Trunk>
      <div className="min-w-0 flex-1">
        <div className="truncate text-[14px] font-semibold text-ink">{first.title}</div>
        <div className="mt-0.5 truncate text-xs text-ink-3">
          {parts.length} {plural(parts.length, 'sections')} · {inscrits} {plural(inscrits, 'entered_count')}
        </div>
      </div>
    </div>
  );
}

export function Classes({ show, go }: { show: ShowView; go: Go }) {
  const t = useT();
  const plural = usePlural();
  const language = useLanguage();
  const toast = useToast();
  const { change } = useChanges();
  const [tab, setTab] = useState<Tab>(() => (sessionStorage.getItem('classes-tab') as Tab) || 'programme');
  const [numbers, setNumbers] = useState(false);
  const [following, setFollowing] = useState<number | null>(null);
  const [newCombined, setNewCombined] = useState(false);
  const choose = (v: Tab) => {
    sessionStorage.setItem('classes-tab', v);
    setTab(v);
  };
  const all = show.days.flatMap(d => d.classes);
  const models = [...new Map(all.map(c => [c.published, c] as const)).values()];
  const setUp = async (published: number) => {
    if (await change('set_up_class', { published })) go({ show: show.id, view: 'classes', class: String(published) });
  };
  const printDay = async (classes: string[]) => {
    toast.show({ text: t('printing'), tone: 'busy' });
    try {
      await openPrint({ show: show.id, language, what: 'start_list', classes });
      toast.clear();
    } catch (e) {
      toast.show({ text: String((e as Error).message ?? e), tone: 'error' });
    }
  };

  return (
    <div className="pane">
      <div className="mx-auto max-w-[1180px] space-y-5 px-3 py-5 sm:px-6 sm:py-7">
        <PageHeader
          icon={ClipboardList}
          title={t('classes')}
          subtitle={`${show.entries} ${plural(show.entries, 'entries')} · ${show.riders} ${plural(show.riders, 'riders')} · ${show.horses} ${plural(show.horses, 'horses')}`}
          actions={
            show.event && (
              <Button icon={Hash} onClick={() => setNumbers(true)}>
                {t('show_numbers')}
              </Button>
            )
          }
        />
        <div className="border-b border-line">
          <Tabs
            value={tab}
            onChange={choose}
            compact
            items={[
              { value: 'programme', label: t('programme_tab'), icon: CalendarDays, count: all.length },
              { value: 'prepare', label: t('to_set_up'), icon: Layers, count: show.not_set_up.length || null, dot: show.not_set_up.length > 0 },
              { value: 'combined', label: t('combined'), icon: Sigma, count: show.combined.length || null },
            ]}
          />
        </div>

        {tab === 'programme' && (
          <div className="space-y-5">
            {show.days.length === 0 && (
              <Card>
                <EmptyState
                  icon={CalendarDays}
                  title={t('no_classes_yet')}
                  action={
                    show.not_set_up.length > 0 && (
                      <Button variant="primary" icon={ArrowRight} onClick={() => choose('prepare')}>
                        {t('to_set_up')}
                      </Button>
                    )
                  }
                >
                  {t('no_classes_yet_hint')}
                </EmptyState>
              </Card>
            )}
            {/* The days' order (the timetable's): a class dragged by its grip to its place, in its
                day or another (it then takes that day); a line shows where it will land. */}
            <SortableGroups
              groups={show.days.map(d => ({ id: d.date, ids: d.classes.map(c => c.id) }))}
              onMove={(id, date, to) => change('move_class', { class: id, date, to })}
              // Drawn above the page: a série dragged out of its class's box stays in sight.
              overlay={id => {
                const c = all.find(x => x.id === id);
                return c && <ClassLine row={c} show={show.id} go={() => {}} language={language} dragging />;
              }}
            >
              {show.days.map(day => {
                const ready = day.classes.filter(c => c.starters > 0).map(c => c.id);
                return (
                  <Card key={day.date} className="overflow-hidden">
                    <CardHeader
                      icon={CalendarDays}
                      title={<span className="capitalize">{formatDate(day.date, language)}</span>}
                      subtitle={`${day.classes.length} ${plural(day.classes.length, 'classes_selected')} · ${day.classes.reduce((n, c) => n + c.starters, 0)} ${plural(
                        day.classes.reduce((n, c) => n + c.starters, 0),
                        'entered_count',
                      )}`}
                      actions={
                        ready.length > 0 && (
                          <Button size="sm" variant="ghost" icon={Printer} onClick={() => printDay(ready)}>
                            <span className="hidden sm:inline">{t('print_day_start_lists')}</span>
                          </Button>
                        )
                      }
                    />
                    {/* A class with séries or phases: one block, its parts under it (each still its own
                      class, opened and moved on its own). */}
                    <SortableGroup ids={day.classes.map(c => c.id)}>
                      <div>
                        {blocks(day.classes).map(b => {
                          const line = (c: ClassRow, part?: Part, parts?: boolean) => (
                            <SortRow key={c.id} id={c.id} disabled={all.length < 2}>
                              {row => (
                                // While dragged, the row stays faded at its place (its copy above the page moves).
                                <div ref={row.ref} style={{ ...row.style, opacity: row.dragging ? 0.3 : undefined }} className={part ? '[&>div]:border-t-0' : ''}>
                                  <DropMark line={row.line} />
                                  <ClassLine
                                    row={c}
                                    show={show.id}
                                    go={go}
                                    language={language}
                                    part={part}
                                    parts={parts}
                                    grip={<Grip grip={row.grip} label={t('drag_to_move')} disabled={all.length < 2} />}
                                  />
                                </div>
                              )}
                            </SortRow>
                          );
                          if (b.kind === 'one') return line(b.row);
                          if (b.kind === 'phases')
                            return (
                              <div key={b.head.id} className="mx-2 my-1.5 overflow-hidden rounded-xl bg-raised/70 ring-1 ring-line sm:mx-3 [&_[role=link]]:px-1">
                                <div className="[&>div>div]:border-t-0">{line(b.head, undefined, true)}</div>
                                {b.parts.map((p, i) =>
                                  line(p, {
                                    tile: (
                                      <span className="grid h-8 min-w-8 shrink-0 place-items-center rounded-lg bg-sunken px-1 text-[11px] font-bold text-ink-2 ring-1 ring-line">{p.discipline}</span>
                                    ),
                                    title: p.phase ? phaseName(t, p.phase) : p.name,
                                    last: i === b.parts.length - 1,
                                  }),
                                )}
                              </div>
                            );
                          return (
                            <div key={b.parts[0].id} className="mx-2 my-1.5 overflow-hidden rounded-xl bg-raised/70 ring-1 ring-line sm:mx-3 [&_[role=link]]:px-1">
                              <SeriesHead parts={b.parts} />
                              {b.parts.map((p, i) => {
                                const letter = p.number.slice(-1);
                                return line(p, {
                                  tile: (
                                    <span className={`grid size-8 shrink-0 place-items-center rounded-lg text-[13px] font-bold text-white ${SERIES[(letter.charCodeAt(0) - 97) % SERIES.length]}`}>
                                      {letter}
                                    </span>
                                  ),
                                  title: `${t('series_label')} ${letter}`,
                                  last: i === b.parts.length - 1,
                                });
                              })}
                            </div>
                          );
                        })}
                      </div>
                    </SortableGroup>
                  </Card>
                );
              })}
            </SortableGroups>
          </div>
        )}

        {tab === 'prepare' && (
          <Card className="overflow-hidden">
            {show.not_set_up.length === 0 ? (
              <EmptyState icon={CircleCheck} title={t('all_set_up')}>
                {t('all_set_up_hint')}
              </EmptyState>
            ) : (
              <>
                <CardHeader icon={Layers} title={t('not_set_up')} subtitle={t('set_up_hint')} />
                {show.not_set_up.map(p => (
                  <div key={p.id} className="flex flex-wrap items-center gap-3 border-t border-line px-3 py-3 first:border-t-0 sm:px-4">
                    <ClassNumber number={p.number} />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[14px] font-semibold">
                        {p.discipline} · {p.category}
                      </div>
                      <div className="truncate text-xs text-ink-3">
                        <span className="capitalize">{formatDate(p.date, language)}</span> · {p.entries} {plural(p.entries, 'entries')}
                        {p.scoring && ` · ${p.scoring}`}
                      </div>
                    </div>
                    <div className="flex gap-2">
                      {models.length > 0 && (
                        <Button size="sm" icon={Copy} onClick={() => setFollowing(p.id)} title={t('following_hint')}>
                          {t('following')}
                        </Button>
                      )}
                      <Button size="sm" variant="primary" icon={ArrowRight} onClick={() => setUp(p.id)} title={t('set_up_hint')}>
                        {t('set_up')}
                      </Button>
                    </div>
                  </div>
                ))}
              </>
            )}
          </Card>
        )}

        {tab === 'combined' && (
          <Card className="overflow-hidden">
            <CardHeader
              icon={Trophy}
              title={t('combined')}
              subtitle={t('combined_hint')}
              actions={
                show.combined.length > 0 && (
                  <Button variant="primary" icon={Plus} onClick={() => setNewCombined(true)}>
                    {t('new_combined')}
                  </Button>
                )
              }
            />
            <CombinedList show={show} go={go} onNew={() => setNewCombined(true)} />
          </Card>
        )}
      </div>
      {show.event && <ShowNumbers event={show.event} numbered={all.some(c => c.numbered)} sent={all.some(c => !!c.start_lists_sent)} open={numbers} onClose={() => setNumbers(false)} />}
      <Following show={show} published={following} models={models} onClose={() => setFollowing(null)} go={go} />
      <NewCombined show={show} open={newCombined} onClose={() => setNewCombined(false)} go={go} />
    </div>
  );
}
