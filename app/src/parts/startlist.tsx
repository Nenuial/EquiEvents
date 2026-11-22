// A class's start list that the office can change in place: drag a starter by its grip (mouse,
// finger or keyboard), or Alt+↑↓ on the selected one, or type its new place; N does not start,
// H hors concours; send a starter to another section. A rider's horses (the "1/2" badge) and the
// teams' order are rearranged in a dialog. A search and a sort by any column (a click on its
// head) change only what is shown, never the order; more columns (as Rosson's table offers:
// licence, passport, signalement, winning points…) are picked in a menu and remembered.

import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { ArrowDown, ArrowRight, ArrowUp, ArrowUpDown, Columns3, Info, Lock, MessageSquareText, NotebookPen, Pencil, RotateCcw, Search, Users, X } from 'lucide-react';
import type { ClassRow, StartLine, StartListView } from '../api';
import { useT, type Key } from '../i18n';
import { Button, Callout, Menu, MenuCheck, MenuItem, MenuLabel, MenuSeparator, Modal, Pill, Switch, Tooltip, overlayOpen } from '../ui';
import { useChanges } from './changes';
import { RiderDialog } from './entries';
import { useMedia } from './common';
import { Grip, SortableList, SortRow } from './sortable';

type Starter = Extract<StartLine, { kind: 'starter' }>;

/** The columns the office can add (none by default), by what they are about; shown in the menu
 *  only when a starter of the list has a value. */
type Extra = { key: string; label: Key; group: 'rider' | 'horse' | 'columns_entry' | 'columns_driving'; numeric?: boolean };
const EXTRA: Extra[] = [
  { key: 'rider_id', label: 'col_licence_no', group: 'rider', numeric: true },
  { key: 'town', label: 'col_town', group: 'rider' },
  { key: 'club', label: 'col_club', group: 'rider' },
  { key: 'nationality', label: 'col_nation', group: 'rider' },
  { key: 'licence', label: 'col_licences', group: 'rider' },
  { key: 'youth', label: 'col_youth', group: 'rider' },
  { key: 'horse_id', label: 'col_passport', group: 'horse', numeric: true },
  { key: 'pony', label: 'col_pony', group: 'horse' },
  { key: 'pony_permit', label: 'col_pony_permit', group: 'horse' },
  { key: 'signalement', label: 'col_signalement', group: 'horse' },
  { key: 'owner', label: 'col_owner', group: 'horse' },
  { key: 'breeder', label: 'col_breeder', group: 'horse' },
  { key: 'sire', label: 'col_sire', group: 'horse' },
  { key: 'dam', label: 'col_dam', group: 'horse' },
  { key: 'dam_sire', label: 'col_dam_sire', group: 'horse' },
  { key: 'level', label: 'level', group: 'columns_entry', numeric: true },
  { key: 'points_pair', label: 'col_points_pair', group: 'columns_entry', numeric: true },
  { key: 'points_rider', label: 'col_points_rider', group: 'columns_entry', numeric: true },
  { key: 'points_horse', label: 'col_points_horse', group: 'columns_entry', numeric: true },
  { key: 'fee', label: 'col_fee', group: 'columns_entry', numeric: true },
  { key: 'status', label: 'col_status', group: 'columns_entry' },
  { key: 'team', label: 'team', group: 'columns_entry' },
  { key: 'remarks', label: 'col_remarks', group: 'columns_entry' },
  { key: 'turnout', label: 'col_turnout', group: 'columns_driving' },
  { key: 'width', label: 'col_width', group: 'columns_driving', numeric: true },
];
const GROUPS: Extra['group'][] = ['rider', 'horse', 'columns_entry', 'columns_driving'];
const valueOf = (l: Starter, key: string) =>
  key === 'town' ? l.town : key === 'club' ? l.club : key === 'remarks' ? [l.more?.remark, l.more?.note].filter(Boolean).join(' · ') : (l.more?.[key] ?? '');

/** The columns shown when the office has not picked any. */
const DEFAULT_COLUMNS = ['remarks'];
/** Where the choice is remembered (a new name when the defaults change: they show again). */
const STORE = 'start-list-columns-2';

/** A starter's remarks: the rider's (made with the entry) and the office's note, each marked. */
function Remarks({ line }: { line: Starter }) {
  const t = useT();
  const remark = line.more?.remark;
  const note = line.more?.note;
  if (!remark && !note) return null;
  return (
    <div className="space-y-0.5 text-xs leading-snug">
      {remark && (
        <div className="flex items-start gap-1.5 text-ink-2" title={`${t('remark_rider')} : ${remark}`}>
          <MessageSquareText className="mt-px size-3.5 shrink-0 text-ink-4" aria-label={t('remark_rider')} />
          <span className="line-clamp-2 min-w-0">{remark}</span>
        </div>
      )}
      {note && (
        <div className="flex items-start gap-1.5 text-warn" title={`${t('col_note')} : ${note}`}>
          <NotebookPen className="mt-px size-3.5 shrink-0" aria-label={t('col_note')} />
          <span className="line-clamp-2 min-w-0 italic">{note}</span>
        </div>
      )}
    </div>
  );
}

