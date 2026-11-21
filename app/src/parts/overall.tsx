// A class made of phases (eventing, driving). Its phases, each a class of its own to open, in
// their order (by drag and drop, as Rosson's "Wertung Teilprüfungen": driving's tie order;
// eventing's ties follow the rules); the overall ranking, with each phase's result and what it
// counts; eventing: the jury's penalties in dressage and jumping.

import { useState } from 'react';
import { call, type ClassRow, type PhaseKind } from '../api';
import { formatDate, useLanguage, useReason, useT, type Key } from '../i18n';
import { useChanges } from './changes';
import { ErrorNote, useLoad } from './common';
import { Button, Modal } from '../ui';
import { Grip, SortableList, SortRow } from './sortable';

type Kind = PhaseKind;

interface Cell {
  value: string | null;
  rank: number | null;
  out: boolean;
  /** Why out (Swiss Equestrian's name). */
  reason: string | null;
  text: string;
  /** Eventing: the jury's penalties in this phase, included in the value. */
  jury: string | null;
}
interface Line {
  entry: number;
  rank: number | null;
  placed: boolean;
  start_number: number | null;
  rider: string;
  horse: string;
  club: string;
  cells: Cell[];
  total: string | null;
  out: boolean;
  reason: string | null;
}
interface OverallView {
  class: ClassRow;
  phases: { id: string; kind: Kind; date: string; results: number }[];
  lines: Line[];
  rank_points: boolean;
  placed_count: number;
}

export const phaseName = (t: (k: Key) => string, kind: Kind) => t(`phase_${kind}` as Key);

