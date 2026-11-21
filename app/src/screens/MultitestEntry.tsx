// Driving's multitest: entering the judges' sheets as Rosson's form does (Rosson's programmes
// B1 and B4: items 1… with their coefficients, marks 0 to 10 in tenths; penalties A for the
// dressage, B per obstacle in halves, C for the ground test). The drivers in start order on the
// left; the selected driver's sheets side by side, a column per judge (on a phone: one judge at
// a time). Enter goes down the column, then to the next judge, then to the next driver.

import { useEffect, useMemo, useRef, useState } from 'react';
import { call, type ClassRow } from '../api';
import { useReason, useT } from '../i18n';
import { useChanges } from '../parts/changes';
import { ErrorNote, useLoad, useNarrow } from '../parts/common';

interface Judge {
  assignment: string;
  letter: string;
  name: string;
}
interface Line {
  entry: number;
  position: number;
  start_number: number | null;
  rider: string;
  horse: string;
  not_starting: boolean;
  out: number | null;
  marks: Record<string, Record<string, string>>;
  totals: Record<string, string>;
  total: string | null;
  rank: number | null;
  placed: boolean;
}
interface View {
  class: ClassRow;
  programme: string;
  items: { number: string; coefficient: string }[];
  judges: Judge[];
  reasons: { code: number; name: string }[];
  lines: Line[];
}

/** The fields of a judge's sheet, in entry order: the items, then penalties A, B, C. */
type Field = { key: string; kind: 'item' | 'whole' | 'half' };
const PENALTIES: Field[] = [
  { key: 'strafpunkte_a', kind: 'whole' },
  { key: 'strafpunkte_b', kind: 'half' },
  { key: 'strafpunkte_c', kind: 'whole' },
];
const PATTERN: Record<Field['kind'], RegExp> = { item: /^\d{1,2}([.,]\d)?$/, whole: /^\d{1,3}$/, half: /^\d{1,3}([.,](0|5))?$/ };

function Cell({
  field,
  value,
  disabled,
  onSave,
  onMove,
  register,
}: {
  field: Field;
  value: string;
  disabled: boolean;
  onSave: (v: string) => void;
  onMove: (step: number) => void;
  register: (el: HTMLInputElement | null) => void;
}) {
  const [text, setText] = useState(value);
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setText(value);
  }, [value]);
  const fits = (t: string) => PATTERN[field.kind].test(t.trim()) && (field.kind !== 'item' || Number(t.trim().replace(',', '.')) <= 10);
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
      onChange={e => setText(e.target.value)}
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
      className={`h-8 w-full rounded-md border bg-card px-2 text-right text-[13px] font-semibold tabular shadow-xs outline-none transition-[border-color,box-shadow] focus:border-info focus:ring-3 focus:ring-info/20 pointer-coarse:h-11 ${valid ? 'border-line-strong' : 'border-danger bg-danger/8'} ${disabled ? 'opacity-50' : ''}`}
    />
  );
}