/** Swiss Equestrian's status of an entry, in the office's words where it has some. */
function useStatusText() {
  const t = useT();
  return (status: string) => (status === 'abgemeldet' ? t('withdrawn') : status === 'falschnennung' ? t('pub_wrong') : status);
}

/** The columns picked, remembered on this device. */
function useColumns(): [string[], (keys: string[] | null) => void] {
  const [keys, setKeys] = useState<string[]>(() => {
    try {
      const kept = localStorage.getItem(STORE);
      return kept === null ? DEFAULT_COLUMNS : JSON.parse(kept);
    } catch {
      return DEFAULT_COLUMNS;
    }
  });
  // `null`: the default columns again.
  const set = (next: string[] | null) => {
    setKeys(next ?? DEFAULT_COLUMNS);
    try {
      if (next === null) localStorage.removeItem(STORE);
      else localStorage.setItem(STORE, JSON.stringify(next));
    } catch {
      // Not remembered: still shown.
    }
  };
  return [keys, set];
}

/** What is shown sorted by: a column (`place`, `number`, `time`, `rider`, `horse`, `carried` or an extra one). */
type Sort = { key: string; desc: boolean } | null;
const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

const th = 'sticky top-0 z-[1] border-b border-line bg-raised px-3 py-2.5 text-left text-xs font-semibold text-ink-3';
const td = 'px-3 py-2.5 align-middle';
// A row's quick toggles: quiet until set, or hovered; on a touch screen always there, larger.
const chip = 'h-7 rounded-md px-2 text-[11px] font-bold transition-colors pointer-coarse:h-9 pointer-coarse:px-3';
const quiet = 'text-ink-4 opacity-0 ring-1 ring-line group-hover:opacity-100 hover:bg-hover hover:text-ink pointer-coarse:opacity-100';

