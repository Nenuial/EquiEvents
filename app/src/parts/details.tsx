// A class's details (Rosson's class info box): how many are placed, the sponsor on its prize
// labels, extra lines printed on its lists and sent to Swiss Equestrian, its
// logo; its published class (Rosson's info box of it: what
// Swiss Equestrian published, the entries by status, the category as printed, the levels).
// Edited in place under the class's head; applied as one change (undo brings them back).

import { Fragment, useState } from 'react';
import { CalendarDays, Plus, X } from 'lucide-react';
import { call, type PhaseKind } from '../api';
import { formatDate, useLanguage, useT, type Key } from '../i18n';
import { useChanges } from './changes';
import { useLoad } from './common';
import { Button, IconButton } from '../ui';
import { Grip, SortableList, SortRow } from './sortable';

interface ClassDetails {
  number: string;
  date: string;
  sponsor: string;
  placed_count: number | null;
  max_starters: number | null;
  extra_fields: { key: string; value: string }[];
  logo: string | null;
  days: string[];
  /** The show's own days (the others: days classes were moved to). */
  planned_days: string[];
  /** The class is on the ideal-time table: the office sets the ideal time (S20). */
  ideal_table: boolean;
  ideal_time: string | null;
  scoring_table: number | null;
  scoring_tables: { id: number; code: string; name: string }[];
  has_results: boolean;
  /** Dressage: the judges may give a total only (before any sheet). */
  dressage: boolean;
  totals_per_judge: boolean;
  /** Team dressage: five judges 3/2 (before any sheet). */
  team_dressage: boolean;
  team_3_2: boolean;
  team_jumping: boolean;
  team_count_all: boolean;
  youth_programme: boolean;
  youth_final: boolean;
  /** Eventing: this class's phase, or (`phases`) the class made of phases. */
  phase: PhaseKind | null;
  phases: boolean;
  ideal_time_penalties: boolean;
  max_time: string | null;
  max_time_penalties: boolean;
  rank_points: boolean;
  /** Eventing: the rules decide penalties or rank points (no choice). */
  eventing: boolean;
  /** A driving phase's settings (seconds for times), the class's licences and turnouts. */
  driving: Driving | null;
  driving_licences: string[] | null;
  /** Driving: a phase of a full event (Vollprüfung): its cones are Wertung A, the overtime to the hundredth. */
  driving_full_event: boolean;
  driving_turnouts: string[] | null;
  published: Published | null;
}

interface Published {
  number: string;
  discipline: string;
  category: string;
  category_text: string;
  levels: number;
  has_levels: boolean;
  description: string;
  fee: string | null;
  scoring_text: string | null;
  counts: { total: number; hors_concours: number; not_starting: number; withdrawn: number; wrong: number; waiting_list: number; organiser_places: number };
}

/** A class's day: one of the show's days (or one a class was moved to), or any other day. */
export function DayField({ value, days, planned, disabled, onChange }: { value: string; days: string[]; planned: string[]; disabled?: boolean; onChange: (day: string) => void }) {
  const t = useT();
  const language = useLanguage();
  const known = days.includes(value) ? days : [...days, value].sort();
  const [other, setOther] = useState(false);
  const outside = planned.length > 0 && value !== '' && !planned.includes(value);
  return (
    <div className="grid gap-1">
      {other ? (
        <div className="flex gap-1">
          <input type="date" value={value} disabled={disabled} onChange={e => e.target.value && onChange(e.target.value)} className="field min-w-0 flex-1" />
          <IconButton icon={CalendarDays} label={t('show_days')} onClick={() => setOther(false)} />
        </div>
      ) : (
        <select value={value} disabled={disabled} onChange={e => (e.target.value === '' ? setOther(true) : onChange(e.target.value))} className="field">
          {known.map(day => (
            <option key={day} value={day}>
              {formatDate(day, language)}
            </option>
          ))}
          <option value="">{t('other_day')}</option>
        </select>
      )}
      {outside && <span className="text-xs text-warn">{t('day_not_planned')}</span>}
    </div>
  );
}