export function OverallPane({ show, classId, open }: { show: string; classId: string; open: (phase: string) => void }) {
  const t = useT();
  const reason = useReason();
  const language = useLanguage();
  const { version, change } = useChanges();
  const view = useLoad(() => call<OverallView>('overall', { show, class: classId }), [show, classId, version]);
  // Eventing: the jury's penalties of a rider in a dressage or jumping phase (25 points).
  const [jury, setJury] = useState<{ phase: string; entry: number; rider: string; kind: Kind; points: string; set: boolean } | null>(null);
  if (view.error) return <ErrorNote error={view.error} />;
  const v = view.data;
  if (!v) return null;
  const locked = v.class.status === 'final';
  const eventing = v.class.discipline === 'CC';
  const driving = v.class.discipline === 'CA';
  const th = 'border-b border-line bg-raised px-3 py-2.5 text-left text-xs font-semibold text-ink-3';
  const td = 'border-b border-line px-3 py-2 align-top';

  return (
    <div className="space-y-4 p-4">
      <section className="max-w-xl">
        <h3 className="mb-1 text-xs font-bold uppercase tracking-wide text-ink-3">{t('phases')}</h3>
        <p className="mb-2 text-xs text-ink-3">{t(eventing ? 'phases_order_hint_eventing' : 'phases_order_hint')}</p>
        <SortableList ids={locked ? [] : v.phases.map(p => p.id)} onMove={(phase, to) => change('move_phase', { phase, to })}>
          <ol className="space-y-1">
            {v.phases.map((p, i) => (
              <SortRow key={p.id} id={p.id} disabled={locked}>
                {row => (
                  <li ref={row.ref} style={row.style} className={`flex items-center gap-2 rounded-lg border border-line bg-card px-2 py-1.5 ${row.dragging ? 'shadow-lg ring-1 ring-accent' : ''}`}>
                    <Grip grip={row.grip} label={t('drag_to_move')} disabled={locked} />
                    <span className="w-5 text-right text-xs text-ink-3 tabular">{i + 1}.</span>
                    <button onClick={() => open(p.id)} className="min-w-0 flex-1 text-left hover:text-accent">
                      <span className="font-semibold">{phaseName(t, p.kind)}</span>
                      <span className="ml-2 text-xs text-ink-3">
                        {formatDate(p.date, language)} · {p.results} {t('results')}
                      </span>
                    </button>
                    <span className="text-ink-3">›</span>
                  </li>
                )}
              </SortRow>
            ))}
          </ol>
        </SortableList>
        {/* Driving: a presentation judged standing, as a phase of its own (L level). */}
        {driving && !locked && !v.phases.some(p => p.kind === 'presentation') && (
          <div className="mt-2 flex flex-wrap items-center gap-2">
            <Button size="sm" variant="secondary" onClick={() => change('add_presentation', { class: classId })}>
              {t('add_presentation')}
            </Button>
            <span className="text-xs text-ink-3">{t('add_presentation_hint')}</span>
          </div>
        )}
      </section>

      <section>
        <h3 className="mb-1 text-xs font-bold uppercase tracking-wide text-ink-3">
          {t('overall')} · {v.placed_count} {t('placed')}
          {v.rank_points && <span className="ml-2 normal-case">({t('rank_points')})</span>}
        </h3>
        <div className="overflow-x-auto">
          <table className="w-full min-w-[36rem] text-sm">
            <thead>
              <tr>
                <th className={`${th} w-12 text-right`}>{t('rank')}</th>
                <th className={th}>{t('rider')}</th>
                {v.phases.map(p => (
                  <th key={p.id} className={`${th} text-right`}>
                    {phaseName(t, p.kind)}
                  </th>
                ))}
                <th className={`${th} w-20 text-right`}>{t('total')}</th>
              </tr>
            </thead>
            <tbody className="tabular">
              {v.lines.map(l => (
                <tr key={l.entry} className="hover:bg-hover">
                  <td className={`${td} text-right ${l.placed ? 'font-bold text-accent-dark' : ''}`}>{l.rank ?? '–'}</td>
                  <td className={`${td} max-w-0`}>
                    <div className="truncate font-medium">
                      {l.start_number !== null && <span className="mr-1 text-xs text-ink-3">{l.start_number}</span>}
                      {l.rider}
                    </div>
                    <div className="truncate text-xs text-ink-3">{l.horse}</div>
                  </td>
                  {l.cells.map((c, i) => {
                    const phase = v.phases[i];
                    // Eventing's dressage and jumping: the jury's penalties, set here.
                    const juryHere = eventing && !locked && !c.out && (phase.kind === 'dressage' || phase.kind === 'jumping');
                    const shown = (
                      <>
                        <div>{c.value ?? ''}</div>
                        <div className="text-[11px] text-ink-3">
                          {c.text}
                          {c.rank !== null && c.text && ` · ${c.rank}.`}
                        </div>
                        {c.jury !== null && (
                          <div className="text-[11px] font-semibold text-warn">
                            +{c.jury} {t('jury_penalty').toLowerCase()}
                          </div>
                        )}
                      </>
                    );
                    return (
                      <td key={i} className={`${td} text-right`}>
                        {c.out ? (
                          <span className="text-warn">{c.reason ? reason(c.reason) : t('out')}</span>
                        ) : juryHere ? (
                          <button
                            title={t('jury_penalty')}
                            onClick={() => setJury({ phase: phase.id, entry: l.entry, rider: l.rider, kind: phase.kind, points: c.jury ?? '25', set: c.jury !== null })}
                            className="w-full rounded text-right hover:bg-hover pointer-coarse:min-h-9"
                          >
                            {shown}
                          </button>
                        ) : (
                          shown
                        )}
                      </td>
                    );
                  })}
                  <td className={`${td} text-right font-semibold`}>{l.out ? <span className="text-warn">{l.reason ? reason(l.reason) : t('out')}</span> : (l.total ?? '')}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
      {jury && (
        <Modal
          open
          onClose={() => setJury(null)}
          title={t('jury_penalty')}
          description={`${jury.rider} · ${phaseName(t, jury.kind)}`}
          footer={
            <>
              {jury.set && (
                <Button
                  variant="ghost"
                  onClick={async () => {
                    if (await change('set_jury_penalty', { class: jury.phase, entry: jury.entry, points: null })) setJury(null);
                  }}
                >
                  {t('jury_penalty_none')}
                </Button>
              )}
              <Button
                variant="primary"
                disabled={!(Number(jury.points) > 0)}
                onClick={async () => {
                  if (await change('set_jury_penalty', { class: jury.phase, entry: jury.entry, points: jury.points.trim() })) setJury(null);
                }}
              >
                {t('save')}
              </Button>
            </>
          }
        >
          <label className="flex flex-wrap items-center gap-3">
            <input id="jury-points" className="field field-sm w-20 text-right" inputMode="decimal" value={jury.points} onChange={e => setJury({ ...jury, points: e.target.value })} />
            <span className="text-xs text-ink-3">{t('jury_penalty_hint')}</span>
          </label>
        </Modal>
      )}
    </div>
  );
}