export function StartList({ show, list, highlight, sections = [] }: { show: string; list: StartListView; highlight?: number; sections?: ClassRow[] }) {
  const t = useT();
  const { change } = useChanges();
  const editable = list.class.status !== 'final';
  const [selected, setSelected] = useState<number | undefined>(highlight);
  const [horsesOf, setHorsesOf] = useState<number | null>(null);
  // A rider's dialog (their name clicked): every entry, a change of rider or horse…
  const [riderOf, setRiderOf] = useState<number | null>(null);
  const [teamOrder, setTeamOrder] = useState(false);
  // A team's name being changed (team classes).
  const [renaming, setRenaming] = useState<string | null>(null);
  const box = useRef<HTMLDivElement>(null);
  const classId = list.class.id;
  const starters = list.lines.filter((l): l is Starter => l.kind === 'starter');
  const team = list.lines.some(l => l.kind === 'team');
  // (Below `sm` the start number, the time and the horse have no column of their own: the number
  // is before the rider, the time before the club, the horse under them; the rider keeps room.)
  const phone = useMedia('(max-width: 639px)');
  const baseColumns = phone ? 3 + (list.qualifier ? 1 : 0) : 4 + (list.start_numbers ? 1 : 0) + (list.start_times ? 1 : 0) + (list.qualifier ? 1 : 0);
  // Riders with several horses (U11, U12): their starts in order, the smallest gap between them.
  const byRider = new Map<number, Starter[]>();
  for (const l of starters) byRider.set(l.person, [...(byRider.get(l.person) ?? []), l]);
  const gapOf = (person: number) => {
    const places = (byRider.get(person) ?? []).map(l => starters.indexOf(l));
    return places.length < 2 ? null : Math.min(...places.slice(1).map((p, i) => p - places[i]));
  };
  // Closer than half the even spacing: worth a look (a late entry at the end, a move by hand).
  const tooClose = (person: number) => {
    const k = byRider.get(person)?.length ?? 0;
    const gap = gapOf(person);
    return gap !== null && gap < Math.floor(starters.length / k / 2);
  };
  const teams = list.lines.filter((l): l is Extract<StartLine, { kind: 'team' }> => l.kind === 'team');

  // What is shown: searched, sorted by a column, with the columns picked. The order stays.
  const [query, setQuery] = useState('');
  const [sort, setSort] = useState<Sort>(null);
  const [chosen, setChosen] = useColumns();
  const statusText = useStatusText();
  // The default columns always (empty or not); the others when a starter has a value.
  const available = EXTRA.filter(c => DEFAULT_COLUMNS.includes(c.key) || starters.some(s => valueOf(s, c.key) !== ''));
  const extras = available.filter(c => chosen.includes(c.key));
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const viewed = sort !== null || words.length > 0;
  const shown = useMemo(() => {
    const found = starters.filter(l => {
      const text = [l.rider, l.horse, l.club, l.town, l.start_number ?? '', ...Object.values(l.more ?? {})].join(' ').toLowerCase();
      return words.every(w => text.includes(w));
    });
    if (!sort) return found;
    const numeric = ['place', 'number', 'carried'].includes(sort.key) || EXTRA.some(c => c.key === sort.key && c.numeric);
    const value = (l: Starter): string => {
      switch (sort.key) {
        case 'place':
          return String(l.position);
        case 'number':
          return l.start_number === null ? '' : String(l.start_number);
        case 'time':
          return l.time ?? '';
        case 'rider':
          return l.more?.rider_sort ?? l.rider;
        case 'horse':
          return l.horse;
        case 'carried':
          return l.carried ?? '';
        default:
          return valueOf(l, sort.key);
      }
    };
    // Empty values last, whichever way.
    return [...found].sort((a, b) => {
      const [x, y] = [value(a), value(b)];
      if (x === '' || y === '') return x === y ? 0 : x === '' ? 1 : -1;
      const c = numeric ? Number(x.replace(',', '.')) - Number(y.replace(',', '.')) : collator.compare(x, y);
      return (sort.desc ? -c : c) || a.position - b.position;
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [list, query, sort]);
  // Moved by hand only in the start order, everything shown; a place can be typed in any view.
  const sortable = editable && !team && !viewed;
  const placeable = editable && !team;
  // A column: up, down, back to the start order. « # »: back to the start order first (from any
  // other sort), then the start order reversed, then the start order again.
  const cycle = (key: string) =>
    setSort(s => {
      if (key === 'place') return s === null ? { key, desc: true } : null;
      return s?.key !== key ? { key, desc: false } : !s.desc ? { key, desc: true } : null;
    });
  const head = (key: string, label: ReactNode, className = '', title?: string) => {
    // Right-aligned (numbers): the arrow before the label, so the label stays over its numbers.
    const right = className.includes('text-right');
    const arrow =
      key === 'place' && sort === null ? (
        <ArrowUp className="size-3.5 shrink-0 text-ink-4" />
      ) : sort?.key === key ? (
        sort.desc ? (
          <ArrowDown className="size-3.5 shrink-0 text-accent" />
        ) : (
          <ArrowUp className="size-3.5 shrink-0 text-accent" />
        )
      ) : (
        <ArrowUpDown className="size-3 shrink-0 opacity-0 transition-opacity group-hover/head:opacity-60" />
      );
    return (
      <th className={`${th} ${className}`} title={title} aria-sort={sort?.key === key ? (sort.desc ? 'descending' : 'ascending') : undefined}>
        <button onClick={() => cycle(key)} className="group/head inline-flex max-w-full items-center gap-1 rounded hover:text-ink">
          {right && arrow}
          <span className="truncate">{label}</span>
          {!right && arrow}
        </button>
      </th>
    );
  };
  // Remarks alone keep the table as it is (no sideways scrolling); on a phone they go under the
  // rider. Any other column widens it.
  const remarks = extras.some(c => c.key === 'remarks');
  const wide = extras.some(c => c.key !== 'remarks');
  const columns = baseColumns + extras.length - (phone && remarks ? 1 : 0);
  const sortedBy =
    sort &&
    (sort.key === 'place'
      ? '#'
      : sort.key === 'number'
        ? t('start_no')
        : sort.key === 'time'
          ? t('time')
          : sort.key === 'rider'
            ? t('rider')
            : sort.key === 'horse'
              ? t('horse')
              : sort.key === 'carried'
                ? t('carried')
                : t(EXTRA.find(c => c.key === sort.key)?.label ?? 'rider'));

  // An entry shown from elsewhere (a rider's dialog, the participants): marked and brought to the
  // middle, once its list is there (a dialog opening, the page laid out: a moment later).
  useEffect(() => {
    setSelected(highlight);
    if (highlight === undefined) return;
    const later = setTimeout(() => box.current?.querySelector(`#entry-${highlight}`)?.scrollIntoView({ block: 'center' }), 50);
    return () => clearTimeout(later);
  }, [highlight, classId]);

  const move = (entry: number, to: number) => change('move_start', { class: classId, entry, to });
  // A place typed (a click on the number, or a digit on the selected row): the starter goes there.
  const [placing, setPlacing] = useState<{ entry: number; text: string; select: boolean } | null>(null);
  const cancelled = useRef(false);
  const place = (entry: number, text: string) => {
    setPlacing(null);
    box.current?.focus();
    const n = Number(text.trim());
    const from = starters.findIndex(s => s.entry === entry) + 1;
    if (!Number.isInteger(n) || n < 1 || text.trim() === '') return;
    const to = Math.min(n, starters.length);
    if (to !== from) move(entry, to);
  };
  const toggle = (entry: number, field: 'not_starting' | 'hors_concours', value: boolean) => change('edit_entry', { entry, [field]: value });

  const onKey = (e: React.KeyboardEvent) => {
    // Typing in a field, or moving something by its grip (the keys are the drag's).
    if (selected === undefined || overlayOpen() || e.target instanceof HTMLInputElement || e.target instanceof HTMLButtonElement) return;
    const order = viewed ? shown : starters;
    const i = order.findIndex(s => s.entry === selected);
    const line = order[i];
    if (!line) return;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const j = i + (e.key === 'ArrowDown' ? 1 : -1);
      if (j < 0 || j >= order.length) return;
      if (e.altKey && sortable) move(line.entry, j + 1);
      else setSelected(order[j].entry);
    } else if (editable && (e.key === 'n' || e.key === 'N')) {
      toggle(line.entry, 'not_starting', !line.not_starting);
    } else if (editable && (e.key === 'h' || e.key === 'H')) {
      toggle(line.entry, 'hors_concours', !line.hors_concours);
    } else if (editable && !team && /^[0-9]$/.test(e.key) && !e.metaKey && !e.ctrlKey && !e.altKey) {
      e.preventDefault();
      setPlacing({ entry: line.entry, text: e.key, select: false });
    }
  };

  useEffect(() => {
    if (selected !== undefined) box.current?.querySelector(`#entry-${selected}`)?.scrollIntoView({ block: 'nearest' });
  }, [selected]);

  // Escape lets the selected row go first (the page's Escape, back to the classes, comes next):
  // wherever the focus is, but not in a field, under a dialog or during a drag (Escape cancels it).
  useEffect(() => {
    if (selected === undefined) return;
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || overlayOpen() || (e.target as HTMLElement).closest('input, textarea, select, [contenteditable]')) return;
      if (document.querySelector('[aria-roledescription][aria-pressed="true"]')) return;
      e.preventDefault();
      setSelected(undefined);
    };
    window.addEventListener('keydown', key, true);
    return () => window.removeEventListener('keydown', key, true);
  }, [selected]);

  return (
    <div ref={box} tabIndex={0} onKeyDown={onKey} className="outline-none">
      {(!editable || list.published_by_name || (editable && team && teams.length > 1)) && (
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-line px-4 py-2.5 text-xs text-ink-3">
          {!editable && (
            <span className="inline-flex items-center gap-1.5 font-semibold text-ok">
              <Lock className="size-3.5" />
              {t('final_locked')}
            </span>
          )}
          {list.published_by_name && (
            <span className="inline-flex items-center gap-1.5">
              <Info className="size-3.5" />
              {t('published_by_name')}
            </span>
          )}
          {editable && team && teams.length > 1 && (
            <Button size="sm" icon={Users} onClick={() => setTeamOrder(true)} className="ml-auto">
              {t('team_order')}
            </Button>
          )}
        </div>
      )}
      {/* Searching and the columns: what is shown, never the order. */}
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-line px-3 py-2 sm:px-4">
        <div className="relative min-w-0 flex-1 basis-48 sm:max-w-xs">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-ink-4" />
          <input
            value={query}
            onChange={e => setQuery(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Escape' && query) {
                e.preventDefault();
                e.stopPropagation();
                setQuery('');
              }
            }}
            placeholder={t('list_search')}
            aria-label={t('list_search')}
            className="field field-sm pr-8 pl-8"
          />
          {query && (
            <button
              onClick={() => setQuery('')}
              aria-label={t('close')}
              className="absolute top-1/2 right-1.5 grid size-6 -translate-y-1/2 place-items-center rounded text-ink-4 hover:bg-hover hover:text-ink"
            >
              <X className="size-3.5" />
            </button>
          )}
        </div>
        {words.length > 0 && <span className="text-xs text-ink-3 tabular">{t('shown_of').replace('{n}', String(shown.length)).replace('{m}', String(starters.length))}</span>}
        {sortedBy && (
          <span className="inline-flex min-w-0 items-center gap-1.5 text-xs text-ink-3">
            <ArrowUpDown className="size-3.5 shrink-0" />
            <span className="truncate">{t('sorted_by').replace('{c}', sortedBy)}</span>
            <Button size="sm" variant="ghost" onClick={() => setSort(null)} className="h-6 px-1.5 text-xs">
              {t('start_order')}
            </Button>
          </span>
        )}
        {available.length > 0 && (
          <span className="ml-auto">
            <Menu
              width="w-72"
              trigger={
                <Button size="sm" variant="ghost" icon={Columns3}>
                  {t('columns')}
                  {extras.length > 0 && <span className="ml-0.5 rounded-full bg-accent-soft px-1.5 text-[11px] font-bold text-accent tabular">{extras.length}</span>}
                </Button>
              }
            >
              {GROUPS.filter(g => available.some(c => c.group === g)).map((g, i) => (
                <div key={g}>
                  {i > 0 && <MenuSeparator />}
                  <MenuLabel>{t(g)}</MenuLabel>
                  {available
                    .filter(c => c.group === g)
                    .map(c => (
                      <MenuCheck key={c.key} checked={chosen.includes(c.key)} onChange={on => setChosen(on ? [...chosen, c.key] : chosen.filter(k => k !== c.key))}>
                        {t(c.label)}
                      </MenuCheck>
                    ))}
                </div>
              ))}
              {(chosen.length !== DEFAULT_COLUMNS.length || chosen.some(k => !DEFAULT_COLUMNS.includes(k))) && (
                <>
                  <MenuSeparator />
                  <MenuItem icon={RotateCcw} onSelect={() => setChosen(null)}>
                    {t('columns_default')}
                  </MenuItem>
                </>
              )}
            </Menu>
          </span>
        )}
      </div>
      <SortableList ids={sortable ? starters.map(s => String(s.entry)) : []} onMove={(entry, to) => move(Number(entry), to + 1)}>
        <div className={wide ? 'overflow-x-auto' : ''}>
          <table className="w-full table-fixed text-[13px]" style={wide ? { minWidth: 720 + extras.length * 128 } : undefined}>
            <thead>
              <tr>
                {head('place', '#', `${placeable ? 'w-[4.5rem]' : 'w-12'} text-right`)}
                {list.start_numbers && head('number', t('start_no'), 'hidden w-14 text-right sm:table-cell')}
                {list.start_times && head('time', t('time'), 'hidden w-16 sm:table-cell')}
                {head('rider', t('rider'))}
                {head('horse', t('horse'), `hidden sm:table-cell ${wide ? 'w-52' : remarks ? 'w-[26%]' : 'w-[34%]'}`)}
                {list.qualifier && head('carried', t('carried'), 'w-16 text-right', `${t('carried_hint')} (${list.qualifier})`)}
                {extras.map(c =>
                  c.key === 'remarks'
                    ? head(c.key, t(c.label), `hidden sm:table-cell ${wide ? 'w-64' : 'w-[26%]'}`, t('remarks_legend'))
                    : head(c.key, t(c.label), `w-32 ${c.numeric ? 'text-right' : ''}`),
                )}
                <th className={`${th} ${sections.length ? 'w-32 sm:w-56' : 'w-24 sm:w-32'}`} />
              </tr>
            </thead>
            <tbody className="tabular">
              {(viewed ? shown : list.lines).map((line, i) => {
                if (line.kind === 'team' && editable && renaming === line.team) {
                  return (
                    <tr key={i}>
                      <td colSpan={columns} className="border-t border-line px-3 pt-4 pb-2">
                        <input
                          autoFocus
                          defaultValue={line.name}
                          aria-label={t('team_name')}
                          onBlur={() => setRenaming(null)}
                          onKeyDown={async e => {
                            if (e.key === 'Escape') setRenaming(null);
                            if (e.key !== 'Enter') return;
                            const name = e.currentTarget.value.trim();
                            if (name && name !== line.name) await change('rename_team', { team: line.team, name });
                            setRenaming(null);
                          }}
                          className="field field-sm max-w-sm font-semibold"
                        />
                      </td>
                    </tr>
                  );
                }
                if (line.kind === 'team') {
                  return (
                    <tr key={i} className="group">
                      <td colSpan={columns} className="border-t border-line bg-raised px-3 pt-3 pb-2">
                        <div className="flex flex-wrap items-center gap-2">
                          <Users className="size-4 text-ink-4" />
                          <span className={`text-[13px] font-bold ${line.not_starting ? 'text-ink-3 line-through' : ''}`}>{line.name}</span>
                          {line.time && <span className="text-xs text-ink-3">{line.time}</span>}
                          {line.not_starting && <Pill>{t('not_starting')}</Pill>}
                          {editable && (
                            <span className="ml-auto inline-flex gap-1">
                              <Tooltip label={t('rename')}>
                                <button onClick={() => setRenaming(line.team)} aria-label={t('rename')} className={`${chip} ${quiet} grid place-items-center`}>
                                  <Pencil className="size-3.5" />
                                </button>
                              </Tooltip>
                              <button
                                onClick={() => change('team_not_starting', { team: line.team, not_starting: !line.not_starting })}
                                title={line.not_starting ? t('starts_again') : t('team_not_starting')}
                                className={`${chip} ${line.not_starting ? 'bg-ink-2 text-card' : quiet}`}
                              >
                                {t('not_starting')}
                              </button>
                            </span>
                          )}
                        </div>
                      </td>
                    </tr>
                  );
                }
                if (line.kind !== 'starter') {
                  // A pause, a block's title, a level, a line of text.
                  const text = line.pause ? `${t('pause')} ${line.pause[0]} – ${line.pause[1]}` : line.level ? `${t('level')} ${line.level}` : line.text;
                  return (
                    <tr key={i}>
                      <td colSpan={columns} className={`border-t border-line px-4 text-xs ${line.pause ? 'bg-sunken py-1.5 text-ink-3 italic' : 'bg-raised pt-3 pb-2 font-bold text-ink-2'}`}>
                        {text}
                      </td>
                    </tr>
                  );
                }
                const struck = line.not_starting ? 'text-ink-3 line-through' : '';
                const isSelected = line.entry === selected;
                const horses = byRider.get(line.person) ?? [];
                return (
                  <SortRow key={i} id={String(line.entry)} disabled={!sortable}>
                    {row => (
                      <tr
                        id={`entry-${line.entry}`}
                        ref={row.ref}
                        style={row.style}
                        onClick={() => setSelected(line.entry)}
                        className={`group border-t border-line transition-colors ${isSelected ? 'bg-accent-soft shadow-[inset_3px_0_0_var(--accent)]' : 'hover:bg-hover'} ${row.dragging ? 'relative z-10 bg-card shadow-pop' : ''}`}
                      >
                        <td className={`${td} text-right text-ink-3`}>
                          <span className="inline-flex items-center gap-1">
                            <Grip grip={row.grip} label={t('drag_to_move')} disabled={!sortable} />
                            {placeable && placing?.entry === line.entry ? (
                              <input
                                autoFocus
                                value={placing.text}
                                inputMode="numeric"
                                aria-label={t('new_place')}
                                onFocus={e => (placing.select ? e.currentTarget.select() : e.currentTarget.setSelectionRange(placing.text.length, placing.text.length))}
                                onChange={e => setPlacing({ ...placing, text: e.target.value.replace(/[^0-9]/g, '') })}
                                onClick={e => e.stopPropagation()}
                                onKeyDown={e => {
                                  if (e.key === 'Enter') {
                                    e.preventDefault();
                                    cancelled.current = true;
                                    place(line.entry, placing.text);
                                  } else if (e.key === 'Escape') {
                                    e.preventDefault();
                                    e.stopPropagation();
                                    cancelled.current = true;
                                    setPlacing(null);
                                    box.current?.focus();
                                  }
                                }}
                                onBlur={() => {
                                  if (cancelled.current) cancelled.current = false;
                                  else place(line.entry, placing.text);
                                }}
                                className="h-7 w-11 rounded-md border border-accent bg-card px-1.5 text-right font-semibold text-ink tabular ring-2 ring-accent/25 outline-none"
                              />
                            ) : placeable ? (
                              <Tooltip label={t('type_place')}>
                                <button
                                  onClick={e => {
                                    e.stopPropagation();
                                    setSelected(line.entry);
                                    setPlacing({ entry: line.entry, text: String(line.position), select: true });
                                  }}
                                  className="min-w-7 rounded-md px-1 py-0.5 text-right tabular hover:bg-card hover:text-ink hover:ring-1 hover:ring-line-strong pointer-coarse:min-h-9"
                                >
                                  {line.position}
                                </button>
                              </Tooltip>
                            ) : (
                              line.position
                            )}
                          </span>
                        </td>
                        {list.start_numbers && <td className={`${td} hidden text-right font-bold text-ink sm:table-cell`}>{line.start_number ?? ''}</td>}
                        {list.start_times && <td className={`${td} hidden font-medium text-ink-2 sm:table-cell`}>{line.time ?? '--:--'}</td>}
                        <td className={`${td} max-w-0`}>
                          <div className="flex min-w-0 items-center gap-1.5">
                            {list.start_numbers && line.start_number !== null && <span className="shrink-0 font-bold text-ink sm:hidden">{line.start_number}</span>}
                            <button
                              type="button"
                              onClick={e => {
                                e.stopPropagation();
                                setRiderOf(line.entry);
                              }}
                              className={`min-w-0 truncate text-left font-semibold text-ink underline-offset-2 hover:text-accent hover:underline focus-visible:underline focus-visible:outline-none ${struck}`}
                            >
                              {line.rider}
                            </button>
                            {horses.length > 1 && (
                              <Tooltip label={`${horses.length} ${t('horses_of_rider')} · ${t('min_gap')} ${gapOf(line.person)}${editable && !team ? ` · ${t('horses_rearrange')}` : ''}`}>
                                <button
                                  onClick={e => {
                                    e.stopPropagation();
                                    if (editable && !team) setHorsesOf(line.person);
                                  }}
                                  className={`shrink-0 rounded-md px-1.5 py-px text-[10px] font-bold ring-1 ring-inset transition-colors ${tooClose(line.person) ? 'bg-accent text-on-accent ring-accent' : 'bg-sunken text-ink-2 ring-line hover:ring-accent'}`}
                                >
                                  {horses.indexOf(line) + 1}/{horses.length}
                                </button>
                              </Tooltip>
                            )}
                          </div>
                          <div className={`truncate text-xs text-ink-3 ${struck}`}>
                            {list.start_times && <span className="font-semibold text-ink-2 sm:hidden">{line.time ?? '--:--'} · </span>}
                            {[line.club, line.town].filter(Boolean).join(' · ')}
                          </div>
                          <div className={`truncate text-xs font-medium sm:hidden ${struck}`}>{line.horse}</div>
                          {remarks && (
                            <div className="mt-1 sm:hidden">
                              <Remarks line={line} />
                            </div>
                          )}
                        </td>
                        <td className={`${td} hidden max-w-0 truncate font-medium sm:table-cell ${struck}`}>{line.horse}</td>
                        {list.qualifier && <td className={`${td} text-right font-semibold`}>{line.carried_out ? '–' : (line.carried ?? '')}</td>}
                        {extras.map(c =>
                          c.key === 'remarks' ? (
                            <td key={c.key} className={`${td} hidden max-w-0 sm:table-cell`}>
                              <Remarks line={line} />
                            </td>
                          ) : (
                            <td key={c.key} className={`${td} max-w-0 truncate text-ink-2 ${c.numeric ? 'text-right' : ''}`} title={valueOf(line, c.key)}>
                              {c.key === 'status' ? statusText(valueOf(line, c.key)) : valueOf(line, c.key)}
                            </td>
                          ),
                        )}
                        <td className={`${td} text-right`}>
                          <span className="inline-flex flex-wrap justify-end gap-1">
                            {editable ? (
                              <>
                                <button
                                  onClick={e => (e.stopPropagation(), toggle(line.entry, 'not_starting', !line.not_starting))}
                                  title={line.not_starting ? t('starts_again') : t('not_starting_long')}
                                  className={`${chip} ${line.not_starting ? 'bg-ink-2 text-card' : quiet}`}
                                >
                                  {t('not_starting')}
                                </button>
                                <button
                                  onClick={e => (e.stopPropagation(), toggle(line.entry, 'hors_concours', !line.hors_concours))}
                                  title={t('hors_concours')}
                                  className={`${chip} ${line.hors_concours ? 'bg-warn text-white' : quiet}`}
                                >
                                  {t('hc')}
                                </button>
                                {sections.map(s => (
                                  <button
                                    key={s.id}
                                    onClick={e => (e.stopPropagation(), change('move_entry', { entry: line.entry, from: classId, to: s.id }))}
                                    title={`${t('move_to')} ${s.number}`}
                                    className={`${chip} ${quiet} inline-flex items-center gap-0.5`}
                                  >
                                    <ArrowRight className="size-3" />
                                    {s.number}
                                  </button>
                                ))}
                              </>
                            ) : (
                              <>
                                {line.not_starting && <Pill>{t('not_starting')}</Pill>}
                                {line.hors_concours && <Pill tone="warn">{t('hc')}</Pill>}
                              </>
                            )}
                          </span>
                        </td>
                      </tr>
                    )}
                  </SortRow>
                );
              })}
            </tbody>
          </table>
        </div>
      </SortableList>
      <RiderDialog show={show} entry={riderOf} onClose={() => setRiderOf(null)} />
      <RiderHorses list={list} person={horsesOf} horses={horsesOf === null ? [] : (byRider.get(horsesOf) ?? [])} onClose={() => setHorsesOf(null)} />
      <Modal
        open={teamOrder}
        onClose={() => setTeamOrder(false)}
        title={t('team_order')}
        description={t('team_order_hint')}
        icon={Users}
        size="sm"
        footer={<Button onClick={() => setTeamOrder(false)}>{t('close')}</Button>}
      >
        <SortableList ids={teams.map(x => x.team)} onMove={(team, to) => change('move_team', { class: classId, team, to })}>
          <ol className="grid gap-1.5">
            {teams.map((x, k) => (
              <SortRow key={x.team} id={x.team}>
                {row => (
                  <li
                    ref={row.ref}
                    style={row.style}
                    className={`flex items-center gap-2 rounded-xl border border-line bg-card px-2 py-1.5 text-[13px] ${row.dragging ? 'shadow-pop ring-1 ring-accent' : ''}`}
                  >
                    <Grip grip={row.grip} label={t('drag_to_move')} />
                    <span className="w-6 text-right text-xs text-ink-3 tabular">{k + 1}.</span>
                    <span className={`min-w-0 flex-1 truncate font-semibold ${x.not_starting ? 'text-ink-3 line-through' : ''}`}>{x.name}</span>
                  </li>
                )}
              </SortRow>
            ))}
          </ol>
        </SortableList>
      </Modal>
    </div>
  );
}

