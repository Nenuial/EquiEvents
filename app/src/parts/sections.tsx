// Dividing a class into sections (U13): every rider with their horses, where they live, their
// club, the entries' comments (often the coach), pony sizes and levels; sections given by a rule
// or by hand, a search to put several at once; the result shown section by section before it is
// applied (one change, undone with ⌘Z). Dividing again, or back to one class, the same way.
// A rider's horses go together, except by pony size or level: each horse where it is ranked.

import { useEffect, useMemo, useState } from 'react';
import { ChevronDown, Search, Split, WandSparkles } from 'lucide-react';
import { call, type ClassRow } from '../api';
import { usePlural, useT } from '../i18n';
import { Button, Callout, Menu, MenuItem, Modal, Segmented } from '../ui';
import { useChanges } from './changes';
import { ErrorNote, useLoad } from './common';

interface Horse {
  entry: number;
  horse: string;
  pony: string | null;
  level: number | null;
  comment: string;
  remarks: string;
  not_starting: boolean;
  start_number: number | null;
  section: number;
}
interface Rider {
  person: number;
  name: string;
  town: string;
  club: string;
  horses: Horse[];
}
interface Plan {
  published: number;
  number: string;
  sections: string[];
  riders: Rider[];
  /** Not to be divided (again), and why. */
  locked: { why: 'teams' | 'phases' | 'final' | 'results'; class: string | null } | null;
}

const letter = (i: number) => String.fromCharCode(97 + i);
/** Each series its colour, the same in both themes (white letters on it). */
/** Too many entries (S19): jumping over 90 must be divided; dressage over 40 must be, over 35 two series are the rule (DR 5.2). */
export function sizeWarning(row: ClassRow, t: ReturnType<typeof useT>): string {
  if (row.discipline !== 'CD') return t('too_many_entries');
  return row.must_divide ? t('dressage_must_divide') : t('dressage_two_series');
}

export const SERIES = ['bg-sky-600', 'bg-amber-600', 'bg-emerald-600', 'bg-violet-600', 'bg-rose-600', 'bg-slate-500'];
const RULES: { rule: string; key: Parameters<ReturnType<typeof useT>>[0]; groups?: string[] }[] = [
  { rule: 'balanced', key: 'rule_balanced' },
  { rule: 'club', key: 'rule_club' },
  { rule: 'town', key: 'rule_town' },
  { rule: 'comment', key: 'rule_comment' },
  { rule: 'level', key: 'rule_level' },
  { rule: 'pony', key: 'rule_pony_ab_c_d', groups: ['AB', 'C', 'D'] },
  { rule: 'pony', key: 'rule_pony_abc_d', groups: ['ABC', 'D'] },
];