/** The lines printed on the lists, each with its own id while edited (they move by drag and drop). */
type Line = { id: string; key: string; value: string };
let lineIds = 0;
const line = (key = '', value = ''): Line => ({ id: `line-${++lineIds}`, key, value });

/** An image file made a PNG of at most 400 px a side, as Rosson keeps a class's logo. */
async function logoOf(file: File): Promise<string> {
  const url = URL.createObjectURL(file);
  try {
    const img = new Image();
    img.src = url;
    await img.decode();
    const scale = Math.min(1, 400 / Math.max(img.width, img.height));
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(1, Math.round(img.width * scale));
    canvas.height = Math.max(1, Math.round(img.height * scale));
    canvas.getContext('2d')?.drawImage(img, 0, 0, canvas.width, canvas.height);
    return canvas.toDataURL('image/png').replace('image/png', 'image/octet-stream');
  } finally {
    URL.revokeObjectURL(url);
  }
}

export function DetailsEditor({ show, classId, done }: { show: string; classId: string; done?: () => void }) {
  const t = useT();
  const { change } = useChanges();
  const loaded = useLoad(() => call<ClassDetails>('class_details', { show, class: classId }), [show, classId]);
  if (!loaded.data) return null;
  return <Form key={classId} d={loaded.data} classId={classId} done={done} change={change} t={t} />;
}

