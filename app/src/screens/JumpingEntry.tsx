// Show-jumping results: the timekeeper's start list out and results file in, and every result
// entered or corrected by hand, in start order. The ranking follows as it is typed.

import { useEffect, useRef, useState } from 'react';
import { call, chooseFile, saveFile, type ResultLine, type ResultsView } from '../api';
import { useReason, useT, type Key } from '../i18n';
import { useChanges } from '../parts/changes';
import { ErrorNote, useLoad, useNarrow } from '../parts/common';
import { findStarter, QuickJump } from '../parts/jump';
import { ScoringTablePicker, type ScoringTableChoice } from '../parts/pickers';
import { Download, Timer, Upload, X } from 'lucide-react';
import { Button, IconButton, Menu, MenuCheck, MenuItem, MenuSeparator, Pill, Tooltip } from '../ui';

// A number; a time also in minutes and seconds ("5:12", cross-country).
const valid = (text: string) => text.trim() === '' || /^\d+([.,]\d+)?$/.test(text.trim()) || /^\d+:[0-5]\d([.,]\d+)?$/.test(text.trim()) || /^\d{1,2}:[0-5]\d:[0-5]\d([.,]\d+)?$/.test(text.trim());

/** Seconds since midnight as the time of day ("36610" → "10:10:10"). */
const timeOfDay = (seconds: string) => {
  const n = Number(seconds);
  if (seconds === '' || !Number.isFinite(n)) return seconds;
  const whole = Math.floor(n);
  const rest = seconds.includes('.') ? seconds.slice(seconds.indexOf('.')) : '';
  const two = (x: number) => String(x).padStart(2, '0');
  return `${Math.floor(whole / 3600)}:${two(Math.floor(whole / 60) % 60)}:${two(whole % 60)}${rest}`;
};

/** A driving field's label (marathon, cones, derby, open); null for jumping's fields. `seconds`:
 *  cones in Wertung B, the faults are seconds (5 s a ball, not 3 points). */
function drivingLabel(field: string, t: (k: Key) => string, rounds: number, seconds = false): string | null {
  const what: Record<string, Key> = {
    start: 'field_start',
    finish: 'field_finish',
    penalties: 'pen_short',
    neutralised: 'field_neutralised',
    time: 'time_short',
    zeit: 'time_short',
    hindernis: 'field_obstacle_faults',
    andere: 'field_other_faults',
    abwuerfe: 'field_knockdowns',
    uebrige: 'field_others',
    sekunden: 'field_seconds',
    punkte: 'field_points',
  };
  let m = /^([ab])_(start|finish|penalties|neutralised)$/.exec(field);
  if (m) return `${m[1].toUpperCase()} · ${t(what[m[2]])}`;
  m = /^h(\d+)_(time|penalties|neutralised)$/.exec(field);
  if (m) return `H${m[1]} · ${t(what[m[2]])}`;
  // The presentation: each judge's overall mark.
  m = /^note_(\d)$/.exec(field);
  if (m) return `${t('field_mark')} ${m[1]}`;
  m = /^(zeit|hindernis|andere)_(\d)$/.exec(field);
  if (m) {
    const name = seconds && m[1] !== 'zeit' ? `${t(what[m[1]])} (s)` : t(what[m[1]]);
    return rounds > 1 ? `${t('round')} ${m[2]} · ${name}` : name;
  }
  return field in what ? t(what[field]) : null;
}