/** The workshop, as a dialog: the series, a proposal, a search; applying (or not) closes it. */
export function SectionsWorkshop({
  show,
  published,
  startParts,
  startRule,
  title,
  done,
}: {
  show: string;
  published: number;
  startParts?: number;
  startRule?: string;
  title: string;
  done: (classId?: string) => void;
}) {
  const t = useT();
  const plural = usePlural();
  const { change } = useChanges();
  const loaded = useLoad(() => call<Plan>('sections', { show, published }), [show, published]);
  const plan = loaded.data;
  const [parts, setParts] = useState(startParts ?? 2);
  // For each rider, the section of each of their horses.
  const [sections, setSections] = useState<number[][] | null>(null);
  const [query, setQuery] = useState('');
  const [error, setError] = useState<string | null>(null);

  // From the class as it is; or, asked from the 90-entries warning, a balanced suggestion.
  useEffect(() => {
    if (!plan || sections) return;
    if (plan.sections.length > 1 && !startRule) {
      setParts(plan.sections.length);
      setSections(plan.riders.map(r => r.horses.map(h => h.section)));
    } else {
      suggest(startRule ?? 'balanced', startParts ?? Math.max(2, plan.sections.length));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plan]);

  async function suggest(rule: string, n: number, groups?: string[]) {
    try {
      const s = await call<number[][]>('suggest_sections', { show, published, rule, parts: n, groups });
      setParts(groups ? groups.length : n);
      setSections(s);
      setError(null);
    } catch (e) {
      setError(String((e as Error).message ?? e));
    }
  }

  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const text = (r: Rider) => [r.name, r.town, r.club, ...r.horses.flatMap(h => [h.horse, h.comment, h.remarks, h.pony ?? ''])].join(' ').toLowerCase();
  const found = useMemo(() => (plan && words.length ? plan.riders.map((r, i) => (words.every(w => text(r).includes(w)) ? i : -1)).filter(i => i >= 0) : []), [plan, query]);

  if (!plan || !sections)
    return (
      <Modal open onClose={() => done()} title={title} icon={Split} size="xl">
        {loaded.error ? <ErrorNote error={loaded.error} /> : <div className="h-40 animate-pulse rounded-xl bg-sunken" />}
      </Modal>
    );

  // A rider (all their horses), or one horse, into a section.
  const put = (indices: number[], s: number) => setSections(cur => (cur ? cur.map((x, i) => (indices.includes(i) ? x.map(() => s) : x)) : cur));
  const putHorse = (i: number, j: number, s: number) => setSections(cur => (cur ? cur.map((x, k) => (k === i ? x.map((y, l) => (l === j ? s : y)) : x)) : cur));
  const changeParts = (n: number) => {
    setParts(n);
    setSections(cur => (cur ? cur.map(x => x.map(s => Math.min(s, n - 1))) : cur));
  };
  const starters = (s: number) => sections.flat().filter(x => x === s).length;
  const ridersIn = (s: number) => sections.filter(x => x.includes(s)).length;
  // A rider whose horses are in different sections (pony sizes, levels).
  const apart = (i: number) => new Set(sections[i]).size > 1;
  const empty = Array.from({ length: parts }, (_, s) => s).some(s => ridersIn(s) === 0);
  const visible = (i: number) => words.length === 0 || found.includes(i);

  const apply = async () => {
    const answer = await change('apply_sections', { published, parts, sections });
    if (answer) done(typeof answer.class === 'string' ? answer.class : undefined);
  };

  return (
    <Modal
      open
      onClose={() => done()}
      title={title}
      icon={Split}
      size="xl"
      focusFirst={false}
      footer={
        <>
          {empty && <span className="mr-auto text-xs font-medium text-warn">{t('sections_empty')}</span>}
          <Button variant="ghost" onClick={() => done()}>
            {t('cancel')}
          </Button>
          <Button variant="primary" icon={Split} disabled={!!plan.locked || empty} onClick={apply}>
            {parts === 1 ? t('sections_apply_whole') : t('sections_apply').replace('{n}', String(parts))}
          </Button>
        </>
      }
    >
      <div className="space-y-4 text-[13px]">
        {/* How many series, a proposal, a search to put several at once (a coach, a club…). */}
        <div className="space-y-3 rounded-xl border border-line bg-raised p-3">
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2.5">
            <div className="flex items-center gap-2.5">
              <span className="text-xs font-semibold text-ink-2">
                {t('sections_divide')} {plan.number}
              </span>
              <Segmented size="sm" value={String(parts)} onChange={v => changeParts(Number(v))} options={[1, 2, 3, 4, 5, 6].map(n => ({ value: String(n), label: String(n) }))} />
            </div>
            {parts > 1 && (
              <Menu
                align="start"
                width="w-72"
                trigger={
                  <Button size="sm" icon={WandSparkles} iconRight={ChevronDown}>
                    {t('sections_suggest')}
                  </Button>
                }
              >
                {RULES.map(r => (
                  <MenuItem key={r.key} onSelect={() => suggest(r.rule, parts, r.groups)}>
                    {t(r.key)}
                  </MenuItem>
                ))}
              </Menu>
            )}
            <div className="relative min-w-0 flex-1 basis-56">
              <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-ink-4" />
              <input value={query} onChange={e => setQuery(e.target.value)} placeholder={t('sections_search')} className="field field-sm pl-8" />
            </div>
          </div>
          <p className="text-xs text-ink-3">{parts === 1 ? t('sections_whole') : t('sections_parts')}</p>
          {found.length > 0 && parts > 1 && (
            <div className="flex flex-wrap items-center gap-1.5 text-xs text-ink-2">
              {found.length} {t('sections_found')} →
              {Array.from({ length: parts }, (_, s) => (
                <button key={s} onClick={() => put(found, s)} className={`grid h-7 min-w-7 place-items-center rounded-md px-2 text-xs font-bold text-white uppercase ${SERIES[s]}`}>
                  {letter(s)}
                </button>
              ))}
            </div>
          )}
        </div>
        {plan.locked && <Callout tone="warn">{t(`sections_locked_${plan.locked.why}`).replace('{n}', plan.locked.class ?? '')}</Callout>}
        <ErrorNote error={error} />

        {/* The result, series by series. */}
        <div className="space-y-3">
          {Array.from({ length: parts }, (_, s) => (
            <section key={s} className="overflow-hidden rounded-xl border border-line bg-card shadow-card">
              <h3 className="flex items-center gap-2.5 border-b border-line bg-raised px-3 py-2">
                <span className={`grid h-6 min-w-6 place-items-center rounded-md px-1.5 text-xs font-bold text-white ${parts > 1 ? SERIES[s] : 'bg-brand'}`}>
                  {parts > 1 ? `${plan.number}${letter(s)}` : plan.number}
                </span>
                <span className="ml-auto text-xs text-ink-3 tabular">
                  {starters(s)} {plural(starters(s), 'starters')} · {ridersIn(s)} {plural(ridersIn(s), 'riders')}
                </span>
              </h3>
              <ul className="divide-y divide-line">
                {plan.riders.map((r, i) =>
                  sections[i].includes(s) && visible(i) ? (
                    <li key={r.person} className={`group flex flex-wrap items-start gap-x-3 gap-y-1.5 px-3 py-2 ${found.includes(i) ? 'bg-accent/8' : ''}`}>
                      <div className="min-w-0 flex-1 basis-56">
                        <div className="truncate font-semibold text-ink">{r.name}</div>
                        <div className="truncate text-xs text-ink-3">{[r.town, r.club].filter(Boolean).join(' · ')}</div>
                      </div>
                      <div className="min-w-0 flex-1 basis-56 space-y-0.5 text-xs">
                        {r.horses.map((h, j) =>
                          sections[i][j] !== s ? null : (
                            <div key={h.entry} className={`flex items-center gap-1.5 ${h.not_starting ? 'text-ink-3 line-through' : 'text-ink'}`}>
                              <span className="min-w-0 truncate">
                                {h.horse}
                                {h.pony && <span className="ml-1 rounded bg-sunken px-1 font-semibold text-ink-2">{h.pony}</span>}
                                {h.level !== null && (
                                  <span className="ml-1 text-ink-3">
                                    · {t('level')} {h.level}
                                  </span>
                                )}
                                {(h.comment || h.remarks) && <span className="ml-1 text-ink-3 italic">· {h.comment || h.remarks}</span>}
                              </span>
                              {/* A horse on its own: with several horses, each can go elsewhere. */}
                              {parts > 1 && r.horses.length > 1 && (
                                <span
                                  className={`ml-auto flex shrink-0 gap-0.5 transition-opacity focus-within:opacity-100 pointer-coarse:opacity-100 ${apart(i) ? '' : 'opacity-0 group-hover:opacity-100'}`}
                                >
                                  {Array.from({ length: parts }, (_, x) => (
                                    <button
                                      key={x}
                                      onClick={() => putHorse(i, j, x)}
                                      className={`grid size-5 place-items-center rounded text-[10px] font-bold uppercase pointer-coarse:size-8 ${x === s ? `${SERIES[x]} text-white` : 'text-ink-4 hover:bg-hover hover:text-ink'}`}
                                    >
                                      {letter(x)}
                                    </button>
                                  ))}
                                </span>
                              )}
                            </div>
                          ),
                        )}
                        {apart(i) && <div className="text-[11px] text-ink-3">{t('sections_apart')}</div>}
                      </div>
                      {parts > 1 && (
                        <div role="radiogroup" className="inline-flex shrink-0 rounded-lg bg-sunken p-0.5">
                          {Array.from({ length: parts }, (_, x) => {
                            const on = !apart(i) && x === s;
                            return (
                              <button
                                key={x}
                                role="radio"
                                aria-checked={on}
                                onClick={() => put([i], x)}
                                className={`grid size-7 place-items-center rounded-md text-xs font-bold uppercase transition-colors pointer-coarse:size-9 ${on ? `${SERIES[x]} text-white shadow-xs` : 'text-ink-3 hover:bg-card hover:text-ink'}`}
                              >
                                {letter(x)}
                              </button>
                            );
                          })}
                        </div>
                      )}
                    </li>
                  ) : null,
                )}
              </ul>
            </section>
          ))}
        </div>
      </div>
    </Modal>
  );
}