function Form({ d, classId, done, change, t }: { d: ClassDetails; classId: string; done?: () => void; change: ReturnType<typeof useChanges>['change']; t: ReturnType<typeof useT> }) {
  const [placed, setPlaced] = useState(d.placed_count === null ? '' : String(d.placed_count));
  const [max, setMax] = useState(d.max_starters === null ? '' : String(d.max_starters));
  const [sponsor, setSponsor] = useState(d.sponsor);
  const [fields, setFields] = useState<Line[]>(() => (d.extra_fields.length ? d.extra_fields.map(f => line(f.key, f.value)) : [line()]));
  const moveLine = (id: string, to: number) =>
    setFields(fs => {
      const next = [...fs];
      const from = next.findIndex(f => f.id === id);
      const [moved] = next.splice(from, 1);
      next.splice(to, 0, moved);
      return next;
    });
  const editLine = (id: string, edit: Partial<Line>) => setFields(fs => fs.map(f => (f.id === id ? { ...f, ...edit } : f)));
  const [logo, setLogo] = useState(d.logo);
  const [logoError, setLogoError] = useState(false);
  const [ideal, setIdeal] = useState(d.ideal_time ?? '');
  const [categoryText, setCategoryText] = useState(d.published?.category_text ?? '');
  const [levels, setLevels] = useState(String(Math.max(1, d.published?.levels ?? 1)));
  const [totals, setTotals] = useState(d.totals_per_judge);
  const [threeTwo, setThreeTwo] = useState(d.team_3_2);
  const [countAll, setCountAll] = useState(d.team_count_all);
  const [youthFinal, setYouthFinal] = useState(d.youth_final);
  const [idealPenalties, setIdealPenalties] = useState(d.ideal_time_penalties);
  const [maxTime, setMaxTime] = useState(d.max_time ?? '');
  const [maxPenalties, setMaxPenalties] = useState(d.max_time_penalties);
  const [rankPoints, setRankPoints] = useState(d.rank_points);
  const [driving, setDriving] = useState<Driving>(d.driving ?? {});

  const placedOk = placed.trim() === '' || (Number.isInteger(Number(placed)) && Number(placed) > 0);
  const maxOk = max.trim() === '' || (Number.isInteger(Number(max)) && Number(max) > 0);
  // Seconds (75, 74.5) or minutes and seconds (1:15).
  const idealOk = ideal.trim() === '' || /^\s*(\d+:[0-5]?\d([.,]\d+)?|\d+([.,]\d+)?)\s*$/.test(ideal);
  const maxTimeOk = maxTime.trim() === '' || /^\s*(\d+:[0-5]?\d([.,]\d+)?|\d+([.,]\d+)?)\s*$/.test(maxTime);
  const levelsOk = /^[1-9]$/.test(levels.trim());
  const ready = placedOk && maxOk && maxTimeOk && idealOk && (!d.published?.has_levels || levelsOk);
  const input = 'field field-sm';
  const label = 'w-full text-xs font-semibold text-ink-2 sm:w-48';
  const row = 'flex flex-wrap items-center gap-x-3 gap-y-1.5';

  const apply = async () => {
    const answer = await change('set_class_details', {
      class: classId,
      placed_count: placed.trim() === '' ? null : Number(placed),
      max_starters: max.trim() === '' ? null : Number(max),
      ...(d.ideal_table ? { ideal_time: ideal.trim() === '' ? null : ideal.trim() } : {}),
      sponsor,
      extra_fields: fields.map(({ key, value }) => ({ key, value })),
      logo,
      ...(d.dressage && totals !== d.totals_per_judge ? { totals_per_judge: totals } : {}),
      ...(d.team_dressage && threeTwo !== d.team_3_2 ? { team_3_2: threeTwo } : {}),
      ...(d.team_jumping && countAll !== d.team_count_all ? { team_count_all: countAll } : {}),
      ...((d.youth_programme || d.youth_final) && youthFinal !== d.youth_final ? { youth_final: youthFinal } : {}),
      ...(d.phase === 'cross' ? { ideal_time_penalties: idealPenalties } : {}),
      ...(d.phase === 'jumping' ? { max_time: maxTime.trim() === '' ? null : maxTime.trim(), max_time_penalties: maxPenalties } : {}),
      ...(d.phases && !d.eventing ? { rank_points: rankPoints } : {}),
      ...(d.driving ? { driving } : {}),
      ...(d.published ? { category_text: categoryText, ...(d.published.has_levels ? { levels: Number(levels) } : {}) } : {}),
    });
    if (answer) done?.();
  };

  return (
    <div className="space-y-4 p-4 text-[13px] sm:p-6">
      <div className={row}>
        <span className={label}>{t('placed_count')}</span>
        <input value={placed} onChange={e => setPlaced(e.target.value)} inputMode="numeric" placeholder="30 %" className={`${input} w-20 text-right ${placedOk ? '' : 'border-accent'}`} />
        <span className="text-xs text-ink-3">{t('placed_count_hint')}</span>
      </div>
      <div className={row}>
        <span className={label}>{t('max_starters')}</span>
        <input value={max} onChange={e => setMax(e.target.value)} inputMode="numeric" placeholder="–" className={`${input} w-20 text-right ${maxOk ? '' : 'border-accent'}`} />
        <span className="text-xs text-ink-3">{t('max_starters_hint')}</span>
      </div>
      {d.ideal_table && (
        <div className={row}>
          <span className={label}>{t('ideal_time')}</span>
          <input value={ideal} onChange={e => setIdeal(e.target.value)} placeholder="1:15" className={`${input} w-24 text-right ${idealOk && ideal.trim() !== '' ? '' : 'border-accent'}`} />
          <span className="text-xs text-ink-3">{t('ideal_time_hint')}</span>
        </div>
      )}
      {d.phase === 'cross' && (
        <div className={row}>
          <span className={label}>{t('ideal_time_penalties')}</span>
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={idealPenalties} onChange={e => setIdealPenalties(e.target.checked)} />
            <span className="text-xs text-ink-3">{t('ideal_time_penalties_hint')}</span>
          </label>
        </div>
      )}
      {d.phase === 'jumping' && (
        <div className={row}>
          <span className={label}>{t('max_time')}</span>
          <input value={maxTime} onChange={e => setMaxTime(e.target.value)} placeholder="1:20" className={`${input} w-24 text-right ${maxTimeOk ? '' : 'border-accent'}`} />
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={maxPenalties} onChange={e => setMaxPenalties(e.target.checked)} />
            <span className="text-xs text-ink-3">{t('max_time_penalties_hint')}</span>
          </label>
        </div>
      )}
      {d.driving && d.phase && (
        <DrivingFields kind={d.phase} value={driving} set={setDriving} licences={d.driving_licences ?? []} turnouts={d.driving_turnouts ?? []} fullEvent={d.driving_full_event} />
      )}
      {d.phases && !d.eventing && (
        <div className={row}>
          <span className={label}>{t('rank_points')}</span>
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={rankPoints} onChange={e => setRankPoints(e.target.checked)} />
            <span className="text-xs text-ink-3">{t('rank_points_hint')}</span>
          </label>
        </div>
      )}
      {d.team_dressage && (
        <div className={row}>
          <span className={label}>{t('team_3_2')}</span>
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={threeTwo} disabled={d.has_results} onChange={e => setThreeTwo(e.target.checked)} />
            <span className="text-xs text-ink-3">{d.has_results ? t('totals_per_judge_locked') : t('team_3_2_hint')}</span>
          </label>
        </div>
      )}
      {d.team_jumping && (
        <div className={row}>
          <span className={label}>{t('team_count_all')}</span>
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={countAll} onChange={e => setCountAll(e.target.checked)} />
            <span className="text-xs text-ink-3">{t('team_count_all_hint')}</span>
          </label>
        </div>
      )}
      {(d.youth_programme || d.youth_final) && (
        <div className={row}>
          <span className={label}>{t('youth_final')}</span>
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={youthFinal} onChange={e => setYouthFinal(e.target.checked)} />
            <span className="text-xs text-ink-3">{t('youth_final_hint')}</span>
          </label>
        </div>
      )}
      {d.dressage && !d.team_dressage && (
        <div className={row}>
          <span className={label}>{t('totals_per_judge')}</span>
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={totals} disabled={d.has_results} onChange={e => setTotals(e.target.checked)} />
            <span className="text-xs text-ink-3">{d.has_results ? t('totals_per_judge_locked') : t('totals_per_judge_hint')}</span>
          </label>
        </div>
      )}
      <div className={row}>
        <span className={label}>{t('sponsor_labels')}</span>
        <input value={sponsor} onChange={e => setSponsor(e.target.value)} className={`${input} flex-1 sm:max-w-md`} />
      </div>
      <div className="space-y-2">
        <div className="text-xs font-semibold text-ink-2">{t('extra_lines')}</div>
        {/* In the order printed: moved by their grip (mouse, finger or keyboard). */}
        {fields.length > 0 && (
          <SortableList ids={fields.map(f => f.id)} onMove={moveLine}>
            <div className="rounded-xl border border-line bg-card sm:max-w-2xl">
              {fields.map(f => (
                <SortRow key={f.id} id={f.id}>
                  {r => (
                    <div
                      ref={r.ref}
                      style={r.style}
                      className={`flex items-center gap-1.5 border-t border-line px-1.5 py-1.5 first:rounded-t-xl first:border-t-0 last:rounded-b-xl ${r.dragging ? 'rounded-xl bg-raised shadow-lift' : 'bg-card'}`}
                    >
                      <span className="flex w-7 shrink-0 justify-center pointer-coarse:w-9">
                        <Grip grip={r.grip} label={t('drag_to_move')} disabled={fields.length < 2} />
                      </span>
                      <input value={f.key} placeholder={t('extra_key')} onChange={e => editLine(f.id, { key: e.target.value })} className={`${input} w-28 shrink-0 sm:w-44`} />
                      <input value={f.value} placeholder={t('extra_value')} onChange={e => editLine(f.id, { value: e.target.value })} className={`${input} min-w-0 flex-1`} />
                      <IconButton icon={X} label={t('remove')} size="sm" onClick={() => setFields(fs => fs.filter(x => x.id !== f.id))} />
                    </div>
                  )}
                </SortRow>
              ))}
            </div>
          </SortableList>
        )}
        <Button size="sm" variant="secondary" icon={Plus} onClick={() => setFields(fs => [...fs, line()])}>
          {t('add_line')}
        </Button>
      </div>
      <div className={row}>
        <span className={label}>{t('logo')}</span>
        {logo && <img src={logo.replace('image/octet-stream', 'image/png')} alt="" className="h-12 max-w-[10rem] rounded border border-line bg-white object-contain p-0.5" />}
        <label className="cursor-pointer btn btn-sm btn-secondary pointer-coarse:py-2">
          {t('choose_image')}
          <input
            type="file"
            accept="image/png,image/jpeg"
            className="hidden"
            onChange={async e => {
              const file = e.target.files?.[0];
              if (!file) return;
              try {
                setLogo(await logoOf(file));
                setLogoError(false);
              } catch {
                setLogoError(true);
              }
            }}
          />
        </label>
        {logo && (
          <button onClick={() => setLogo(null)} className="rounded px-2 py-1 text-xs text-ink-3 hover:bg-hover">
            {t('remove')}
          </button>
        )}
        {logoError && <span className="text-xs text-accent">{t('not_an_image')}</span>}
      </div>
      {d.published && (
        <div className="space-y-3 rounded-xl border border-line bg-raised p-4">
          <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
            {t('pub_title')} {d.published.number} · <span className="normal-case">{t('pub_shared')}</span>
          </div>
          <div className="text-xs text-ink-2">
            {d.published.discipline} · {d.published.category}
            {d.published.scoring_text && ` · ${d.published.scoring_text}`}
            {d.published.fee && ` · ${t('pub_fee')} ${d.published.fee} CHF`}
          </div>
          {d.published.description && <p className="max-h-24 overflow-auto whitespace-pre-line text-xs text-ink-3">{d.published.description}</p>}
          <div className="text-xs text-ink-2">
            {t('pub_counts')} : <b>{d.published.counts.total}</b>
            {(
              [
                [d.published.counts.hors_concours, 'HC'],
                [d.published.counts.not_starting, t('pub_not_starting')],
                [d.published.counts.withdrawn, t('pub_withdrawn')],
                [d.published.counts.wrong, t('pub_wrong')],
                [d.published.counts.waiting_list, t('pub_waiting')],
                [d.published.counts.organiser_places, t('pub_organiser')],
              ] as [number, string][]
            )
              .filter(([n]) => n > 0)
              .map(([n, what]) => ` · ${n} ${what}`)
              .join('')}
          </div>
          <div className={row}>
            <span className={label}>{t('pub_category_text')}</span>
            <input value={categoryText} placeholder={d.published.category} onChange={e => setCategoryText(e.target.value)} className={`${input} flex-1 sm:max-w-md`} />
          </div>
          {d.published.has_levels && (
            <div className={row}>
              <span className={label}>{t('pub_levels')}</span>
              <input value={levels} onChange={e => setLevels(e.target.value)} inputMode="numeric" className={`${input} w-14 text-right ${levelsOk ? '' : 'border-accent'}`} />
              <span className="text-xs text-ink-3">{t('pub_levels_hint')}</span>
            </div>
          )}
        </div>
      )}
      <div className="flex justify-end gap-2 border-t border-line pt-4">
        {done && (
          <button onClick={done} className="btn btn-md btn-ghost">
            {t('cancel')}
          </button>
        )}
        <button disabled={!ready} onClick={apply} className="btn btn-md btn-primary">
          {t('apply')}
        </button>
      </div>
    </div>
  );
}

