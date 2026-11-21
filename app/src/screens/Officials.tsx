// Officials (U6, after JuryApp's jury matrix), in tabs: the event's own officials (technical
// delegate, course designer, vet…), then each show day as a table: the officials as rows, the
// day's classes as columns, a role in each cell. A key types a role; a click (a tap) opens the
// roles at the cell. People come from the national list of officials, found in a dialog.

import { useEffect, useMemo, useRef, useState, type RefObject } from 'react';
import { Popover } from 'radix-ui';
import { ArrowRight, CalendarDays, CalendarRange, Check, ChevronRight, Gavel, Keyboard, Lock, Plus, Search, Trash2, TriangleAlert, UserMinus, UserPlus } from 'lucide-react';
import { call } from '../api';
import { formatDate, formatDayShort, useLanguage, usePlural, useT, type Key, type Language } from '../i18n';
import { useChanges } from '../parts/changes';
import { ErrorNote, useLoad } from '../parts/common';
import { PrintButton } from '../parts/print';
import { Avatar, Button, Callout, Card, CardHeader, EmptyState, Field, IconButton, Kbd, Modal, PageHeader, Pill, Switch, Tabs, useToast } from '../ui';

interface Cell {
  role: string;
  letter: string | null;
  /** Judges' marks are entered under this role: it stays (its position may change). */
  marked: boolean;
}
interface Row {
  person: number;
  name: string;
  town: string;
  cells: Record<string, Cell>;
}
interface Head {
  id: string;
  number: string;
  discipline: string;
  category: string;
  time: string;
  scoring: string;
  locked: boolean;
  /** Dressage judges' positions, as the result lists show them: E H C M B (driving's too). */
  letters: string;
  /** A driving class: no youth day's judge (T). */
  driving: boolean;
}
interface Day {
  date: string;
  classes: Head[];
  rows: Row[];
}
interface Table {
  days: Day[];
  roles: { code: string; de: string; fr: string; it: string }[];
}
interface Candidate {
  person: number;
  name: string;
  town: string;
  /** What the official is (Juge national de saut…), as Swiss Equestrian says it. */
  titles: { de: string; fr: string; it: string }[];
}
interface EventRole {
  assignment: string;
  person: number;
  name: string;
  town: string;
  role: string;
  role_id: number | null;
  role_name: string;
  days: number | null;
  responsible: boolean;
  readonly: boolean;
}
interface EventView {
  rows: EventRole[];
  roles: [number, string][];
}

/** A class's judges' positions (driving: C R S P V). */
const positions = (h: Head) => (h.letters || 'EHCMB').split('');
/** The style judge: EquiEvents' own role (not Swiss Equestrian's "R", the jumping judge), never
 * sent to Swiss Equestrian. */
const STYLE = 'eq-style';
/** A class marked by judges at positions, on judges' sheets: dressage (riding or driving) and
 *  driving's multi-test. */
const dressage = (h: Head) => h.discipline === 'CD' || h.discipline === 'MT';
/** A driving class (dressage, multi-test): its judges at E H C M B too. */
const driving = (h: Head) => h.driving;
/** The youth day's dressage judge (T): riding dressage only. */
const youth = (h: Head) => h.discipline === 'CD' && !driving(h);
const isJudge = (cell: Cell | undefined) => cell?.role === 'DRI' || cell?.role === 'DRITdj';

/** How a role shows in a cell: the judge's position for dressage judges, else a short code. */
function label(cell: Cell | undefined, language: Language): string {
  if (!cell) return '';
  if (isJudge(cell)) return `${cell.letter ?? '?'}${cell.role === 'DRITdj' ? '·T' : ''}`;
  if (cell.role === STYLE) return 'S';
  if (language === 'fr') return { R: 'J', RW: 'RI' }[cell.role] ?? cell.role;
  if (language === 'it') return { R: 'G' }[cell.role] ?? cell.role;
  return cell.role;
}

/** A role's colour in the table, so a day reads at a glance. */
function tone(cell: Cell): string {
  if (cell.role === 'P') return 'bg-accent-soft text-accent';
  if (cell.role === 'A') return 'bg-warn/14 text-warn';
  if (cell.role === STYLE) return 'bg-ok/12 text-ok';
  if (cell.role === 'R' || cell.role === 'RW' || isJudge(cell)) return 'bg-info/12 text-info';
  return 'bg-sunken text-ink';
}

/** What a key does in a cell: a role (with a position), clearing it, or nothing. */
function roleForKey(key: string, head: Head, cell: Cell | undefined): { role: string | null; letter?: string } | undefined {
  const k = key.toUpperCase();
  if (key === 'Backspace' || key === 'Delete') return { role: null };
  if (dressage(head) && positions(head).includes(k)) return { role: cell?.role === 'DRITdj' ? 'DRITdj' : 'DRI', letter: k };
  if (youth(head) && k === 'T' && cell && isJudge(cell)) return { role: cell.role === 'DRI' ? 'DRITdj' : 'DRI', letter: cell.letter ?? undefined };
  if (k === 'P') return { role: 'P' };
  // The jumping judge: R (Richter), J (juge), G (giudice).
  if (k === 'R' || k === 'J' || k === 'G') return { role: 'R' };
  if (k === 'S' && !dressage(head)) return { role: STYLE };
  if (k === 'W') return { role: 'RW' };
  if (k === 'A') return { role: 'A' };
  return undefined;
}

