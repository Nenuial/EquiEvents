// Dressage: entering the judges' sheets (U4, U5). The riders in start order on the left; the
// selected rider's sheets side by side, a column per judge. A sheet entered for the wrong rider
// or judge is sent where it belongs (or swapped), or cut and pasted later (the clipboard: sheets
// put aside), never typed again; a sheet can be deleted (undo brings it back). Fast entry (optional): "6." writes 6.5. Classes where the judges give a total only: one
// field per judge. Children tests: one sheet for the judges. Freestyle: parts A and B.

import { useEffect, useMemo, useRef, useState } from 'react';
import { call } from '../api';
import { usePlural, useReason, useT, type Language } from '../i18n';
import { useChanges } from '../parts/changes';
import { ErrorNote, useLoad, useNarrow } from '../parts/common';
import { findStarter, QuickJump } from '../parts/jump';
import { ProgrammePicker } from '../parts/pickers';
import { ArrowLeftRight, ChevronDown, ChevronLeft, ChevronUp, ClipboardList, ClipboardPaste, Scissors, Send, Trash2 } from 'lucide-react';
import { Button, Callout, Field, IconButton, Menu, MenuItem, MenuLabel, MenuSeparator, Modal, Tooltip } from '../ui';

interface Movement {
  number: string;
  de: string;
  fr: string;
  it: string;
  coefficient: string;
}
interface Programme {
  id: number;
  name: string;
  title_de: string;
  title_fr: string;
  title_it: string;
  max_points: string;
  sections: { title_de: string; title_fr: string; title_it: string; movements: Movement[] }[];
  penalties: { de: string; fr: string; it: string; cost_de: string; cost_fr: string; cost_it: string; points: string | null; percent: string | null }[];
  /** children, joint (young horses): one sheet for the jury, entered as the first judge's. */
  kind: 'standard' | 'freestyle' | 'children' | 'joint';
}
interface Sheet {
  marks: Record<string, string>;
  penalties: number[];
  complete: boolean;
  total: string | null;
  percent: string | null;
  entered: string | null;
}
interface Rider {
  entry: number;
  position: number;
  start_number: number | null;
  rider: string;
  horse: string;
  not_starting: boolean;
  programme: number | null;
  sheets: Record<string, Sheet>;
  /** The penalties ticked: one set for every judge (places in the programme's list). */
  penalties: number[];
  percentage: string | null;
  part_a: string | null;
  part_b: string | null;
  rank: number | null;
  /** Why out of the class, as the office entered it. */
  out: number | null;
  /** Team dressage: the row is this team's. */
  team?: string;
  /** Team dressage 3/2: the rider each of H, C, M judges (judge → name). */
  judge_riders?: Record<string, string>;
}
interface Judge {
  assignment: string;
  letter: string;
  name: string;
}
interface View {
  class: { id: string; status: string; number: string };
  programmes: Record<string, Programme>;
  judges: Judge[];
  riders: Rider[];
  /** The reasons for being out (eliminated, retired…). */
  reasons: { code: number; name: string }[];
  /** Cut sheets (the clipboard): their first marks, to know the paper sheet. */
  aside: Cut[];
  totals_per_judge: boolean;
  teams: boolean;
  /** Driving dressage: totals on 160, lowest first. */
  driving: boolean;
}

interface Cut {
  slot: number;
  marks: number;
  first: string[];
  total: string | null;
}

/** Swiss Equestrian's text in the office's language (Italian, else German: it always has German). */
const say = (language: Language, fr: string, de: string, it: string) => (language === 'de' ? de : language === 'it' ? it || de : fr);

/** A row's key: its team (team dressage), else its entry. */
const keyOf = (r: Rider) => r.team ?? String(r.entry);
/** One sheet for the jury: Children tests, young horses' tests. */
const oneSheet = (kind: string | undefined) => kind === 'children' || kind === 'joint';
/** Whose sheet: the team's or the rider's. */
const whose = (r: Rider) => (r.team ? { team: r.team } : { entry: r.entry });