/** A driving phase's settings as kept (times in seconds, as text). */
interface Driving {
  a_allowed?: string | null;
  a_minimum?: string | null;
  b_allowed?: string | null;
  b_minimum?: string | null;
  a_maximum?: string | null;
  b_maximum?: string | null;
  transfer?: string | null;
  factor?: string | null;
  obstacles?: number | null;
  cones?: 'a' | 'b' | 'junior_cup' | null;
  rounds?: number | null;
  allowed?: Record<string, string>;
  maximum?: Record<string, string>;
  international?: boolean;
  bonus?: Record<string, string>;
  time_factor?: string | null;
  open?: 'time' | 'points' | 'penalty_points' | 'points_then_time' | null;
  multitest?: string | null;
}

/** Seconds shown as m:ss; m:ss (or seconds) read as seconds. */
const minutes = (s: string | null | undefined) => {
  if (s === null || s === undefined || s === '') return '';
  const n = Number(s);
  return Number.isFinite(n) ? `${Math.floor(n / 60)}:${String(Math.round(n % 60)).padStart(2, '0')}` : s;
};
const seconds = (text: string): string | null => {
  const m = /^\s*(\d+):([0-5]?\d(?:[.,]\d+)?)\s*$/.exec(text);
  if (m) return String(Number(m[1]) * 60 + Number(m[2].replace(',', '.')));
  const n = Number(text.trim().replace(',', '.'));
  return text.trim() === '' || !Number.isFinite(n) ? null : String(n);
};