/** Another dialog or menu is open (not the roles at a cell): the keys are its own. */
const elsewhere = () => [...document.querySelectorAll('[role="dialog"][data-state="open"], [role="menu"]')].some(e => !e.hasAttribute('data-roles'));

export default function Officials({ show, language }: { show: string; language: Language }) {
  const t = useT();
  const { version } = useChanges();
  const table = useLoad(() => call<Table>('officials', { show }), [show, version]);
  const event = useLoad(() => call<EventView>('event_officials', { show, language }), [show, version, language]);
  const [tab, setTab] = useState(() => sessionStorage.getItem('officials-tab') ?? 'event');
  const [keys, setKeys] = useState(false);
  const days = table.data?.days ?? [];
  const day = days.find(d => d.date === tab);
  const open = (v: string) => {
    setTab(v);
    sessionStorage.setItem('officials-tab', v);
  };
  return (
    <div className="h-full overflow-auto">
      <div className="mx-auto max-w-[1600px] space-y-5 px-3 py-5 sm:px-6 sm:py-7">
        <PageHeader
          icon={Gavel}
          title={t('officials')}
          subtitle={t('officials_hint')}
          actions={
            <>
              <span className="hidden md:block">
                <Button variant="ghost" icon={Keyboard} onClick={() => setKeys(true)}>
                  {t('keyboard')}
                </Button>
              </span>
              <PrintButton show={show} args={{ what: 'officials' }} label={t('officials_plan')} />
            </>
          }
        />
        <ErrorNote error={table.error ?? event.error} />
        {table.data && (
          <Tabs
            value={day ? day.date : 'event'}
            onChange={open}
            className="border-b border-line"
            items={[
              { value: 'event', label: t('event_officials'), icon: Gavel, count: event.data?.rows.length ?? null },
              ...days.map(d => ({ value: d.date, label: formatDayShort(d.date, language), icon: CalendarDays, count: d.rows.length })),
            ]}
          />
        )}
        {!day && table.data && days.length === 0 && <Callout tone="info">{t('officials_no_classes')}</Callout>}
        {!day && event.data && <EventOfficials show={show} view={event.data} />}
        {day && table.data && <DayTable key={day.date} show={show} day={day} roles={table.data.roles} language={language} />}
      </div>
      <Modal open={keys} onClose={() => setKeys(false)} title={t('keyboard')} icon={Keyboard} size="sm">
        <dl className="grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-2.5 text-[13px]">
          {(
            [
              ['← → ↑ ↓', 'legend_move'],
              ['⇧ ← →', 'legend_several'],
              ['↵', 'roles_open'],
              ['P', 'legend_president'],
              [t('judge_key'), 'legend_judge'],
              ['S', 'legend_style'],
              ['W', 'legend_water'],
              ['A', 'legend_warmup'],
              ['E H C M B', 'legend_position'],
              ['T', 'legend_youth'],
              ['⌫', 'legend_remove'],
            ] as [string, Key][]
          ).map(([k, what]) => (
            <div key={what} className="contents">
              <dt className="flex gap-1">
                {k.split(' ').map(x => (
                  <Kbd key={x}>{x}</Kbd>
                ))}
              </dt>
              <dd className="text-ink-2 first-letter:uppercase">{t(what)}</dd>
            </div>
          ))}
        </dl>
      </Modal>
    </div>
  );
}