/** A mark field: shows what is saved, keeps what is being typed, saves on Enter, Tab or leaving. */
function MarkInput({
  value,
  fast,
  disabled,
  onSave,
  onMove,
  register,
  max = 10,
}: {
  value: string;
  fast: boolean;
  disabled: boolean;
  onSave: (v: string) => void;
  onMove: (step: number) => void;
  register: (el: HTMLInputElement | null) => void;
  /** A judge's total (up to the programme's maximum, two decimals) instead of a mark. */
  max?: number;
}) {
  const [text, setText] = useState(value);
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setText(value);
  }, [value]);
  const pattern = max === 10 ? /^\d{1,2}([.,](0|5))?$/ : /^\d{1,4}([.,]\d{1,2})?$/;
  const fits = (t: string) => pattern.test(t.trim()) && Number(t.trim().replace(',', '.')) <= max;
  const valid = text.trim() === '' || fits(text);
  const save = (t: string) => {
    if (t.trim() !== value && (t.trim() === '' || fits(t))) onSave(t.trim().replace(',', '.'));
  };
  return (
    <input
      ref={register}
      value={text}
      disabled={disabled}
      inputMode="decimal"
      enterKeyHint="next"
      onFocus={e => {
        focused.current = true;
        e.target.select();
      }}
      onBlur={() => {
        focused.current = false;
        save(text);
      }}
      onChange={e => {
        const typed = e.target.value;
        // Fast entry: "6." is 6.5, and on to the next mark (whatever typed the point).
        const m = fast && max === 10 ? /^\s*(\d)[.,]\s*$/.exec(typed) : null;
        if (m) {
          const v = `${m[1]}.5`;
          setText(v);
          onSave(v);
          onMove(1);
        } else {
          setText(typed);
        }
      }}
      onKeyDown={e => {
        if (e.key === 'Enter' || (e.key === 'Tab' && !e.shiftKey) || e.key === 'ArrowDown') {
          e.preventDefault();
          save(text);
          onMove(e.shiftKey ? -1 : 1);
        } else if ((e.key === 'Tab' && e.shiftKey) || e.key === 'ArrowUp') {
          e.preventDefault();
          save(text);
          onMove(-1);
        } else if (e.key === 'Escape') {
          setText(value);
        }
      }}
      className={`h-8 w-full rounded-md border bg-card px-2 text-right text-[13px] font-semibold tabular shadow-xs outline-none transition-[border-color,box-shadow] focus:border-info focus:ring-3 focus:ring-info/20 pointer-coarse:h-11 phone:scroll-mt-40 ${valid ? 'border-line-strong' : 'border-danger bg-danger/8'} ${disabled ? 'opacity-50' : ''}`}
    />
  );
}