/** A driving phase's settings: marathon times and factor, cones' scoring and allowed times per
 *  round and licence, derby's factor, open's ranking, multitest's programme. */
function DrivingFields({
  kind,
  value,
  set,
  licences,
  turnouts,
  fullEvent,
}: {
  kind: PhaseKind;
  value: Driving;
  set: (d: Driving) => void;
  licences: string[];
  turnouts: string[];
  /** A full event's cones: Wertung A, the overtime to the hundredth, by the rules (no choice). */
  fullEvent: boolean;
}) {
  const t = useT();
  const input = 'field field-sm';
  const label = 'w-full text-xs font-semibold text-ink-2 sm:w-48';
  const row = 'flex flex-wrap items-center gap-x-3 gap-y-1.5';
  const time = (key: 'a_allowed' | 'a_minimum' | 'b_allowed' | 'b_minimum' | 'a_maximum' | 'b_maximum', placeholder = '30:00') => (
    <div className={row}>
      <span className={label}>{t(`drv_${key}` as Key)}</span>
      <input defaultValue={minutes(value[key])} onBlur={e => set({ ...value, [key]: seconds(e.target.value) })} placeholder={placeholder} className={`${input} w-24 text-right`} />
    </div>
  );
  // The time limits, empty: A's allowed time + 20 %, B's twice (the rules).
  const limit = (allowed: string | null | undefined, factor: number) => (allowed ? minutes(String(Math.round(Number(allowed) * factor))) : '');
  const number = (key: 'factor' | 'time_factor' | 'transfer', placeholder: string) => (
    <div className={row}>
      <span className={label}>{t(`drv_${key}` as Key)}</span>
      <input
        defaultValue={value[key] ?? ''}
        onBlur={e => set({ ...value, [key]: e.target.value.trim().replace(',', '.') || null })}
        placeholder={placeholder}
        inputMode="decimal"
        className={`${input} w-20 text-right`}
      />
    </div>
  );
  if (kind === 'marathon')
    return (
      <>
        {time('a_allowed')}
        {time('a_minimum')}
        {time('a_maximum', limit(value.a_allowed, 1.2))}
        {time('b_allowed')}
        {time('b_minimum')}
        {time('b_maximum', limit(value.b_allowed, 2))}
        {number('transfer', '10')}
        {number('factor', '0.25')}
        <div className={row}>
          <span className={label}>{t('drv_obstacles')}</span>
          <input type="number" min={1} max={12} value={value.obstacles ?? 5} onChange={e => set({ ...value, obstacles: Number(e.target.value) })} className={`${input} w-20 text-right`} />
        </div>
      </>
    );
  if (kind === 'cones') {
    const rounds = value.rounds ?? 1;
    // A time per round and licence: the allowed time, and the time limit (empty: twice the
    // allowed time, as the rules say).
    const perLicence = (key: 'allowed' | 'maximum', placeholder: (k: string) => string) => (
      <div className={row}>
        <span className={label}>{t(`drv_${key}` as Key)}</span>
        <div className="grid gap-1" style={{ gridTemplateColumns: `auto repeat(${licences.length}, minmax(0, 5rem))` }}>
          <span />
          {licences.map(l => (
            <span key={l} className="text-center text-xs font-semibold text-ink-3">
              {l}
            </span>
          ))}
          {Array.from({ length: rounds }, (_, i) => i + 1).map(r => (
            <Fragment key={r}>
              <span className="pr-1 text-xs text-ink-3">
                {t('round')} {r}
              </span>
              {licences.map(l => (
                <input
                  key={l}
                  defaultValue={minutes(value[key]?.[`${r}:${l}`])}
                  onBlur={e => {
                    const times = { ...(value[key] ?? {}) };
                    const s = seconds(e.target.value);
                    if (s === null) delete times[`${r}:${l}`];
                    else times[`${r}:${l}`] = s;
                    set({ ...value, [key]: times });
                  }}
                  placeholder={placeholder(`${r}:${l}`)}
                  className={`${input} w-full text-right`}
                />
              ))}
            </Fragment>
          ))}
        </div>
      </div>
    );
    return (
      <>
        <div className={row}>
          <span className={label}>{t('drv_cones')}</span>
          <select value={fullEvent ? 'a' : (value.cones ?? 'a')} disabled={fullEvent} onChange={e => set({ ...value, cones: e.target.value as Driving['cones'] })} className={input}>
            <option value="a">{t('drv_cones_a')}</option>
            <option value="b">{t('drv_cones_b')}</option>
            <option value="junior_cup">{t('drv_cones_jc')}</option>
          </select>
          <span className="text-xs text-ink-3">{t('drv_rounds')}</span>
          <select value={rounds} onChange={e => set({ ...value, rounds: Number(e.target.value) })} className={input}>
            <option value={1}>1</option>
            <option value={2}>2</option>
          </select>
        </div>
        {perLicence('allowed', () => '2:00')}
        {perLicence('maximum', key => (value.allowed?.[key] ? minutes(String(Number(value.allowed[key]) * 2)) : '4:00'))}
        {turnouts.length > 0 && (
          <div className={row}>
            <span className={label}>{t('drv_bonus')}</span>
            {turnouts.map(g => (
              <label key={g} className="flex items-center gap-1 text-xs">
                {g}×
                <input
                  defaultValue={value.bonus?.[g] ?? ''}
                  onBlur={e => {
                    const bonus = { ...(value.bonus ?? {}) };
                    const s = seconds(e.target.value);
                    if (s === null) delete bonus[g];
                    else bonus[g] = s;
                    set({ ...value, bonus });
                  }}
                  inputMode="decimal"
                  className={`${input} w-16 text-right`}
                />
              </label>
            ))}
          </div>
        )}
        <label className="flex items-center gap-2 text-sm">
          <input type="checkbox" checked={fullEvent || (value.international ?? false)} disabled={fullEvent} onChange={e => set({ ...value, international: e.target.checked })} />
          <span className="text-xs text-ink-3">{t('drv_international')}</span>
        </label>
        {fullEvent && <p className="text-xs text-ink-3">{t('drv_full_event_cones')}</p>}
      </>
    );
  }
  if (kind === 'derby') return number('time_factor', '0.25');
  if (kind === 'open')
    return (
      <div className={row}>
        <span className={label}>{t('drv_open')}</span>
        <select value={value.open ?? 'time'} onChange={e => set({ ...value, open: e.target.value as Driving['open'] })} className={input}>
          <option value="time">{t('drv_open_time')}</option>
          <option value="points">{t('drv_open_points')}</option>
          <option value="penalty_points">{t('drv_open_penalty')}</option>
          <option value="points_then_time">{t('drv_open_points_time')}</option>
        </select>
      </div>
    );
  if (kind === 'multitest')
    return (
      <div className={row}>
        <span className={label}>{t('drv_multitest')}</span>
        <select value={value.multitest ?? 'B4'} onChange={e => set({ ...value, multitest: e.target.value })} className={input}>
          <option value="B1">B1</option>
          <option value="B4">B4</option>
        </select>
      </div>
    );
  return null;
}