/** A show day: its officials (rows) and classes (columns), a role in each cell. */
function DayTable({ show, day, roles, language }: { show: string; day: Day; roles: Table['roles']; language: Language }) {
  const t = useT();
  const plural = usePlural();
  const { change } = useChanges();
  const { show: note } = useToast();
  // Who is being taken off the day (a dialog: confirm, or why not).
  const [removing, setRemoving] = useState<Row | null>(null);
  // Officials added to the day in this session, before they have a role.
  const [added, setAdded] = useState<Candidate[]>([]);
  const [active, setActive] = useState<{ person: number; cls: string } | null>(null);
  // Several classes of the active official's row (Shift+←→, Shift+click, or the whole day): from
  // this class to the active one.
  const [anchor, setAnchor] = useState<string | null>(null);
  // The roles, open at the active cell.
  const [menu, setMenu] = useState(false);
  const [finding, setFinding] = useState(false);
  const activeCell = useRef<HTMLTableCellElement>(null);

  const rows = useMemo(
    () => [...day.rows, ...added.filter(a => !day.rows.some(r => r.person === a.person)).map((a): Row => ({ person: a.person, name: a.name, town: a.town, cells: {} }))],
    [day.rows, added],
  );
  const head = active ? day.classes.find(c => c.id === active.cls) : undefined;
  const row = active ? rows.find(r => r.person === active.person) : undefined;

  // The selected classes of the row: from the anchor to the active one.
  const selected = (() => {
    if (!active || !head) return [];
    const to = day.classes.findIndex(c => c.id === active.cls);
    const from = anchor ? day.classes.findIndex(c => c.id === anchor) : to;
    if (from < 0) return [head];
    return day.classes.slice(Math.min(from, to), Math.max(from, to) + 1);
  })();

  /** A key typed, or a role picked: set the role of the selected cells (each class its own: a
   *  judge's position only in dressage classes; final classes left as they are). */
  function apply(key: string): boolean {
    if (!active || !head) return false;
    // A judge with marks keeps the role (only its position may change).
    const kept = (c: Head, role: NonNullable<ReturnType<typeof roleForKey>>) => {
      const cell = row?.cells[c.id];
      return !!cell?.marked && role.role !== cell.role;
    };
    if (selected.length <= 1) {
      if (head.locked) return false;
      const role = roleForKey(key, head, row?.cells[head.id]);
      if (!role) return false;
      if (kept(head, role)) {
        note({ text: t('marks_keep_role').replace('{c}', head.number), tone: 'info' });
        return true;
      }
      change('set_role', { class: head.id, person: active.person, role: role.role, letter: role.letter ?? null });
      return true;
    }
    const picked = selected
      .filter(c => !c.locked)
      .map(c => ({ c, role: roleForKey(key, c, row?.cells[c.id]) }))
      .filter((x): x is { c: Head; role: NonNullable<ReturnType<typeof roleForKey>> } => !!x.role);
    const held = picked.filter(x => kept(x.c, x.role));
    const roles = picked.filter(x => !kept(x.c, x.role)).map(({ c, role }) => ({ class: c.id, role: role.role, letter: role.letter ?? null }));
    if (held.length > 0) note({ text: held.map(x => t('marks_keep_role').replace('{c}', x.c.number)).join(' '), tone: 'info' });
    if (roles.length === 0) return held.length > 0;
    change('set_roles', { person: active.person, roles });
    return true;
  }

  useEffect(() => {
    if (!active) return;
    const key = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target?.closest('input, select, textarea') || e.metaKey || e.ctrlKey || e.altKey || elsewhere()) return;
      const ci = day.classes.findIndex(c => c.id === active.cls);
      const ri = rows.findIndex(r => r.person === active.person);
      if (ci < 0 || ri < 0) return;
      const move = (dc: number, dr: number) => {
        e.preventDefault();
        setMenu(false);
        setAnchor(null);
        setActive({ person: rows[(ri + dr + rows.length) % rows.length].person, cls: day.classes[(ci + dc + day.classes.length) % day.classes.length].id });
      };
      // Shift+←→: the selection grows along the row (no wrapping).
      const extend = (dc: number) => {
        e.preventDefault();
        const nc = ci + dc;
        if (nc < 0 || nc >= day.classes.length) return;
        setAnchor(a => a ?? active.cls);
        setActive({ ...active, cls: day.classes[nc].id });
      };
      if (e.shiftKey && e.key === 'ArrowRight') return extend(1);
      if (e.shiftKey && e.key === 'ArrowLeft') return extend(-1);
      if (e.key === 'ArrowRight' || (e.key === 'Tab' && !e.shiftKey)) return move(1, 0);
      if (e.key === 'ArrowLeft' || (e.key === 'Tab' && e.shiftKey)) return move(-1, 0);
      if (e.key === 'ArrowDown') return move(0, 1);
      if (e.key === 'ArrowUp') return move(0, -1);
      if (e.key === 'Enter') {
        e.preventDefault();
        return setMenu(m => !m);
      }
      if (e.key === 'Escape') {
        if (menu) return setMenu(false);
        return anchor ? setAnchor(null) : setActive(null);
      }
      if (apply(e.key)) {
        e.preventDefault();
        setMenu(false);
      }
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  });

  const add = () => setFinding(true);
  const th = 'border-b border-l border-line bg-raised px-2 py-2.5 text-center align-bottom';
  return (
    <Card className="overflow-hidden">
      <CardHeader
        icon={CalendarDays}
        title={<span className="capitalize">{formatDate(day.date, language)}</span>}
        subtitle={`${rows.length} ${plural(rows.length, 'officials_count')} · ${day.classes.length} ${plural(day.classes.length, 'classes_selected')}`}
        actions={
          <Button variant="secondary" icon={Plus} onClick={add}>
            {t('add_official_short')}
          </Button>
        }
      />
      <div className="overflow-x-auto">
        <table className="border-separate border-spacing-0 text-sm" style={{ width: 'max-content', minWidth: '100%' }}>
          <thead>
            <tr>
              <th className="sticky left-0 z-10 w-40 min-w-40 border-r border-b border-line bg-raised px-4 py-2.5 text-left align-bottom text-xs font-semibold text-ink-3 sm:w-64 sm:min-w-64">
                {t('official')}
              </th>
              {day.classes.map(c => (
                <th key={c.id} className={`${th} w-16 min-w-16 sm:w-24 sm:min-w-24`} title={c.scoring}>
                  <div className="flex items-center justify-center gap-1 text-[13px] font-bold text-ink">
                    {c.locked && <Lock className="size-3 text-ok" aria-label={t('status_final')} />}
                    {c.number}
                  </div>
                  <div className="truncate text-[10px] leading-tight font-medium text-ink-3">
                    {day.classes.some(x => x !== c && x.number === c.number) ? `${c.discipline} · ${c.category}` : c.category}
                  </div>
                  {c.time && <div className="text-[10px] leading-tight font-normal text-ink-4 tabular">{c.time}</div>}
                </th>
              ))}
              <th className="border-b border-l border-line bg-raised" />
            </tr>
          </thead>
          <tbody>
            {rows.map(r => (
              <tr key={r.person} className="group">
                <td className="sticky left-0 z-10 max-w-40 border-r border-b border-line bg-card px-4 py-2 group-hover:bg-raised sm:max-w-none">
                  <div className="flex min-w-0 items-center gap-2.5">
                    <span className="hidden sm:block">
                      <Avatar name={r.name} size="sm" />
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-[13px] font-semibold text-ink">{r.name}</div>
                      <div className="truncate text-xs text-ink-3">{r.town}</div>
                    </div>
                    <span className="opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100 pointer-coarse:opacity-100">
                      <IconButton icon={UserMinus} label={t('remove_official_day')} size="sm" onClick={() => setRemoving(r)} />
                    </span>
                  </div>
                </td>
                {day.classes.map(c => {
                  const cell = r.cells[c.id];
                  const on = active?.person === r.person && active.cls === c.id;
                  const inSelection = !on && active?.person === r.person && selected.length > 1 && selected.some(x => x.id === c.id);
                  const duplicate = cell && isJudge(cell) && rows.some(o => o.person !== r.person && o.cells[c.id]?.letter === cell.letter && o.cells[c.id]?.role.startsWith('DRI'));
                  return (
                    <td
                      key={c.id}
                      ref={on ? activeCell : undefined}
                      onClick={e => {
                        // Shift+click: from the selected cell to this one, in the same row.
                        if (e.shiftKey && active?.person === r.person) setAnchor(a => a ?? active.cls);
                        else setAnchor(null);
                        setActive({ person: r.person, cls: c.id });
                        setMenu(m => (on && !e.shiftKey ? !m : true));
                      }}
                      title={cell ? roles.find(x => x.code === cell.role)?.[language] : undefined}
                      className={`relative h-12 cursor-pointer border-b border-l border-line p-1 text-center select-none ${
                        on ? 'bg-accent/6 outline-2 -outline-offset-2 outline-accent' : inSelection ? 'bg-accent/6 outline-1 -outline-offset-2 outline-accent/50' : 'hover:bg-hover'
                      } ${c.locked ? 'bg-sunken/50' : ''}`}
                    >
                      {cell && (
                        <span
                          className={`relative inline-grid h-7 min-w-8 place-items-center rounded-md px-1.5 text-xs font-bold tabular ${duplicate ? 'bg-danger/15 text-danger ring-1 ring-danger/40' : tone(cell)} ${c.locked ? 'opacity-60' : ''}`}
                        >
                          {label(cell, language)}
                          {/* Marks entered: the role stays. */}
                          {cell.marked && <span className="absolute -top-1 -right-1 size-2.5 rounded-full bg-warn ring-2 ring-card" aria-label={t('marks_entered')} />}
                        </span>
                      )}
                      {on && menu && head && row && (
                        <RolesPopover
                          cellRef={activeCell}
                          head={head}
                          official={row.name}
                          cell={row.cells[head.id]}
                          show={show}
                          count={selected.length}
                          apply={apply}
                          wholeDay={() => {
                            setAnchor(day.classes[0].id);
                            setActive({ person: r.person, cls: day.classes[day.classes.length - 1].id });
                          }}
                          close={() => setMenu(false)}
                        />
                      )}
                    </td>
                  );
                })}
                <td className="border-b border-l border-line" />
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {rows.length === 0 && (
        <EmptyState
          icon={UserPlus}
          title={t('officials_empty_day')}
          action={
            <Button variant="primary" icon={Plus} onClick={add}>
              {t('add_official_short')}
            </Button>
          }
        >
          {t('officials_empty_day_hint')}
        </EmptyState>
      )}
      {removing && (
        <RemoveOfficial
          show={show}
          row={removing}
          classes={day.classes}
          language={language}
          onClose={() => setRemoving(null)}
          remove={async () => {
            const cells = Object.keys(removing.cells);
            const done = cells.length === 0 || (await change('set_roles', { person: removing.person, roles: cells.map(c => ({ class: c })) }));
            if (done) {
              setAdded(a => a.filter(x => x.person !== removing.person));
              if (active?.person === removing.person) setActive(null);
              setRemoving(null);
            }
          }}
        />
      )}
      <FindOfficial
        open={finding}
        onClose={() => setFinding(false)}
        show={show}
        listed={new Set(rows.map(r => r.person))}
        onPick={c => {
          setFinding(false);
          if (!rows.some(r => r.person === c.person)) setAdded(a => [...a, c]);
          // The new official's first cell, ready for a key.
          if (day.classes[0]) {
            setAnchor(null);
            setMenu(false);
            setActive({ person: c.person, cls: day.classes[0].id });
          }
        }}
      />
    </Card>
  );
}

/** The roles for the selected cell(s), at the cell: what each key does, for a mouse or a finger. */
function RolesPopover({
  cellRef,
  head,
  official,
  cell,
  show,
  count,
  apply,
  wholeDay,
  close,
}: {
  cellRef: RefObject<HTMLTableCellElement | null>;
  head: Head;
  official: string;
  cell: Cell | undefined;
  show: string;
  count: number;
  apply: (key: string) => boolean;
  wholeDay: () => void;
  close: () => void;
}) {
  const t = useT();
  const plural = usePlural();
  const several = count > 1;
  const judge = isJudge(cell);
  const locked = !several && head.locked;
  // Marks entered under the role: it stays; a judge's position may change.
  const marked = !several && !!cell?.marked;
  const run = (key: string) => apply(key) && close();
  const item =
    'flex min-h-9 w-full items-center gap-2.5 rounded-lg px-2 text-left text-[13px] text-ink outline-none transition-colors hover:bg-hover focus-visible:bg-hover disabled:pointer-events-none disabled:opacity-40 pointer-coarse:min-h-11';
  const roles: [string, Key, boolean][] = [
    ['P', 'legend_president', cell?.role === 'P'],
    [t('judge_key'), 'legend_judge', cell?.role === 'R'],
    ['S', 'legend_style', cell?.role === STYLE],
    ['W', 'legend_water', cell?.role === 'RW'],
    ['A', 'legend_warmup', cell?.role === 'A'],
  ];
  return (
    <Popover.Root open onOpenChange={o => !o && close()}>
      <Popover.Anchor asChild>
        <span className="pointer-events-none absolute inset-0" aria-hidden />
      </Popover.Anchor>
      <Popover.Portal>
        <Popover.Content
          data-roles=""
          side="bottom"
          align="center"
          sideOffset={6}
          collisionPadding={10}
          onOpenAutoFocus={e => e.preventDefault()}
          onCloseAutoFocus={e => e.preventDefault()}
          // Esc closes the roles only; a second Esc unselects the cell (the table's).
          onEscapeKeyDown={e => e.stopPropagation()}
          onPointerDownOutside={e => {
            if (cellRef.current?.contains(e.target as Node)) e.preventDefault();
          }}
          // The cell's click is the React parent's: not for clicks in here.
          onClick={e => e.stopPropagation()}
          className="z-[55] w-64 max-w-[calc(100vw-20px)] animate-pop-in rounded-xl border border-line bg-card p-1.5 text-left font-normal shadow-pop"
        >
          <div className="px-2 pt-1 pb-2">
            <div className="truncate text-[13px] font-semibold text-ink">{official}</div>
            <div className="truncate text-xs text-ink-3">{several ? `${count} ${plural(count, 'classes_selected')}` : `${head.number} · ${head.category}${head.time ? ` · ${head.time}` : ''}`}</div>
          </div>
          {marked && !locked && (
            <div className="mx-1 mb-1.5 rounded-lg bg-warn/10 px-2.5 py-2 text-xs text-warn">
              <div className="flex items-start gap-2">
                <TriangleAlert className="mt-px size-3.5 shrink-0" />
                <span>{t('marks_entered')}</span>
              </div>
              <a href={resultsHref(show, head.id)} onClick={toResults} className="mt-1 ml-5.5 inline-flex items-center gap-1 font-semibold hover:underline">
                {t('open_results')}
                <ArrowRight className="size-3" />
              </a>
            </div>
          )}
          {locked ? (
            <div className="mx-1 mb-1 flex items-center gap-2 rounded-lg bg-sunken px-2.5 py-2 text-xs text-ink-3">
              <Lock className="size-3.5" />
              {t('status_final')}
            </div>
          ) : dressage(head) ? (
            <div className="space-y-1 px-1 pb-1">
              <div className="px-1 text-[11px] font-semibold text-ink-3 first-letter:uppercase">{t(driving(head) ? 'legend_position_driving' : 'legend_position')}</div>
              <div className="flex gap-1">
                {positions(head).map(p => {
                  const on = judge && cell?.letter === p;
                  return (
                    <button
                      key={p}
                      onClick={() => run(p)}
                      className={`grid h-10 flex-1 place-items-center rounded-lg text-sm font-bold transition-colors ${on ? 'bg-accent text-on-accent' : 'bg-sunken text-ink hover:bg-line'}`}
                    >
                      {p}
                    </button>
                  );
                })}
              </div>
              {youth(head) && (
                <button disabled={!judge || marked} onClick={() => run('T')} className={item}>
                  <Kbd>T</Kbd>
                  <span className="flex-1 first-letter:uppercase">{t('legend_youth')}</span>
                  {cell?.role === 'DRITdj' && <Check className="size-4 text-accent" />}
                </button>
              )}
              {/* The warm-up arena's judge, under the positions. */}
              <button disabled={marked && cell?.role !== 'A'} onClick={() => run('A')} className={item}>
                <span className={`grid size-6 shrink-0 place-items-center rounded-md text-xs font-bold ${cell?.role === 'A' ? 'bg-accent text-on-accent' : 'bg-sunken text-ink'}`}>A</span>
                <span className="flex-1 first-letter:uppercase">{t('legend_warmup')}</span>
                {cell?.role === 'A' && <Check className="size-4 text-accent" />}
              </button>
            </div>
          ) : (
            <div>
              {roles.map(([key, what, on]) => (
                <button key={key} disabled={marked && !on} onClick={() => run(key)} className={item}>
                  <span className={`grid size-6 shrink-0 place-items-center rounded-md text-xs font-bold ${on ? 'bg-accent text-on-accent' : 'bg-sunken text-ink'}`}>{key}</span>
                  <span className="flex-1 first-letter:uppercase">{t(what)}</span>
                  {on && <Check className="size-4 text-accent" />}
                </button>
              ))}
            </div>
          )}
          <div className="mx-1 my-1 h-px bg-line" />
          <button disabled={!several && (locked || !cell || marked)} onClick={() => run('Delete')} className={`${item} text-danger hover:bg-danger/8`}>
            <Trash2 className="size-4 shrink-0" />
            <span className="flex-1 first-letter:uppercase">{t('legend_remove')}</span>
            <Kbd>⌫</Kbd>
          </button>
          {!several && (
            <button onClick={wholeDay} title={t('whole_day_hint')} className={item}>
              <CalendarRange className="size-4 shrink-0 text-ink-3" />
              <span className="flex-1">{t('whole_day')}</span>
            </button>
          )}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

/** A class's page on its results (« Résultats »), where marks are deleted. */
const resultsHref = (show: string, cls: string) => `#/show/${encodeURIComponent(show)}/classes/${encodeURIComponent(cls)}`;
const toResults = () => {
  try {
    localStorage.setItem('class-tab', 'entry');
  } catch {
    // The class opens on its last tab.
  }
};

/** Taking an official off a day: their roles there go, after a confirmation; never while judges'
 *  marks are entered under one of them (the classes are named, each a link to its results). */
function RemoveOfficial({ show, row, classes, language, onClose, remove }: { show: string; row: Row; classes: Head[]; language: Language; onClose: () => void; remove: () => Promise<void> }) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const roles = classes.filter(c => row.cells[c.id]);
  const marked = roles.filter(c => row.cells[c.id].marked);
  const chip = (c: Head) => <span className={`inline-grid h-6 min-w-7 place-items-center rounded-md px-1.5 text-[11px] font-bold ${tone(row.cells[c.id])}`}>{label(row.cells[c.id], language)}</span>;
  if (marked.length > 0)
    return (
      <Modal open onClose={onClose} title={t('cannot_remove_official').replace('{name}', row.name)} icon={TriangleAlert} tone="warn" size="sm" footer={<Button onClick={onClose}>{t('close')}</Button>}>
        <div className="grid gap-4">
          <Callout tone="warn">{t('marks_tied')}</Callout>
          <ul className="overflow-hidden rounded-xl border border-line">
            {marked.map(c => (
              <li key={c.id} className="border-t border-line first:border-t-0">
                <a
                  href={resultsHref(show, c.id)}
                  onClick={() => {
                    toResults();
                    onClose();
                  }}
                  className="flex items-center gap-3 px-3 py-2.5 hover:bg-hover"
                >
                  <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-sunken text-[13px] font-bold text-ink ring-1 ring-line">{c.number}</span>
                  <span className="min-w-0 flex-1 truncate text-[13px] text-ink">
                    {c.discipline} · {c.category}
                  </span>
                  {chip(c)}
                  <span className="inline-flex items-center gap-1 text-xs font-semibold text-accent">
                    {t('open_results')}
                    <ArrowRight className="size-3.5" />
                  </span>
                </a>
              </li>
            ))}
          </ul>
        </div>
      </Modal>
    );
  return (
    <Modal
      open
      onClose={onClose}
      title={t('remove_official_title').replace('{name}', row.name)}
      icon={UserMinus}
      tone="danger"
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button
            variant="danger-solid"
            icon={UserMinus}
            loading={busy}
            onClick={async () => {
              setBusy(true);
              await remove();
              setBusy(false);
            }}
          >
            {t('remove')}
          </Button>
        </>
      }
    >
      {roles.length === 0 ? (
        <p className="text-sm text-ink-2">{t('remove_official_none')}</p>
      ) : (
        <div className="grid gap-2.5">
          <p className="text-sm text-ink-2">{t('remove_official_roles')}</p>
          <div className="flex flex-wrap gap-2">
            {roles.map(c => (
              <span key={c.id} className="inline-flex items-center gap-1.5 rounded-lg bg-sunken py-1 pr-1 pl-2 text-xs font-semibold text-ink">
                {c.number}
                {chip(c)}
              </span>
            ))}
          </div>
        </div>
      )}
    </Modal>
  );
}

/** Finding someone in the national list of officials (and among the show's people). */
function FindOfficial({ open, onClose, show, listed, onPick }: { open: boolean; onClose: () => void; show: string; listed: Set<number>; onPick: (c: Candidate) => void }) {
  const t = useT();
  const language = useLanguage();
  const [query, setQuery] = useState('');
  const [found, setFound] = useState<Candidate[] | null>(null);
  const [index, setIndex] = useState(0);
  const list = useRef<HTMLUListElement>(null);
  useEffect(() => {
    if (!query.trim()) return setFound(null);
    let live = true;
    call<Candidate[]>('find_officials', { show, query }).then(f => live && (setFound(f), setIndex(0)));
    return () => {
      live = false;
    };
  }, [show, query]);
  useEffect(() => {
    list.current?.querySelector('[data-on]')?.scrollIntoView({ block: 'nearest' });
  }, [found, index]);
  const reset = () => {
    setQuery('');
    setFound(null);
  };
  const pick = (c: Candidate | undefined) => {
    if (!c) return;
    reset();
    onPick(c);
  };
  return (
    <Modal
      open={open}
      onClose={() => {
        reset();
        onClose();
      }}
      title={t('add_official_title')}
      description={t('find_official_hint')}
      icon={UserPlus}
      size="md"
    >
      <div className="relative">
        <Search className="pointer-events-none absolute top-1/2 left-3.5 size-4 -translate-y-1/2 text-ink-4" />
        <input
          value={query}
          onChange={e => setQuery(e.target.value)}
          onKeyDown={e => {
            if (e.key === 'ArrowDown') (e.preventDefault(), setIndex(i => Math.min(i + 1, (found?.length ?? 1) - 1)));
            if (e.key === 'ArrowUp') (e.preventDefault(), setIndex(i => Math.max(i - 1, 0)));
            if (e.key === 'Enter') pick(found?.[index]);
          }}
          placeholder={t('find_official_placeholder')}
          className="field h-11 pl-10"
        />
      </div>
      {found && (
        <ul ref={list} className="mt-3 max-h-[46vh] overflow-auto rounded-xl border border-line">
          {found.length === 0 && <li className="px-4 py-8 text-center text-sm text-ink-3">{t('no_official_found')}</li>}
          {found.map((c, i) => (
            <li key={c.person} className="border-t border-line first:border-t-0">
              <button
                data-on={i === index ? '' : undefined}
                onMouseMove={() => setIndex(i)}
                onClick={() => pick(c)}
                className={`flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors ${i === index ? 'bg-hover' : ''}`}
              >
                <Avatar name={c.name} size="sm" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[13px] font-semibold text-ink">
                    {c.name} <span className="font-normal text-ink-3">· {c.town}</span>
                  </span>
                  {c.titles.length > 0 && <span className="line-clamp-2 text-xs text-ink-3">{c.titles.map(title => title[language]).join(' · ')}</span>}
                </span>
                {listed.has(c.person) && <Pill>{t('already_listed')}</Pill>}
              </button>
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}

/** The officials of the event itself: a list; one opens in a dialog (role, days, responsible). */
function EventOfficials({ show, view }: { show: string; view: EventView }) {
  const t = useT();
  const [finding, setFinding] = useState(false);
  // The official in the dialog: one of the list, or someone just found.
  const [editing, setEditing] = useState<{ row?: EventRole; person: number; name: string; town: string } | null>(null);
  return (
    <Card className="overflow-hidden">
      <CardHeader
        icon={Gavel}
        title={t('event_officials')}
        subtitle={t('event_officials_hint')}
        actions={
          <Button variant="secondary" icon={Plus} onClick={() => setFinding(true)}>
            {t('add_official_short')}
          </Button>
        }
      />
      {view.rows.length === 0 && <EmptyState icon={Gavel} title={t('event_officials_none')} />}
      <ul>
        {view.rows.map(r => (
          <li key={r.assignment} className="border-t border-line first:border-t-0">
            <button
              onClick={() => setEditing({ row: r, person: r.person, name: r.name, town: r.town })}
              className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-hover sm:px-5"
            >
              <Avatar name={r.name} />
              <span className="min-w-0 flex-1 sm:grid sm:grid-cols-2 sm:items-center sm:gap-4">
                <span className="block min-w-0">
                  <span className="block truncate text-[13px] font-semibold text-ink">{r.name}</span>
                  <span className="block truncate text-xs text-ink-3">{r.town}</span>
                </span>
                <span className="block truncate text-[13px] text-ink-2">{r.role_name}</span>
              </span>
              <span className="flex shrink-0 items-center justify-end gap-1.5 sm:w-72">
                {r.days != null && (
                  <Pill>
                    {r.days} {t('days')}
                  </Pill>
                )}
                {r.responsible && (
                  <Pill tone="info" icon={Check}>
                    {t('responsible')}
                  </Pill>
                )}
                {r.readonly && (
                  <Pill tone="brand" title={t('set_by_se')}>
                    Swiss Equestrian
                  </Pill>
                )}
                <ChevronRight className="size-4 text-ink-4" />
              </span>
            </button>
          </li>
        ))}
      </ul>
      <FindOfficial
        open={finding}
        onClose={() => setFinding(false)}
        show={show}
        listed={new Set(view.rows.map(r => r.person))}
        onPick={c => {
          setFinding(false);
          setEditing({ person: c.person, name: c.name, town: c.town });
        }}
      />
      {editing && <EventRoleModal key={editing.row?.assignment ?? `new-${editing.person}`} editing={editing} roles={view.roles} onClose={() => setEditing(null)} />}
    </Card>
  );
}

/** One official of the event: role (unless Swiss Equestrian's), days of work, responsible. */
function EventRoleModal({ editing, roles, onClose }: { editing: { row?: EventRole; person: number; name: string; town: string }; roles: [number, string][]; onClose: () => void }) {
  const t = useT();
  const { change } = useChanges();
  const row = editing.row;
  const [role, setRole] = useState(row?.role_id != null ? String(row.role_id) : '');
  const [days, setDays] = useState(row?.days != null ? String(row.days) : '');
  const [responsible, setResponsible] = useState(row?.responsible ?? false);
  const [busy, setBusy] = useState(false);
  const daysOk = !days.trim() || /^\d+$/.test(days.trim());
  const ready = (row?.readonly || role !== '') && daysOk;
  const save = async () => {
    if (!ready) return;
    setBusy(true);
    const done = await change('set_event_role', {
      ...(row ? { assignment: row.assignment } : {}),
      person: editing.person,
      role: row?.readonly ? row.role_id : Number(role),
      days: days.trim() ? Number(days) : null,
      responsible,
    });
    setBusy(false);
    if (done) onClose();
  };
  const remove = async () => {
    if (!row) return;
    setBusy(true);
    const done = await change('remove_event_role', { assignment: row.assignment });
    setBusy(false);
    if (done) onClose();
  };
  return (
    <Modal
      open
      onClose={onClose}
      title={row ? t('event_official') : t('add_official_title')}
      icon={Gavel}
      size="sm"
      footer={
        <>
          {row && !row.readonly && (
            <Button variant="danger" icon={Trash2} className="mr-auto" disabled={busy} onClick={remove}>
              {t('remove')}
            </Button>
          )}
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button variant="primary" loading={busy} disabled={!ready} onClick={save}>
            {row ? t('save') : t('add')}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="flex items-center gap-3 rounded-xl bg-raised px-3 py-2.5">
          <Avatar name={editing.name} />
          <div className="min-w-0">
            <div className="truncate text-[13px] font-semibold text-ink">{editing.name}</div>
            <div className="truncate text-xs text-ink-3">{editing.town}</div>
          </div>
        </div>
        {row?.readonly ? (
          <Field label={t('role')} hint={t('set_by_se')}>
            <div className="field flex items-center bg-sunken text-ink-2">{row.role_name}</div>
          </Field>
        ) : (
          <Field label={t('role')}>
            <select value={role} onChange={e => setRole(e.target.value)} className="field">
              {!row && <option value="">—</option>}
              {roles.map(([code, name]) => (
                <option key={code} value={code}>
                  {name}
                </option>
              ))}
            </select>
          </Field>
        )}
        <Field label={t('days_worked')} hint={daysOk ? undefined : t('days_whole')}>
          <input value={days} onChange={e => setDays(e.target.value)} onKeyDown={e => e.key === 'Enter' && save()} inputMode="numeric" className="field w-28 tabular" />
        </Field>
        <Switch checked={responsible} onChange={setResponsible} label={t('responsible_long')} hint={t('responsible_hint')} />
      </div>
    </Modal>
  );
}