/** A rider's horses (U12), swapped among the places they hold; the numbers follow if asked. */
function RiderHorses({ list, person, horses, onClose }: { list: StartListView; person: number | null; horses: Starter[]; onClose: () => void }) {
  const t = useT();
  const { change } = useChanges();
  const [follow, setFollow] = useState(() => {
    try {
      return localStorage.getItem('numbers-follow') === '1';
    } catch {
      return false;
    }
  });
  const rearrange = (entry: number, to: number) => {
    const order = horses.map(l => l.entry).filter(e => e !== entry);
    order.splice(to, 0, entry);
    change('rearrange_rider', { class: list.class.id, entries: order, numbers_follow: follow });
  };
  return (
    <Modal
      open={person !== null && horses.length > 1}
      onClose={onClose}
      title={`${t('horses_of')} ${horses[0]?.rider ?? ''}`}
      description={t('horses_rearrange_hint')}
      icon={Users}
      size="md"
      footer={<Button onClick={onClose}>{t('close')}</Button>}
    >
      <div className="grid gap-4">
        <SortableList ids={horses.map(h => String(h.entry))} onMove={(entry, to) => rearrange(Number(entry), to)}>
          <ol className="grid gap-1.5">
            {horses.map(h => (
              <SortRow key={h.entry} id={String(h.entry)}>
                {r => (
                  <li
                    ref={r.ref}
                    style={r.style}
                    className={`flex items-center gap-3 rounded-xl border border-line bg-card px-2 py-2 text-[13px] ${r.dragging ? 'shadow-pop ring-1 ring-accent' : ''}`}
                  >
                    <Grip grip={r.grip} label={t('drag_to_move')} />
                    <span className="w-10 text-right text-xs text-ink-3 tabular">{h.position}.</span>
                    {list.start_numbers && <span className="w-10 text-right font-bold tabular">{h.start_number ?? ''}</span>}
                    <span className={`min-w-0 flex-1 truncate font-semibold ${h.not_starting ? 'text-ink-3 line-through' : ''}`}>{h.horse}</span>
                  </li>
                )}
              </SortRow>
            ))}
          </ol>
        </SortableList>
        <Switch
          checked={follow}
          onChange={v => {
            setFollow(v);
            try {
              localStorage.setItem('numbers-follow', v ? '1' : '0');
            } catch {
              /* not kept */
            }
          }}
          label={t('numbers_follow')}
          hint={t('numbers_follow_hint')}
        />
      </div>
    </Modal>
  );
}