export default function DressageEntry({ show, classId, language }: { show: string; classId: string; language: Language }) {
  const t = useT();
  const plural = usePlural();
  const reason = useReason();
  const { version, change } = useChanges();
  const view = useLoad(() => call<View>('sheets', { show, class: classId }), [show, classId, version]);
  const [selected, setSelected] = useState<string | null>(null);
  const [fast, setFast] = useState(() => localStorage.getItem('fast-marks') === '1');
  const [moving, setMoving] = useState<{ judge: string } | null>(null);
  const inputs = useRef<Map<string, HTMLInputElement>>(new Map());
  // On a phone (U10): the riders, or one rider's sheet, one judge at a time.
  const narrow = useNarrow();
  const [sheetOpen, setSheetOpen] = useState(false);
  const [judgeIndex, setJudgeIndex] = useState(0);
  // On a phone the page scrolls: a rider's sheet opened (or the next rider's) starts at the top
  // of the screen, the class's head and tools above it out of the way.
  const sheetTop = useRef<HTMLDivElement>(null);
  const v = view.data;
  const locked = v?.class.status === 'final';

  const riders = useMemo(() => (v?.riders ?? []).filter(r => !r.not_starting), [v]);
  const rider = riders.find(r => keyOf(r) === selected) ?? riders[0];
  useEffect(() => {
    if (rider && selected === null) setSelected(keyOf(rider));
  }, [rider, selected]);
  const programme = rider?.programme !== null && rider ? v?.programmes[String(rider.programme)] : undefined;
  useEffect(() => {
    if (narrow && sheetOpen) sheetTop.current?.scrollIntoView({ block: 'start' });
  }, [narrow, sheetOpen, selected, judgeIndex]);
  const movements = useMemo(() => programme?.sections.flatMap(s => s.movements) ?? [], [programme]);
  // What is entered for each judge: the marks, or one total.
  const fields = v?.totals_per_judge ? 1 : movements.length;
  const lang = language;

  // Alt+↑↓: the previous / next rider.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (!e.altKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') || !rider) return;
      e.preventDefault();
      const i = riders.findIndex(r => keyOf(r) === keyOf(rider));
      const next = riders[i + (e.key === 'ArrowDown' ? 1 : -1)];
      if (next) setSelected(keyOf(next));
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [rider, riders]);

  if (view.error) return <ErrorNote error={view.error} />;
  if (!v) return null;

  const classProgramme = Object.keys(v.programmes).length === 0 && !riders.some(r => r.programme !== null);
  // Marks entered (or cut): the test stays.
  const hasMarks = v.riders.some(r => Object.keys(r.sheets).length > 0) || v.aside.length > 0;
  const programmePicker = (
    <ProgrammePicker
      show={show}
      classId={classId}
      value={programme?.id ?? v.riders[0]?.programme ?? null}
      onChange={p => change('edit_class', { class: classId, programme: p })}
      disabled={locked || hasMarks}
    />
  );
  // Children and young horses: one sheet for the judges, entered as the first judge's.
  const judges = oneSheet(programme?.kind) ? v.judges.slice(0, 1) : v.judges;
  // A cut sheet: its number, and its first marks (or its total) to know the paper sheet.
  const cutName = (a: Cut) => `${t('clipboard_sheet')} ${a.slot}`;
  const cutMarks = (a: Cut) => (a.total ? a.total : `${a.marks} ${plural(a.marks, 'marks')} : ${a.first.join(' · ')}${a.marks > a.first.length ? ' …' : ''}`);
  const paste = (slot: number, j: Judge) => change('move_sheet', { class: classId, from: { aside: slot }, to: { entry: rider!.entry, judge: j.assignment } });
  const toolbar = (
    <div className="flex flex-wrap items-center gap-3 border-b border-line bg-card px-3 py-2.5 text-[13px] sm:px-6">
      {/* The class's test, from a searchable list; not once there are marks (its settings too). */}
      <div className="flex min-w-0 items-center gap-2 phone:w-full">
        <span className="text-xs font-semibold text-ink-2">{t('programme')}</span>
        {hasMarks && !locked ? (
          <Tooltip label={t('programme_locked')} side="bottom">
            <div className="w-[min(60vw,26rem)] phone:w-auto phone:min-w-0 phone:flex-1">{programmePicker}</div>
          </Tooltip>
        ) : (
          <div className="w-[min(60vw,26rem)] phone:w-auto phone:min-w-0 phone:flex-1">{programmePicker}</div>
        )}
      </div>
      <Tooltip label={t('fast_hint')} side="bottom">
        <label className="flex cursor-pointer items-center gap-2 text-xs font-semibold text-ink-2">
          <input
            type="checkbox"
            className="check"
            checked={fast}
            onChange={e => {
              setFast(e.target.checked);
              localStorage.setItem('fast-marks', e.target.checked ? '1' : '0');
            }}
          />
          {t('fast_entry')}
        </label>
      </Tooltip>
      <span className="hidden text-xs text-ink-4 lg:inline">{t('marks_keys')}</span>
      <QuickJump
        onJump={n => {
          // The rider's sheet, at the first judge's first mark.
          const r = findStarter(riders, n);
          if (!r) return false;
          setSelected(keyOf(r));
          if (narrow) {
            setSheetOpen(true);
            setJudgeIndex(0);
          }
          setTimeout(() => inputs.current.get(`${v.judges[0]?.assignment}:0`)?.focus(), 80);
          return true;
        }}
      />
      {v.aside.length > 0 && (
        <Menu width="w-72" trigger={<IconButton icon={ClipboardList} label={t('clipboard')} badge={v.aside.length} variant="secondary" size="sm" className="ml-auto text-warn" />}>
          {v.aside.map((a, i) => (
            <div key={a.slot}>
              {i > 0 && <MenuSeparator />}
              <MenuLabel>
                {cutName(a)} · {cutMarks(a)}
              </MenuLabel>
              {!locked &&
                rider &&
                !rider.team &&
                judges.map(j => (
                  <MenuItem key={j.assignment} icon={ClipboardPaste} hint={`${rider.start_number ?? rider.position} ${rider.rider} · ${j.name}`} onSelect={() => paste(a.slot, j)}>
                    {t('paste_for')} {j.letter}
                  </MenuItem>
                ))}
              {!locked && (
                <MenuItem icon={Trash2} danger onSelect={() => change('delete_sheet', { class: classId, aside: a.slot })}>
                  {t('delete_cut')}
                </MenuItem>
              )}
            </div>
          ))}
        </Menu>
      )}
    </div>
  );

  if (v.judges.length === 0)
    return (
      <div>
        {toolbar}
        <p className="p-6 text-ink-2">{t('no_judges')}</p>
      </div>
    );
  if (classProgramme)
    return (
      <div>
        {toolbar}
        <p className="p-6 text-ink-2">{t('no_programme')}</p>
      </div>
    );

  const focusCell = (judge: number, movement: number) => {
    const j = judges[judge];
    const el = j && inputs.current.get(`${j.assignment}:${movement}`);
    el?.focus();
  };
  // On a phone one judge is shown: `judge` counts from it; going past its last mark shows the next.
  const base = narrow ? Math.min(judgeIndex, judges.length - 1) : 0;
  const move = (judge: number, movement: number, step: number) => {
    let [j, m] = [base + judge, movement + step];
    if (m >= fields) [j, m] = [j + 1, 0];
    if (m < 0) [j, m] = [j - 1, fields - 1];
    if (j >= judges.length) {
      // The rider's last mark: on to the next rider's first.
      const i = riders.findIndex(r => keyOf(r) === keyOf(rider!));
      const next = riders[i + 1];
      if (next) {
        setSelected(keyOf(next));
        if (narrow) setJudgeIndex(0);
        setTimeout(() => focusCell(0, 0), 50);
      }
      return;
    }
    if (j < 0) return;
    if (narrow && j !== base) {
      setJudgeIndex(j);
      setTimeout(() => focusCell(j, m), 50);
    } else focusCell(j, m);
  };
  const shown = narrow ? judges.slice(base, base + 1) : judges;
  const save = (judge: Judge, number: string, value: string) => change('edit_sheet', { class: classId, ...whose(rider!), judge: judge.assignment, marks: { [number]: value === '' ? null : value } });
  const saveTotal = (judge: Judge, value: string) => change('edit_sheet', { class: classId, entry: rider!.entry, judge: judge.assignment, marks: {}, total: value === '' ? null : value });

  // A rider's judges: Children, the one sheet.
  const ridersJudges = (r: Rider) => (r.programme !== null && oneSheet(v.programmes[String(r.programme)]?.kind) ? v.judges.slice(0, 1) : v.judges);
  // Every judge's sheet complete: the percentage is the rider's; before, only an indication.
  const complete = (r: Rider) => ridersJudges(r).every(j => r.sheets[j.assignment]?.complete);
  const parts = (r: Rider) =>
    r.part_a !== null || r.part_b !== null ? (
      <span className="text-xs font-normal text-ink-3 tabular">
        <span title={t('part_a')}>A {r.part_a ?? '–'}%</span> · <span title={t('part_b')}>B {r.part_b ?? '–'}%</span>
      </span>
    ) : null;
  const chip = (r: Rider, j: Judge) => {
    const s = r.sheets[j.assignment];
    const tone = !s ? 'text-ink-3 border-line' : s.complete ? 'border-ok/50 bg-ok/15 text-ok' : 'border-warn/50 bg-warn/15 text-warn';
    return (
      <span key={j.assignment} className={`grid h-5 min-w-5 place-items-center rounded-md border px-1 text-[10px] font-bold ${tone}`}>
        {j.letter}
      </span>
    );
  };

  // What can be done with a judge's sheet: send it elsewhere (swapped with one there), cut it
  // (to the clipboard), paste a cut one here, delete it. Team sheets stay with their team: deleted
  // only.
  const sheetTools = (j: Judge) => {
    if (!rider || locked) return null;
    const s = rider.sheets[j.assignment];
    const pastes = !rider.team && v.aside.length > 0;
    if (!s && !pastes) return null;
    const pasteHint = t(s ? 'paste_swap_hint' : 'paste_hint');
    return (
      <div className="mt-1 flex gap-0.5 pointer-coarse:gap-1.5">
        {s && !rider.team && (
          <>
            <IconButton icon={ArrowLeftRight} size="xs" label={t('send_hint')} onClick={() => setMoving({ judge: j.assignment })} />
            <IconButton icon={Scissors} size="xs" label={t('cut_hint')} onClick={() => change('move_sheet', { class: classId, from: { entry: rider.entry, judge: j.assignment } })} />
          </>
        )}
        {pastes &&
          (v.aside.length === 1 ? (
            <IconButton icon={ClipboardPaste} size="xs" label={pasteHint} className="text-warn" onClick={() => paste(v.aside[0].slot, j)} />
          ) : (
            <Menu width="w-72" align="start" trigger={<IconButton icon={ClipboardPaste} size="xs" label={pasteHint} className="text-warn" />}>
              {v.aside.map(a => (
                <MenuItem key={a.slot} icon={ClipboardPaste} hint={cutMarks(a)} onSelect={() => paste(a.slot, j)}>
                  {cutName(a)}
                </MenuItem>
              ))}
            </Menu>
          ))}
        {s && (
          <IconButton icon={Trash2} size="xs" variant="danger-ghost" label={t('delete_sheet_hint')} onClick={() => change('delete_sheet', { class: classId, ...whose(rider), judge: j.assignment })} />
        )}
      </div>
    );
  };
  const step = (by: number) => {
    const i = riders.findIndex(r => rider !== undefined && keyOf(r) === keyOf(rider));
    const next = riders[i + by];
    if (next) setSelected(keyOf(next));
  };

  const riderList = (
    <div className={narrow ? 'w-full bg-card' : 'w-80 shrink-0 overflow-auto border-r border-line bg-card'}>
      {riders.map(r => (
        <button
          key={keyOf(r)}
          onClick={() => {
            setSelected(keyOf(r));
            if (narrow) setSheetOpen(true);
          }}
          className={`block w-full border-b border-line px-3 py-2 text-left transition-colors pointer-coarse:py-3 ${rider !== undefined && keyOf(rider) === keyOf(r) && !narrow ? 'bg-accent-soft shadow-[inset_3px_0_0_var(--accent)]' : 'hover:bg-hover'}`}
        >
          <div className="flex items-center gap-2">
            <span className="w-8 text-right text-xs text-ink-3 tabular">{r.start_number ?? r.position}</span>
            <span className="min-w-0 flex-1 truncate font-semibold">{r.rider}</span>
            <span className={`text-xs tabular ${complete(r) ? '' : 'text-ink-3 italic'}`} title={complete(r) ? undefined : t('partial')}>
              {r.percentage ? (v.driving ? `${r.percentage} ${t('points_160')}` : `${r.percentage}%`) : ''}
            </span>
          </div>
          <div className="flex items-center gap-2 pl-10">
            <span className="min-w-0 flex-1 truncate text-xs text-ink-3">{r.horse}</span>
            <span className="flex gap-0.5">{ridersJudges(r).map(j => chip(r, j))}</span>
          </div>
        </button>
      ))}
    </div>
  );

  // Out of the class (retired, eliminated…): a rider's, or the team's in team dressage.
  const status = rider && (
    <div className="flex items-center gap-2">
      <label className="text-xs text-ink-3">{t('status_col')}</label>
      <select
        value={rider.out === null ? '' : String(rider.out)}
        disabled={locked}
        onChange={e => change('edit_result', { class: classId, ...whose(rider), values: {}, out: e.target.value === '' ? null : Number(e.target.value) })}
        className={`field field-sm w-auto ${rider.out !== null ? 'font-semibold text-warn' : ''}`}
      >
        <option value="">{t('in_class')}</option>
        {v.reasons.map(r => (
          <option key={r.code} value={r.code}>
            {reason(r.name)}
          </option>
        ))}
      </select>
    </div>
  );
  // The penalties: one set for every judge (a box ticked counts on every sheet; the exclusion box,
  // with no value, switches the rider to eliminated).
  const penalties = rider && programme && (
    <div className="flex flex-wrap gap-x-4 gap-y-0.5 phone:mt-1 phone:grid phone:gap-0 phone:font-normal phone:text-ink">
      {programme.penalties.map((p, i) => (
        <label key={i} className="flex min-w-0 items-center gap-1 text-[11px] pointer-coarse:gap-2 pointer-coarse:py-1.5 pointer-coarse:text-[13px]" title={say(lang, p.cost_fr, p.cost_de, p.cost_it)}>
          <input
            type="checkbox"
            className="check"
            disabled={locked}
            checked={rider.penalties.includes(i)}
            onChange={e => change('set_penalties', { class: classId, ...whose(rider), penalties: e.target.checked ? [...rider.penalties, i] : rider.penalties.filter(x => x !== i) })}
          />
          <span className="truncate">{say(lang, p.fr, p.de, p.it)}</span>
          <span className="shrink-0 text-ink-4">{say(lang, p.cost_fr, p.cost_de, p.cost_it)}</span>
        </label>
      ))}
    </div>
  );
  // On a phone the head stays on top while the marks scroll under it: who, the result, the judge
  // shown and the next rider; the status and the sheet's tools below it, scrolling away.
  const sheet = rider && programme && (
    <div ref={sheetTop} className="min-w-0 flex-1 overflow-auto phone:overflow-visible">
      <div className="sticky top-0 z-10 border-b border-line bg-card/95 px-3 py-3 backdrop-blur-sm sm:px-5 phone:py-2">
        <div className="flex items-center gap-3 phone:gap-2">
          {narrow && (
            <button onClick={() => setSheetOpen(false)} className="-ml-1 grid size-10 shrink-0 place-items-center rounded-lg text-ink-3 hover:bg-hover" aria-label={t('riders')}>
              <ChevronLeft className="size-5" />
            </button>
          )}
          <div className="grid size-11 shrink-0 place-items-center rounded-xl bg-sunken text-lg font-bold tabular ring-1 ring-line phone:size-10 phone:text-base">
            {rider.start_number ?? rider.position}
          </div>
          <div className="min-w-0">
            <div className="truncate text-[15px] font-semibold">{rider.rider}</div>
            <div className="truncate text-xs text-ink-3">
              {rider.horse}
              {!narrow && ` · ${programme.name} ${say(lang, programme.title_fr, programme.title_de, programme.title_it)}`}
            </div>
          </div>
          <div className="ml-auto shrink-0 text-right">
            <div className={`text-xl font-bold tabular phone:text-lg ${complete(rider) ? 'text-ink' : 'text-ink-3 italic'}`} title={complete(rider) ? undefined : t('partial')}>
              {rider.percentage ? (v.driving ? `${rider.percentage} ${t('points_160')}` : `${rider.percentage}%`) : '–'}
            </div>
            {parts(rider)}
            {rider.rank && (
              <div className="text-xs text-ink-3">
                {t('rank')} {rider.rank}
              </div>
            )}
          </div>
        </div>
        {!narrow && <div className="mt-1">{status}</div>}
        {narrow && (
          <div className="mt-2 flex items-center gap-1.5">
            {judges.map((j, i) => {
              const s = rider.sheets[j.assignment];
              const tone = !s ? 'border-line' : s.complete ? 'border-ok/60 text-ok' : 'border-warn/60 text-warn';
              return (
                <button
                  key={j.assignment}
                  onClick={() => (setJudgeIndex(i), setMoving(null))}
                  className={`min-h-10 min-w-10 rounded-lg border-2 font-bold ${i === base ? 'border-accent bg-accent text-on-accent' : `${tone} bg-card`}`}
                >
                  {j.letter}
                </button>
              );
            })}
            <span className="ml-1 min-w-0 flex-1 truncate text-xs text-ink-3">
              {shown[0]?.name}
              {shown[0] && rider.judge_riders?.[shown[0].assignment] && ` → ${rider.judge_riders[shown[0].assignment]}`}
            </span>
            <button onClick={() => step(-1)} disabled={riders[0] === rider} className="btn btn-sm btn-secondary" aria-label={t('rider_previous')}>
              <ChevronUp className="size-4" />
            </button>
            <button onClick={() => step(1)} disabled={riders[riders.length - 1] === rider} className="btn btn-sm btn-secondary" aria-label={t('rider_next')}>
              <ChevronDown className="size-4" />
            </button>
          </div>
        )}
      </div>
      {narrow && (
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-line px-3 py-2">
          {status}
          {shown[0] && sheetTools(shown[0])}
        </div>
      )}
      <table className="w-full table-fixed text-sm">
        <thead className={narrow ? '' : 'sticky top-[57px] z-10 bg-card'}>
          <tr>
            <th className="w-10 border-b border-line bg-raised px-2 py-2 text-left text-xs font-semibold text-ink-3 sm:w-12">#</th>
            <th className="border-b border-line bg-raised px-2 py-2 text-left text-xs font-semibold text-ink-3">{t('movement')}</th>
            <th className="w-8 border-b border-line bg-raised px-1 py-2 text-xs font-semibold text-ink-3 sm:w-10">×</th>
            {shown.map(j => (
              <th key={j.assignment} className="w-20 border-b border-l border-line bg-raised px-2 py-2 text-left align-bottom sm:w-36">
                <div className="text-base font-bold">{oneSheet(programme.kind) ? v.judges.map(x => x.letter).join(' ') : j.letter}</div>
                {!narrow && (
                  <>
                    <div className="truncate text-[11px] font-normal text-ink-3">{j.name}</div>
                    {rider.judge_riders?.[j.assignment] && <div className="truncate text-[11px] font-semibold text-ink-2">→ {rider.judge_riders[j.assignment]}</div>}
                    {sheetTools(j)}
                  </>
                )}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {v.totals_per_judge && (
            <tr>
              <td colSpan={3} className="border-b border-line px-2 py-1.5 text-ink-2">
                {t('judge_points')} <span className="text-xs text-ink-3">(max. {programme.max_points})</span>
              </td>
              {shown.map((j, ji) => (
                <td key={j.assignment} className="border-b border-l border-line px-2 py-1">
                  <MarkInput
                    value={rider.sheets[j.assignment]?.entered ?? ''}
                    fast={false}
                    max={Number(programme.max_points)}
                    disabled={locked}
                    onSave={value => saveTotal(j, value)}
                    onMove={step => move(ji, 0, step)}
                    register={el => {
                      const key = `${j.assignment}:0`;
                      if (el) inputs.current.set(key, el);
                      else inputs.current.delete(key);
                    }}
                  />
                </td>
              ))}
            </tr>
          )}
          {!v.totals_per_judge &&
            programme.sections.map((section, si) => (
              <SectionRows
                key={si}
                title={say(lang, section.title_fr, section.title_de, section.title_it)}
                movements={section.movements}
                offset={programme.sections.slice(0, si).reduce((n, s) => n + s.movements.length, 0)}
                judges={shown}
                rider={rider}
                lang={lang}
                fast={fast}
                locked={locked}
                narrow={narrow}
                save={save}
                move={move}
                inputs={inputs.current}
              />
            ))}
          {programme.penalties.length > 0 && (
            <tr>
              {/* (A phone: under the title, the whole width.) */}
              <td colSpan={narrow ? 3 + shown.length : 3} className="border-t-2 border-line px-2 py-2 align-top text-xs font-semibold text-ink-3">
                {t('penalties')}
                {narrow && penalties}
              </td>
              {!narrow && (
                <td colSpan={shown.length} className="border-l border-t-2 border-line px-2 py-1 align-top">
                  {penalties}
                </td>
              )}
            </tr>
          )}
          <tr className="font-bold">
            <td colSpan={3} className="border-t-2 border-line px-2 py-1 text-right">
              {t('total')}
            </td>
            {shown.map(j => {
              const s = rider.sheets[j.assignment];
              return (
                <td key={j.assignment} className="border-l border-t-2 border-line px-2 py-1 text-right tabular">
                  {s?.total ?? '–'} <span className="text-xs font-normal text-ink-3">{s?.percent ? `${s.percent}%` : ''}</span>
                </td>
              );
            })}
          </tr>
        </tbody>
      </table>
    </div>
  );

  return (
    <div className="flex h-full flex-col phone:h-auto">
      {toolbar}
      <div className="flex min-h-0 flex-1">
        {narrow ? (
          sheetOpen && sheet ? (
            sheet
          ) : (
            riderList
          )
        ) : (
          <>
            {riderList}
            {sheet}
          </>
        )}
      </div>
      <SendSheet riders={v.riders} judges={v.judges} from={moving && rider ? { entry: rider.entry, judge: moving.judge } : null} done={() => setMoving(null)} classId={classId} />
    </div>
  );
}

function SectionRows(props: {
  title: string;
  movements: Movement[];
  offset: number;
  judges: Judge[];
  rider: Rider;
  lang: Language;
  fast: boolean;
  locked: boolean;
  narrow: boolean;
  save: (judge: Judge, number: string, value: string) => void;
  move: (judge: number, movement: number, step: number) => void;
  inputs: Map<string, HTMLInputElement>;
}) {
  const { title, movements, offset, judges, rider, lang, fast, locked, narrow, save, move, inputs } = props;
  return (
    <>
      {title && (
        <tr>
          <td colSpan={3 + judges.length} className="border-b border-line bg-raised px-2 pt-3 pb-1.5 text-xs font-bold text-ink-2">
            {title}
          </td>
        </tr>
      )}
      {movements.map((m, i) => (
        <tr key={m.number} className="hover:bg-hover">
          <td className="border-b border-line px-2 py-0.5 text-ink-3 tabular">{m.number}</td>
          <td className={`max-w-0 border-b border-line px-2 py-0.5 ${narrow ? '' : 'truncate'}`} title={say(lang, m.fr, m.de, m.it)}>
            {narrow ? <div className="py-1 text-xs leading-snug">{say(lang, m.fr, m.de, m.it)}</div> : say(lang, m.fr, m.de, m.it)}
          </td>
          <td className="border-b border-line px-1 py-0.5 text-center text-xs text-ink-3">{m.coefficient !== '1' ? m.coefficient : ''}</td>
          {judges.map((j, ji) => (
            <td key={j.assignment} className="border-b border-l border-line px-2 py-0.5">
              <MarkInput
                value={rider.sheets[j.assignment]?.marks[m.number] ?? ''}
                fast={fast}
                disabled={locked}
                onSave={value => save(j, m.number, value)}
                onMove={step => move(ji, offset + i, step)}
                register={el => {
                  const key = `${j.assignment}:${offset + i}`;
                  if (el) inputs.set(key, el);
                  else inputs.delete(key);
                }}
              />
            </td>
          ))}
        </tr>
      ))}
    </>
  );
}

/** Send a judge's sheet to another rider and judge; a sheet already there comes back here. */
/** A judge's sheet sent to another rider or judge (swapped with the sheet there, if any). */
function SendSheet({ riders, judges, from, done, classId }: { riders: Rider[]; judges: Judge[]; from: { entry: number; judge: string } | null; done: () => void; classId: string }) {
  const t = useT();
  const { change } = useChanges();
  const [entry, setEntry] = useState(from?.entry ?? 0);
  const [judge, setJudge] = useState(from?.judge ?? '');
  useEffect(() => {
    if (from) {
      setEntry(from.entry);
      setJudge(from.judge);
    }
  }, [from]);
  const target = riders.find(r => r.entry === entry);
  const occupied = !!from && target?.sheets[judge] !== undefined && !(entry === from.entry && judge === from.judge);
  const source = riders.find(r => r.entry === from?.entry);
  const sourceJudge = judges.find(j => j.assignment === from?.judge);
  return (
    <Modal
      open={from !== null}
      onClose={done}
      title={t('send_sheet')}
      description={source ? `${sourceJudge?.letter ?? ''} · ${source.start_number ?? source.position} ${source.rider}` : undefined}
      icon={Send}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={done}>
            {t('cancel')}
          </Button>
          <Button
            variant="primary"
            onClick={async () => {
              if (from && (await change('move_sheet', { class: classId, from, to: { entry, judge } }))) done();
            }}
          >
            {occupied ? t('swap') : t('send')}
          </Button>
        </>
      }
    >
      <div className="grid gap-4">
        <Field label={t('rider')}>
          <select value={entry} onChange={e => setEntry(Number(e.target.value))} className="field">
            {riders.map(r => (
              <option key={r.entry} value={r.entry}>
                {r.start_number ?? r.position} {r.rider}
              </option>
            ))}
          </select>
        </Field>
        <Field label={t('judge')}>
          <select value={judge} onChange={e => setJudge(e.target.value)} className="field">
            {judges.map(j => (
              <option key={j.assignment} value={j.assignment}>
                {j.letter} {j.name}
              </option>
            ))}
          </select>
        </Field>
        {occupied && <Callout tone="warn">{t('swap_note')}</Callout>}
      </div>
    </Modal>
  );
}