export default function MultitestEntry({ show, classId }: { show: string; classId: string }) {
  const t = useT();
  const reason = useReason();
  // Out: why, in the office's language.
  const outName = (code: number) => reason(view.data?.reasons.find(r => r.code === code)?.name ?? '') || t('out');
  const { version, change } = useChanges();
  const view = useLoad(() => call<View>('multitest', { show, class: classId }), [show, classId, version]);
  const [selected, setSelected] = useState<number | null>(null);
  const inputs = useRef<Map<string, HTMLInputElement>>(new Map());
  const narrow = useNarrow();
  const [sheetOpen, setSheetOpen] = useState(false);
  const [judgeIndex, setJudgeIndex] = useState(0);
  const v = view.data;
  const locked = v?.class.status === 'final';
  const drivers = useMemo(() => (v?.lines ?? []).filter(l => !l.not_starting), [v]);
  const driver = drivers.find(l => l.entry === selected) ?? drivers[0];
  const fields: Field[] = useMemo(() => [...(v?.items ?? []).map(i => ({ key: i.number, kind: 'item' as const })), ...PENALTIES], [v]);

  if (view.error) return <ErrorNote error={view.error} />;
  if (!v) return null;
  if (v.judges.length === 0) return <p className="p-4 text-sm text-ink-3">{t('no_judges_driving')}</p>;

  const judges = narrow ? [v.judges[Math.min(judgeIndex, v.judges.length - 1)]] : v.judges;
  const focus = (entry: number, judge: string, row: number) => setTimeout(() => inputs.current.get(`${entry}:${judge}:${row}`)?.focus(), 30);
  // Down the column; past the last field, the next judge's first; past the last judge, the next driver.
  const move = (judge: string, row: number, step: number) => {
    if (!driver) return;
    const next = row + step;
    if (next >= 0 && next < fields.length) return focus(driver.entry, judge, next);
    const j = v.judges.findIndex(x => x.assignment === judge);
    if (step > 0 && j + 1 < v.judges.length) {
      if (narrow) setJudgeIndex(j + 1);
      return focus(driver.entry, v.judges[j + 1].assignment, 0);
    }
    if (step > 0) {
      const i = drivers.findIndex(d => d.entry === driver.entry);
      const nextDriver = drivers[i + 1];
      if (nextDriver) {
        setSelected(nextDriver.entry);
        setJudgeIndex(0);
        focus(nextDriver.entry, v.judges[0].assignment, 0);
      }
    }
  };
  const save = (judge: string, key: string, value: string) => change('edit_sheet', { class: classId, entry: driver.entry, judge, marks: { [key]: value === '' ? null : value } });
  const complete = (l: Line) => v.judges.every(j => l.totals[j.assignment] !== undefined);
  const chip = (l: Line, j: Judge) => {
    const marks = l.marks[j.assignment];
    const tone = !marks ? 'text-ink-3 border-line' : l.totals[j.assignment] !== undefined ? 'border-ok/50 bg-ok/15 text-ok' : 'border-warn/50 bg-warn/15 text-warn';
    return (
      <span key={j.assignment} className={`rounded border px-1 text-[11px] font-bold ${tone}`}>
        {j.letter}
      </span>
    );
  };
  const penaltyLabel: Record<string, string> = { strafpunkte_a: t('mt_pen_a'), strafpunkte_b: t('mt_pen_b'), strafpunkte_c: t('mt_pen_c') };
  const coefficient = (key: string) => v.items.find(i => i.number === key)?.coefficient;

  const list = (
    <div className={narrow ? 'h-full w-full overflow-auto' : 'w-80 shrink-0 overflow-auto border-r border-line'}>
      {drivers.map(l => (
        <button
          key={l.entry}
          onClick={() => {
            setSelected(l.entry);
            if (narrow) setSheetOpen(true);
          }}
          className={`block w-full border-b border-line px-3 py-1.5 text-left pointer-coarse:py-2.5 ${driver?.entry === l.entry && !narrow ? 'bg-accent/10 outline outline-2 -outline-offset-2 outline-accent' : 'hover:bg-hover'}`}
        >
          <div className="flex items-center gap-2">
            <span className="w-8 text-right text-xs text-ink-3 tabular">{l.start_number ?? l.position}</span>
            <span className="min-w-0 flex-1 truncate font-medium">{l.rider}</span>
            <span className={`text-xs tabular ${complete(l) ? '' : 'text-ink-3 italic'}`} title={complete(l) ? undefined : t('partial')}>
              {l.out !== null ? outName(l.out) : (l.total ?? '')}
            </span>
          </div>
          <div className="flex items-center gap-2 pl-10">
            <span className="min-w-0 flex-1 truncate text-xs text-ink-3">{l.horse}</span>
            <span className="flex gap-0.5">{v.judges.map(j => chip(l, j))}</span>
          </div>
        </button>
      ))}
    </div>
  );

  const th = 'border-b border-line bg-raised px-2 py-2.5 text-left text-xs font-semibold text-ink-3';
  const td = 'border-b border-line px-2 py-1.5 align-middle';
  const sheet = driver && (
    <div className="min-w-0 flex-1 overflow-auto">
      <div className="sticky top-0 z-10 border-b border-line bg-card px-3 py-2 sm:px-4">
        <div className="flex items-center gap-3">
          {narrow && (
            <button onClick={() => setSheetOpen(false)} className="-ml-1 min-h-9 rounded-lg px-2 text-lg text-ink-3 hover:bg-hover" aria-label={t('riders')}>
              ‹
            </button>
          )}
          <div className="text-lg font-bold tabular">{driver.start_number ?? driver.position}</div>
          <div className="min-w-0">
            <div className="truncate font-semibold">{driver.rider}</div>
            <div className="truncate text-xs text-ink-3">
              {driver.horse} · {t('phase_multitest')} {v.programme}
            </div>
          </div>
          <div className="ml-auto text-right">
            <div className={`text-lg font-bold tabular ${complete(driver) ? '' : 'text-ink-3 italic'}`}>{driver.out !== null ? outName(driver.out) : (driver.total ?? '–')}</div>
            {driver.rank !== null && (
              <div className={`text-xs ${driver.placed ? 'font-bold text-accent-dark' : 'text-ink-3'}`}>
                {driver.rank}. {t('rank')}
              </div>
            )}
          </div>
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <label className="text-xs text-ink-3">{t('status_col')}</label>
          <select
            value={driver.out === null ? '' : String(driver.out)}
            disabled={locked}
            onChange={e => change('edit_result', { class: classId, entry: driver.entry, values: {}, out: e.target.value === '' ? null : Number(e.target.value) })}
            className={`field field-sm w-auto text-xs pointer-coarse:py-2 ${driver.out !== null ? 'font-semibold text-warn' : ''}`}
          >
            <option value="">{t('in_class')}</option>
            {v.reasons.map(r => (
              <option key={r.code} value={r.code}>
                {reason(r.name)}
              </option>
            ))}
          </select>
          {narrow && v.judges.length > 1 && (
            <div className="ml-auto flex gap-1">
              {v.judges.map((j, i) => (
                <button
                  key={j.assignment}
                  onClick={() => setJudgeIndex(i)}
                  className={`min-h-9 min-w-9 rounded border px-2 text-sm font-bold ${i === Math.min(judgeIndex, v.judges.length - 1) ? 'border-accent bg-accent text-on-accent' : 'border-line'}`}
                >
                  {j.letter}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
      <table className="text-sm">
        <thead>
          <tr>
            <th className={th}>{t('mt_item')}</th>
            <th className={`${th} w-14 text-right`}>{t('coeff_short')}</th>
            {judges.map(j => (
              <th key={j.assignment} className={`${th} w-32 min-w-28`}>
                <span className="font-bold text-ink">{j.letter}</span> <span className="normal-case">{j.name}</span>
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="tabular">
          {fields.map((f, row) => (
            <tr key={f.key} className={f.kind === 'item' ? '' : 'bg-page/40'}>
              <td className={td}>{f.kind === 'item' ? `${t('mt_number')} ${f.key}` : penaltyLabel[f.key]}</td>
              <td className={`${td} text-right text-ink-3`}>{coefficient(f.key) ?? ''}</td>
              {judges.map(j => (
                <td key={j.assignment} className={td}>
                  <Cell
                    field={f}
                    value={driver.marks[j.assignment]?.[f.key] ?? ''}
                    disabled={locked}
                    onSave={value => save(j.assignment, f.key, value)}
                    onMove={step => move(j.assignment, row, step)}
                    register={el => {
                      const k = `${driver.entry}:${j.assignment}:${row}`;
                      if (el) inputs.current.set(k, el);
                      else inputs.current.delete(k);
                    }}
                  />
                </td>
              ))}
            </tr>
          ))}
          <tr>
            <td className={`${td} font-semibold`} colSpan={2}>
              {t('total')}
            </td>
            {judges.map(j => (
              <td key={j.assignment} className={`${td} text-right font-semibold`}>
                {driver.totals[j.assignment] ?? ''}
              </td>
            ))}
          </tr>
        </tbody>
      </table>
    </div>
  );

  if (narrow) return <div className="h-full">{sheetOpen ? sheet : list}</div>;
  return (
    <div className="flex h-full">
      {list}
      {sheet}
    </div>
  );
}