/** Entries of the class on none of its start lists (delivered by Swiss Equestrian, or entered,
 *  after the class was set up): each goes at the end of a list, with the next start number. */
export function Unplaced({ list, sections = [] }: { list: StartListView; sections?: ClassRow[] }) {
  const t = useT();
  const { change } = useChanges();
  if (list.unplaced.length === 0) return null;
  const editable = list.class.status !== 'final';
  const targets = [list.class, ...sections];
  return (
    <Callout tone="warn" title={`${list.unplaced.length} ${t('unplaced')}`}>
      <p className="mb-2 text-xs">{list.class.max_starters ? t('unplaced_max').replace('{n}', String(list.class.max_starters)) : t('unplaced_hint')}</p>
      <ul className="grid gap-1.5">
        {list.unplaced.map(u => (
          <li key={u.entry} className="flex flex-wrap items-center gap-2">
            <span className="min-w-0 flex-1 truncate">
              <span className="font-semibold text-ink">{u.rider}</span> <span className="text-ink-3">· {u.horse}</span>
            </span>
            {editable &&
              targets.map(c => (
                <Button key={c.id} size="sm" onClick={() => change('place_entry', { entry: u.entry, class: c.id })}>
                  + {targets.length > 1 ? c.number : t('to_the_list')}
                </Button>
              ))}
          </li>
        ))}
      </ul>
    </Callout>
  );
}