/** Seconds as minutes and seconds ("312" → "5:12"). */
const clock = (seconds: string) => {
  const n = Number(seconds);
  if (seconds === '' || !Number.isFinite(n)) return seconds;
  const whole = Math.floor(n);
  const rest = seconds.includes('.') ? seconds.slice(seconds.indexOf('.')) : '';
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}${rest}`;
};

/** A field: shows what is saved, keeps what is being typed, saves on Enter, Tab or leaving. */
function FieldInput({
  value,
  disabled,
  onSave,
  onMove,
  register,
  label,
}: {
  value: string;
  disabled: boolean;
  onSave: (v: string) => void;
  onMove: (step: number) => void;
  register: (el: HTMLInputElement | null) => void;
  label?: string;
}) {
  const [text, setText] = useState(value);
  const focused = useRef(false);
  useEffect(() => {
    if (!focused.current) setText(value);
  }, [value]);
  const save = () => {
    if (valid(text) && text.trim() !== value) onSave(text.trim());
  };
  return (
    <input
      ref={register}
      value={text}
      disabled={disabled}
      aria-label={label}
      inputMode="decimal"
      enterKeyHint="next"
      onFocus={e => {
        focused.current = true;
        e.target.select();
      }}
      onBlur={() => {
        focused.current = false;
        save();
      }}
      onChange={e => setText(e.target.value)}
      onKeyDown={e => {
        if (e.key === 'Enter' || e.key === 'ArrowDown' || e.key === 'ArrowUp') {
          e.preventDefault();
          save();
          onMove(e.key === 'ArrowUp' || e.shiftKey ? -1 : 1);
        } else if (e.key === 'Escape') {
          setText(value);
        }
      }}
      className={`h-8 w-full rounded-md border bg-card px-2 text-right text-[13px] font-medium tabular shadow-xs outline-none transition-[border-color,box-shadow] focus:border-info focus:ring-3 focus:ring-info/20 pointer-coarse:h-11 ${valid(text) ? 'border-line-strong' : 'border-danger bg-danger/8'} ${disabled ? 'opacity-40' : ''}`}
    />
  );
}

/** A field's round ("1", "2", "J1"…) and kind. */
function parse(field: string): { round: string; kind: 'penalties' | 'time' } {
  const m = /^(strafpunkte|zeit)_(initial|stechen)_(\d)$/.exec(field);
  if (!m) return { round: field, kind: 'penalties' };
  return { round: m[2] === 'initial' ? m[3] : `J${m[3]}`, kind: m[1] === 'zeit' ? 'time' : 'penalties' };
}

export default function JumpingEntry({ show, classId }: { show: string; classId: string }) {
  const t = useT();
  const { version, change } = useChanges();
  const view = useLoad(() => call<ResultsView>('results', { show, class: classId }), [show, classId, version]);
  // The class's scoring table, where the results are entered (as dressage's test): changed only
  // before results.
  const details = useLoad(
    () => call<{ scoring_table: number | null; scoring_tables: ScoringTableChoice[]; has_results: boolean }>('class_details', { show, class: classId }),
    [show, classId, version],
  ).data;
  const narrow = useNarrow();
  const [adopt, setAdopt] = useState(false);
  const [imported, setImported] = useState<{ results: number; notes: string[] } | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [open, setOpen] = useState<number | null>(null);
  const inputs = useRef(new Map<string, HTMLInputElement>());

  if (view.error) return <ErrorNote error={view.error} />;
  const v = view.data;
  if (!v) return null;
  const locked = v.class.status === 'final';

  const rounds = [...new Set(v.fields.map(f => parse(f).round))];
  const roundLabel = (round: string) => {
    if (round.startsWith('J')) return v.fields.some(f => f.endsWith('stechen_2')) ? `${t('jump_off')} ${round.slice(1)}` : t('jump_off');
    return `${v.two_phase ? t('phase') : t('round')} ${round}`;
  };
  // Driving phases: each field has its own label (no rounds of jumping).
  const drivingFields = v.fields.some(f => drivingLabel(f, t, 1) !== null);
  const fieldLabel = (f: string) => {
    const driving = drivingLabel(f, t, v.fields.filter(x => /^zeit_\d$/.test(x)).length, v.faults_in_seconds);
    if (driving) return driving;
    const { round, kind } = parse(f);
    const what = kind === 'time' ? t('time_short') : v.points ? t('field_points') : t('pen_short');
    return rounds.length > 1 ? `${roundLabel(round)} · ${what}` : what;
  };
  // The usual reasons in the office's language; others as Swiss Equestrian names them.
  const reasonLabel = useReason();
  const statusOf = (l: ResultLine) => (l.out === null ? '' : String(l.out));

  const save = (l: ResultLine, field: string, text: string) => change('edit_result', { class: classId, entry: l.entry, values: { [field]: text === '' ? null : text } });
  const setOut = (l: ResultLine, code: string) => change('edit_result', { class: classId, entry: l.entry, out: code === '' ? null : Number(code) });

  const rows = v.lines;
  const focus = (row: number, field: string) => inputs.current.get(`${rows[row]?.entry}:${field}`)?.focus();
  // Enter: the same field of the next rider who starts; on a phone, the next field of the open form.
  const move = (row: number, field: string, step: number) => {
    if (narrow) {
      const i = v.fields.indexOf(field) + step;
      if (i >= 0 && i < v.fields.length) return focus(row, v.fields[i]);
      if (step > 0) {
        const next = rows.findIndex((l, j) => j > row && !l.not_starting);
        if (next >= 0) {
          setOpen(rows[next].entry);
          setTimeout(() => focus(next, v.fields[0]), 50);
        }
      }
      return;
    }
    for (let j = row + step; j >= 0 && j < rows.length; j += step) {
      if (!rows[j].not_starting) return focus(j, field);
    }
  };

  // The quick jump: the rider's first field (on a phone, their result opened).
  const jump = (n: number) => {
    const l = findStarter(rows, n);
    if (!l) return false;
    const i = rows.indexOf(l);
    if (narrow) setOpen(l.entry);
    setTimeout(() => {
      focus(i, v.fields[0]);
      document.getElementById(`result-${l.entry}`)?.scrollIntoView({ block: 'center' });
    }, 50);
    return true;
  };

  const exportList = async () => {
    try {
      const where = await saveFile(`Prüfung_${v.class.number}.xlsx`, 'xlsx', path => call('timekeeper_file', { show, class: classId, path }));
      if (where) setSaved(where);
    } catch (e) {
      setSaved(null);
      setImported({ results: 0, notes: [String((e as Error).message ?? e)] });
    }
  };
  const importFile = async () => {
    const path = await chooseFile(['xlsx', 'xls', 'txt', 'csv'], t('timekeeper_file'));
    if (!path) return;
    const answer = await change('import_results', { class: classId, path, adopt_ranks: adopt });
    if (answer) setImported({ results: Number(answer.results ?? 0), notes: (answer.notes as string[]) ?? [] });
  };

  const rank = (l: ResultLine) => (
    <span className={`tabular ${l.placed ? 'font-bold text-accent' : 'font-semibold'}`} title={l.manual_rank ? 'set by hand' : undefined}>
      {l.rank ?? (l.has_result && (l.out !== null || l.over_maximum) ? '–' : '')}
      {l.manual_rank && '*'}
    </span>
  );
  const status = (l: ResultLine, big = false) => (
    <select
      value={statusOf(l)}
      disabled={locked || l.not_starting}
      onChange={e => setOut(l, e.target.value)}
      className={`field ${big ? '' : 'field-sm'} ${l.out !== null ? 'font-semibold text-warn' : ''}`}
    >
      <option value="">{l.has_result ? t('in_class') : '—'}</option>
      {v.reasons.map(r => (
        <option key={r.code} value={r.code}>
          {reasonLabel(r.name)}
        </option>
      ))}
    </select>
  );
  // Cones: over the time limit, eliminated whatever the office set.
  const overMaximum = (l: ResultLine) => l.over_maximum && <span className="text-xs font-semibold text-warn">{t('over_maximum')}</span>;
  // Cross-country: times in minutes and seconds.
  const cross = v.class.discipline === 'CROSS';
  const input = (l: ResultLine, i: number, f: string) => (
    <FieldInput
      value={/_(start|finish)$/.test(f) ? timeOfDay(l.values[f] ?? '') : cross && f.startsWith('zeit') ? clock(l.values[f] ?? '') : (l.values[f] ?? '')}
      disabled={locked || l.not_starting}
      label={fieldLabel(f)}
      onSave={text => save(l, f, text)}
      onMove={step => move(i, f, step)}
      register={el => {
        const key = `${l.entry}:${f}`;
        if (el) inputs.current.set(key, el);
        else inputs.current.delete(key);
      }}
    />
  );

  const tablePicker = details && details.scoring_tables.length > 0 && (
    <ScoringTablePicker
      value={details.scoring_table}
      tables={details.scoring_tables}
      onChange={table => change('edit_class', { class: classId, scoring_table: table })}
      disabled={locked || details.has_results}
    />
  );
  const toolbar = (
    <div className="flex flex-wrap items-center gap-2 border-b border-line bg-card px-3 py-2.5 sm:px-6">
      {tablePicker && (
        <div className="mr-1 flex min-w-0 items-center gap-2 phone:mr-0 phone:w-full">
          <span className="text-xs font-semibold text-ink-2">{t('scoring_label')}</span>
          {details?.has_results && !locked ? (
            <Tooltip label={t('pub_scoring_locked')} side="bottom">
              <div className="w-[min(60vw,22rem)] phone:w-auto phone:min-w-0 phone:flex-1">{tablePicker}</div>
            </Tooltip>
          ) : (
            <div className="w-[min(60vw,22rem)] phone:w-auto phone:min-w-0 phone:flex-1">{tablePicker}</div>
          )}
        </div>
      )}
      {/* The timekeeper's files: on a phone in one menu (rarely there), the results first. */}
      <div className="contents phone:hidden">
        <Button size="sm" icon={Download} onClick={exportList}>
          {t('timekeeper_out')}
        </Button>
        <Button size="sm" variant="primary" icon={Upload} disabled={locked} onClick={importFile}>
          {t('timekeeper_in')}
        </Button>
        <label className="ml-1 flex cursor-pointer items-center gap-2 text-xs font-semibold text-ink-2">
          <input type="checkbox" className="check" checked={adopt} onChange={e => setAdopt(e.target.checked)} />
          {t('adopt_ranks')}
        </label>
      </div>
      <div className="hidden phone:block">
        <Menu
          width="w-72"
          align="start"
          trigger={
            <Button size="sm" icon={Timer}>
              {t('timekeeper_menu')}
            </Button>
          }
        >
          <MenuItem icon={Upload} disabled={locked} onSelect={importFile}>
            {t('timekeeper_in')}
          </MenuItem>
          <MenuItem icon={Download} onSelect={exportList}>
            {t('timekeeper_out')}
          </MenuItem>
          <MenuSeparator />
          <MenuCheck checked={adopt} onChange={setAdopt}>
            {t('adopt_ranks')}
          </MenuCheck>
        </Menu>
      </div>
      {v.qualifier && (
        <Pill tone="info">
          {t('qualifier')}
          {t('colon')}
          {v.qualifier}
        </Pill>
      )}
      <span className="ml-auto hidden text-xs text-ink-4 lg:inline">{t('jumping_keys')}</span>
      <QuickJump onJump={jump} />
    </div>
  );

  const notes = (
    <>
      {saved && (
        <div className="flex items-center gap-2 border-b border-line bg-ok/10 px-4 py-2 text-xs">
          <span className="min-w-0 flex-1 truncate">
            {t('saved_to')}
            {t('colon')}
            {saved}
          </span>
          <IconButton icon={X} label={t('close')} size="sm" onClick={() => setSaved(null)} />
        </div>
      )}
      {imported && (
        <div className={`border-b border-line px-4 py-2 text-xs ${imported.notes.length ? 'bg-warn/10' : 'bg-ok/10'}`}>
          <div className="flex items-center gap-2">
            <span className="flex-1 font-semibold">
              {imported.results} {t('results_read')}
              {imported.notes.length > 0 && ` · ${t('to_check')}${t('colon').trimEnd()}`}
            </span>
            <IconButton icon={X} label={t('close')} size="sm" onClick={() => setImported(null)} />
          </div>
          {imported.notes.length > 0 && (
            <ul className="mt-1 list-disc space-y-0.5 pl-5">
              {imported.notes.map((n, i) => (
                <li key={i}>{n}</li>
              ))}
            </ul>
          )}
        </div>
      )}
      {v.faults_in_seconds && <div className="border-b border-line bg-info/6 px-4 py-2 text-xs text-ink-2">{t('cones_b_hint')}</div>}
      {v.ranking_error && v.lines.some(l => l.has_result) && (
        <div className="border-b border-line bg-warn/10 px-4 py-2 text-xs">
          {t('no_ranking')}
          {t('colon')}
          {v.ranking_error}
        </div>
      )}
      {v.fields.length === 0 && <p className="p-4 text-sm text-ink-2">{t('no_results_fields')}</p>}
    </>
  );

  const who = (l: ResultLine) => (
    <>
      <div className={`truncate font-medium ${l.not_starting ? 'text-ink-3 line-through' : ''}`}>
        {l.rider}
        {l.hors_concours && <span className="ml-1 text-xs font-semibold text-warn">HC</span>}
      </div>
      <div className="truncate text-xs text-ink-3">{l.horse}</div>
    </>
  );

  // A phone: the riders as a list; one rider's result opens under its row.
  if (narrow)
    return (
      <div className="flex h-full flex-col phone:h-auto">
        {toolbar}
        <div className="min-h-0 flex-1 overflow-auto phone:overflow-visible">
          {notes}
          <ul className="text-sm">
            {rows.map((l, i) => (
              <li key={l.entry} id={`result-${l.entry}`} className={`border-b border-line ${open === l.entry ? 'bg-hover' : ''}`}>
                <button onClick={() => setOpen(open === l.entry ? null : l.entry)} className="flex w-full items-center gap-3 px-3 py-2 text-left">
                  <span className="w-7 text-right text-xs text-ink-3 tabular">{l.start_number ?? l.position}</span>
                  <span className="min-w-0 flex-1">{who(l)}</span>
                  {l.carried !== null && (
                    <span className="text-xs text-ink-3 tabular" title={t('carried_hint')}>
                      ({l.carried})
                    </span>
                  )}
                  <span className="text-right text-xs tabular">
                    {l.out !== null ? (
                      <span className="font-semibold text-warn">{reasonLabel(v.reasons.find(r => r.code === l.out)?.name ?? '')}</span>
                    ) : l.over_maximum ? (
                      <span className="font-semibold text-warn">{t('over_maximum')}</span>
                    ) : l.has_result ? (
                      `${l.faults ?? ''} · ${l.time === null ? '' : cross ? clock(l.time) : Number(l.time).toFixed(2)}`
                    ) : (
                      ''
                    )}
                  </span>
                  <span className="w-6 text-right">{rank(l)}</span>
                </button>
                {open === l.entry && (
                  <div className="grid grid-cols-2 gap-x-3 gap-y-2 px-3 pb-3">
                    {v.fields.map(f => (
                      <label key={f} className="text-xs text-ink-3">
                        {fieldLabel(f)}
                        {input(l, i, f)}
                      </label>
                    ))}
                    <label className="col-span-2 text-xs text-ink-3">
                      {t('status_col')}
                      {status(l, true)}
                      {overMaximum(l)}
                    </label>
                  </div>
                )}
              </li>
            ))}
          </ul>
        </div>
      </div>
    );

  const th = 'border-b border-line bg-raised px-2 py-2.5 text-xs font-semibold text-ink-3';
  const td = 'border-b border-line px-2 py-1.5 align-middle';
  const hasCarried = v.qualifier !== null;
  return (
    <div className="flex h-full flex-col">
      {toolbar}
      <div className="min-h-0 flex-1 overflow-auto">
        {notes}
        {v.fields.length > 0 && (
          <table className="w-full table-fixed text-sm" style={{ minWidth: `${Math.max(44, 38 + v.fields.length * 5.5)}rem` }}>
            <thead className="sticky top-0 z-10 bg-raised">
              {rounds.length > 1 && !drivingFields && (
                <tr>
                  <th colSpan={hasCarried ? 4 : 3} />
                  {rounds.map(r => (
                    <th key={r} colSpan={v.fields.filter(f => parse(f).round === r).length} className="border-l border-line bg-raised px-2 pt-2 text-center text-xs font-bold text-ink-2">
                      {roundLabel(r)}
                    </th>
                  ))}
                  <th colSpan={4} />
                </tr>
              )}
              <tr>
                <th className={`${th} w-10 text-right`}>#</th>
                <th className={`${th} w-12 text-right`}>{t('start_no')}</th>
                <th className={`${th} text-left`}>{t('rider')}</th>
                {hasCarried && (
                  <th className={`${th} w-16 text-right`} title={t('carried_hint')}>
                    {t('carried')}
                  </th>
                )}
                {v.fields.map(f => (
                  <th key={f} className={`${th} ${/_(start|finish)$/.test(f) ? 'w-24' : 'w-20'} border-l text-right`}>
                    {drivingFields ? fieldLabel(f) : parse(f).kind === 'time' ? t('time_short') : v.points ? t('field_points') : t('pen_short')}
                  </th>
                ))}
                <th className={`${th} w-32 border-l text-left`}>{t('status_col')}</th>
                <th className={`${th} w-14 border-l text-right`}>{t('rank')}</th>
                <th className={`${th} w-16 text-right`}>{v.points ? t('field_points') : t('pen_short')}</th>
                <th className={`${th} w-20 text-right`}>{t('time_short')}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((l, i) => (
                <tr key={l.entry} id={`result-${l.entry}`} className="hover:bg-hover">
                  <td className={`${td} text-right text-ink-3 tabular`}>{l.position}</td>
                  <td className={`${td} text-right font-semibold tabular`}>{l.start_number ?? ''}</td>
                  <td className={`${td} max-w-0`}>{who(l)}</td>
                  {hasCarried && <td className={`${td} text-right text-ink-2 tabular`}>{l.carried ?? ''}</td>}
                  {v.fields.map(f => (
                    <td key={f} className={`${td} border-l`}>
                      {input(l, i, f)}
                    </td>
                  ))}
                  <td className={`${td} border-l`}>
                    {status(l)}
                    {overMaximum(l)}
                  </td>
                  <td className={`${td} border-l text-right`}>{rank(l)}</td>
                  <td className={`${td} text-right font-semibold tabular`}>{l.faults ?? ''}</td>
                  <td className={`${td} text-right tabular`}>{l.time === null ? '' : cross ? clock(l.time) : Number(l.time).toFixed(2)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
