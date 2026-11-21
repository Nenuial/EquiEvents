// Entries made and changed at the show (U2), in dialogs: a late entry; an entry with its tabs
// (the entry: does not start, hors concours, class, level, note; its horse; its rider; the
// rider's contacts). Riders and horses come from the show or from Swiss Equestrian's registry,
// never typed in, and the office never changes their data (a rider's address: read only).

import { useEffect, useState } from 'react';
import {
  ArrowRight,
  ArrowRightLeft,
  ClipboardList,
  Contact,
  Ellipsis,
  Eye,
  Layers,
  Lock,
  Mail,
  MapPin,
  MessageSquarePlus,
  MessageSquareText,
  NotebookPen,
  Phone,
  Search,
  Trash2,
  UserRound,
  X,
} from 'lucide-react';
import { call, type EntryRow, type HorseFound, type PhaseKind, type PublishedClass, type RegistryAbout, type RiderFound } from '../api';
import { href } from '../App';
import { formatDayShort, useLanguage, usePlural, useT } from '../i18n';
import { Button, CheckRow, ConfirmModal, Field, IconButton, Menu, MenuItem, MenuSeparator, Modal, Pill, Tabs } from '../ui';
import { Horseshoe } from '../ui/icons';
import { useChanges } from './changes';
import { useLoad } from './common';
import { phaseName } from './overall';

/** A rider picked: `guest`, they hold a guest licence (a guest horse is theirs to ride). */
type RiderPick = { person: number; label: string; guest: boolean };
type HorsePick = { horse: number; label: string } | { guest: string };

/** Swiss Equestrian's registry on this computer: what it holds and from when. It is downloaded
 *  and updated on the Swiss Equestrian page, not here. */
export function RegistryNote({ about }: { about?: RegistryAbout }) {
  const t = useT();
  const language = useLanguage();
  if (!about) return null;
  const empty = about.persons === 0 && about.horses === 0;
  const at = about.persons_at ?? about.horses_at;
  const counts = t('registry_counts')
    .replace('{p}', String(about.persons))
    .replace('{h}', String(about.horses))
    .replace('{d}', at ? new Date(at).toLocaleDateString(`${language}-CH`) : '–');
  return (
    <div className="text-[11px] text-ink-3">
      {t('registry')} : {empty ? t('registry_none') : counts}
      {!empty && about.simulated && ` (${t('registry_simulated')})`}
    </div>
  );
}

/** Riders matching a search: the show's first, then the registry's. */
function useRiders(show: string, query: string) {
  const [answer, setAnswer] = useState<{ found: RiderFound[]; registry: RegistryAbout } | null>(null);
  const [again, setAgain] = useState(0);
  useEffect(() => {
    let live = true;
    call<{ found: RiderFound[]; registry: RegistryAbout }>('find_riders', { show, query }).then(a => live && setAnswer(a));
    return () => {
      live = false;
    };
  }, [show, query, again]);
  return { found: query.trim() ? (answer?.found ?? []) : [], registry: answer?.registry, again: () => setAgain(n => n + 1) };
}

/** Horses matching a search: the show's, then the registry's; `entry`: whose rider may have a guest horse. */
export function useHorses(show: string, query: string, entry?: number) {
  type Answer = { found: HorseFound[]; guest: boolean; year: number | null; registry: RegistryAbout };
  const [answer, setAnswer] = useState<Answer | null>(null);
  const [again, setAgain] = useState(0);
  useEffect(() => {
    let live = true;
    call<Answer>('find_horses', { show, query, entry }).then(a => live && setAnswer(a));
    return () => {
      live = false;
    };
  }, [show, query, entry, again]);
  return { found: query.trim() ? (answer?.found ?? []) : [], guest: answer?.guest ?? false, year: answer?.year ?? null, registry: answer?.registry, again: () => setAgain(n => n + 1) };
}

/** A tag on what is not the show's yet: it comes from the registry. */
function FromRegistry() {
  const t = useT();
  return <Pill tone="info">{t('from_registry')}</Pill>;
}

/** A search field, with the results as a list under it. */
function SearchBox({ value, onChange, placeholder }: { value: string; onChange: (v: string) => void; placeholder: string }) {
  return (
    <div className="relative">
      <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-ink-4" />
      <input data-autofocus value={value} onChange={e => onChange(e.target.value)} placeholder={placeholder} className="field pl-9" />
    </div>
  );
}

const resultRow = 'flex w-full items-center gap-3 border-t border-line px-3 py-2.5 text-left first:border-t-0 hover:bg-hover disabled:cursor-not-allowed disabled:opacity-50 pointer-coarse:py-3';

/** A rider: searched among the show's and Swiss Equestrian's registry. */
function RiderSearch({ show, onPick, note = true }: { show: string; onPick: (r: RiderPick) => void; note?: boolean }) {
  const t = useT();
  const [query, setQuery] = useState('');
  const riders = useRiders(show, query);
  return (
    <div className="grid gap-2">
      <SearchBox value={query} onChange={setQuery} placeholder={t('search_rider')} />
      {riders.found.length > 0 && (
        <div className="max-h-64 overflow-auto rounded-xl border border-line">
          {riders.found.map(f => (
            <button key={f.person} onClick={() => (onPick({ person: f.person, label: f.name, guest: f.guest }), setQuery(''))} className={resultRow}>
              <UserRound className="size-4 shrink-0 text-ink-4" />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[13px] font-semibold">{f.name}</span>
                <span className="block truncate text-xs text-ink-3">{[f.person, f.town, f.club].filter(Boolean).join(' · ')}</span>
              </span>
              {!f.in_show && <FromRegistry />}
            </button>
          ))}
        </div>
      )}
      {note && <RegistryNote about={riders.registry} />}
    </div>
  );
}

/** A horse: searched among the show's and the registry's (those Swiss Equestrian lets start this
 *  year), or a guest horse for a rider with a guest licence (`guest`). */
function HorseSearch({ show, entry, guestAllowed, onPick }: { show: string; entry?: number; guestAllowed?: boolean; onPick: (h: HorsePick) => void }) {
  const t = useT();
  const [query, setQuery] = useState('');
  const horses = useHorses(show, query, entry);
  const guest = guestAllowed ?? horses.guest;
  return (
    <div className="grid gap-2">
      <SearchBox value={query} onChange={setQuery} placeholder={t('search_horse')} />
      {(horses.found.length > 0 || (guest && query.trim())) && (
        <div className="max-h-64 overflow-auto rounded-xl border border-line">
          {horses.found.map(h => (
            <button key={h.horse} disabled={!h.paid} onClick={() => (onPick({ horse: h.horse, label: h.name }), setQuery(''))} className={resultRow}>
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[13px] font-semibold">{h.name}</span>
                <span className="block truncate text-xs text-ink-3">{[h.horse, h.detail].filter(Boolean).join(' · ')}</span>
              </span>
              {!h.paid && <Pill tone="warn">{t('not_paid').replace('{y}', String(horses.year ?? ''))}</Pill>}
              {!h.in_show && <FromRegistry />}
            </button>
          ))}
          {guest && query.trim() && (
            <button onClick={() => (onPick({ guest: query.trim().toUpperCase() }), setQuery(''))} className={resultRow}>
              <span className="min-w-0 flex-1 text-[13px]">
                <span className="font-semibold">{t('guest_as')}</span> {query.trim().toUpperCase()}
              </span>
              <Pill tone="accent">{t('guest_horse_label')}</Pill>
            </button>
          )}
        </div>
      )}
      <RegistryNote about={horses.registry} />
    </div>
  );
}

/** A late entry (an organiser's place): a rider, a horse, and the classes (several at once, one
 *  change; a team class alone, with its team); `done` with the first new entry. */
export function LateEntryModal({ show, open, onClose, done }: { show: string; open: boolean; onClose: () => void; done: (entry: number) => void }) {
  const t = useT();
  const language = useLanguage();
  const { change } = useChanges();
  const published = useLoad(() => (open ? call<PublishedClass[]>('published', { show }) : Promise.resolve(null)), [show, open]);
  const [classes, setClasses] = useState<number[]>([]);
  const [classQuery, setClassQuery] = useState('');
  const [rider, setRider] = useState<RiderPick | null>(null);
  const [horse, setHorse] = useState<HorsePick | null>(null);
  const list = [...(published.data ?? [])].sort((a, b) => compare(a.date, b.date) || a.number.localeCompare(b.number, undefined, { numeric: true }));
  // Team classes: a team with a free place, or a new one ('' until chosen; 'new' with a name).
  const teamClass = classes.length === 1 && (list.find(p => p.id === classes[0])?.teams ?? false);
  const teams = useLoad(
    () => (teamClass ? call<{ size: number | null; teams: { id: string; name: string; riders: number }[] }>('teams', { show, published: classes[0] }) : Promise.resolve(null)),
    [show, classes, teamClass],
  );
  const [team, setTeam] = useState('');
  const [teamName, setTeamName] = useState('');
  const free = (teams.data?.teams ?? []).filter(x => teams.data?.size == null || x.riders < teams.data.size);
  const teamReady = !teamClass || (team === 'new' ? teamName.trim() !== '' : team !== '');
  // The classes matching the search (not those picked): its number first, then by day.
  const words = classQuery.toLowerCase().split(/\s+/).filter(Boolean);
  const exact = classQuery.trim().toLowerCase();
  const found = words.length
    ? list
        .filter(p => !classes.includes(p.id) && words.every(w => `${p.number} ${p.discipline} ${p.category} ${p.date ? formatDayShort(p.date, language) : ''}`.toLowerCase().includes(w)))
        .sort(
          (a, b) =>
            Number(b.number.toLowerCase() === exact) - Number(a.number.toLowerCase() === exact) || Number(b.number.toLowerCase().startsWith(exact)) - Number(a.number.toLowerCase().startsWith(exact)),
        )
        .slice(0, 8)
    : [];
  const ready = classes.length > 0 && !!rider && !!horse && ('horse' in horse || horse.guest.trim() !== '') && teamReady;
  // Ticking: a team class goes alone; ticking another class unticks it.
  const tick = (p: PublishedClass, on: boolean) => {
    setTeam('');
    setClasses(before => {
      if (!on) return before.filter(id => id !== p.id);
      if (p.teams) return [p.id];
      return [...before.filter(id => !list.find(x => x.id === id)?.teams), p.id];
    });
  };
  const reset = () => {
    setRider(null);
    setHorse(null);
    setClasses([]);
    setClassQuery('');
    setTeam('');
    setTeamName('');
  };
  const enter = async () => {
    if (!ready || !rider || !horse) return;
    const answer = await change('add_entry', {
      published: classes,
      rider: { person: rider.person },
      horse: 'horse' in horse ? { horse: horse.horse } : { guest: horse.guest.trim().toUpperCase() },
      ...(teamClass ? (team === 'new' ? { new_team: teamName.trim() } : { team }) : {}),
    });
    if (answer) {
      reset();
      done(Number(answer.entry));
    }
  };
  const picked = (label: string, clear: () => void) => (
    <div className="flex items-center justify-between gap-3 rounded-xl border border-accent/40 bg-accent-soft px-3 py-2.5">
      <span className="min-w-0 truncate text-[13px] font-semibold">{label}</span>
      <Button size="sm" variant="ghost" onClick={clear}>
        {t('change')}
      </Button>
    </div>
  );
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t('late_entry_title')}
      icon={ClipboardList}
      tone="accent"
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button variant="primary" disabled={!ready} onClick={enter}>
            {classes.length > 1 ? t('enter_in_many').replace('{n}', String(classes.length)) : t('enter')}
          </Button>
        </>
      }
    >
      <div className="grid gap-5">
        <Field label={t('rider')}>{rider ? picked(rider.label, () => (setRider(null), setHorse(null))) : <RiderSearch show={show} onPick={setRider} note={false} />}</Field>
        <Field label={t('horse')}>
          {horse ? (
            picked('horse' in horse ? horse.label : `${t('guest_as')} ${horse.guest}`, () => setHorse(null))
          ) : (
            <HorseSearch show={show} guestAllowed={rider?.guest ?? false} onPick={setHorse} />
          )}
        </Field>
        {/* The classes: searched (a show can have fifty), the ones picked as chips. */}
        <Field label={t('late_entry_classes')} hint={t('late_entry_classes_hint')}>
          <div className="grid gap-2">
            {classes.length > 0 && (
              <div className="flex flex-wrap gap-1.5">
                {classes.map(id => {
                  const p = list.find(x => x.id === id);
                  if (!p) return null;
                  return (
                    <span key={id} className="inline-flex items-center gap-1.5 rounded-lg bg-accent-soft py-1 pr-1 pl-2 text-xs text-accent ring-1 ring-accent/30">
                      <b className="tabular">{p.number}</b>
                      <span className="max-w-40 truncate">{p.category}</span>
                      <button onClick={() => tick(p, false)} aria-label={t('remove')} className="grid size-5 place-items-center rounded hover:bg-accent/15 pointer-coarse:size-7">
                        <X className="size-3.5" />
                      </button>
                    </span>
                  );
                })}
              </div>
            )}
            <div className="relative">
              <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-ink-4" />
              <input
                value={classQuery}
                onChange={e => setClassQuery(e.target.value)}
                onKeyDown={e => {
                  if (e.key === 'Enter' && found[0]) {
                    e.preventDefault();
                    tick(found[0], true);
                    setClassQuery('');
                  }
                }}
                placeholder={t('search_class')}
                className="field pl-9"
              />
            </div>
            {found.length > 0 && (
              <div className="max-h-64 overflow-auto rounded-xl border border-line">
                {found.map(p => (
                  <button key={p.id} onClick={() => (tick(p, true), setClassQuery(''))} className={resultRow}>
                    <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-sunken text-[13px] font-bold text-ink ring-1 ring-line tabular">{p.number}</span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[13px] font-semibold text-ink">
                        {p.discipline} · {p.category}
                      </span>
                      <span className="block truncate text-xs text-ink-3">{p.date && formatDayShort(p.date, language)}</span>
                    </span>
                    {p.teams && <Pill tone="brand">{t('team')}</Pill>}
                  </button>
                ))}
              </div>
            )}
            {classQuery.trim() !== '' && found.length === 0 && <p className="text-xs text-ink-3">{t('no_class_found')}</p>}
          </div>
        </Field>
        {teamClass && (
          <Field label={t('team')}>
            <div className="grid gap-2 sm:grid-cols-2">
              <select value={team} onChange={e => setTeam(e.target.value)} className="field">
                <option value="">—</option>
                {free.map(x => (
                  <option key={x.id} value={x.id}>
                    {x.name} ({x.riders}/{teams.data?.size ?? '–'})
                  </option>
                ))}
                <option value="new">{t('new_team')}</option>
              </select>
              {team === 'new' && <input value={teamName} onChange={e => setTeamName(e.target.value)} placeholder={t('team_name')} className="field" />}
            </div>
          </Field>
        )}
      </div>
    </Modal>
  );
}

type DialogTab = 'entries' | 'contact';

/** A rider's horses at the show, each with its entries, in the order they first start. */
type HorseGroup = { key: string; name: string; guest: boolean; entries: EntryRow[] };
function byHorse(entries: EntryRow[]): HorseGroup[] {
  const when = (e: EntryRow) => (e.starts[0] ? `${e.starts[0].date} ${e.starts[0].time}` : `~${e.class_number.padStart(4, '0')}`);
  const sorted = [...entries].sort((a, b) => compare(when(a), when(b)));
  const horses: HorseGroup[] = [];
  for (const e of sorted) {
    const key = e.guest_horse ? `guest:${e.horse}` : String(e.horse_id);
    const h = horses.find(x => x.key === key);
    if (h) h.entries.push(e);
    else horses.push({ key, name: e.horse, guest: e.guest_horse, entries: [e] });
  }
  return horses;
}

/** A rider, in a dialog: their entries (every horse, every class), and their contacts. Opened
 *  from one entry (`entry`, marked); `peek` opens a class's start list, the entry marked on it;
 *  `select` marks another of the rider's entries (when the one marked goes). */
export function EntryModal({
  show,
  entry,
  entries,
  onClose,
  peek,
  select,
}: {
  show: string;
  entry: EntryRow | null;
  entries: EntryRow[];
  onClose: () => void;
  peek: (classId: string, entry: number) => void;
  select: (entry: number) => void;
}) {
  const t = useT();
  const [tab, setTab] = useState<DialogTab>('entries');
  useEffect(() => {
    if (entry === null) setTab('entries');
  }, [entry]);
  const mine = entry ? entries.filter(e => e.rider_id === entry.rider_id) : [];
  if (entry && !mine.some(e => e.id === entry.id)) mine.push(entry);
  return (
    <Modal
      open={entry !== null}
      onClose={onClose}
      title={entry?.rider ?? ''}
      description={entry ? [entry.club, entry.town].filter(Boolean).join(' · ') : ''}
      icon={UserRound}
      size="lg"
      bodyClass="pt-0"
      focusFirst={false}
    >
      {entry && (
        <>
          <div className="sticky top-0 z-10 -mx-5 mb-4 border-b border-line bg-card px-3 sm:-mx-6 sm:px-4">
            <Tabs
              value={tab}
              onChange={setTab}
              items={[
                { value: 'entries', label: t('entries_tab'), icon: ClipboardList, count: mine.length },
                { value: 'contact', label: t('contact_tab'), icon: Contact },
              ]}
            />
          </div>
          {tab === 'entries' && (
            <RiderEntries
              show={show}
              entry={entry}
              mine={mine}
              peek={peek}
              removed={id => {
                const other = mine.find(e => e.id !== id);
                if (id !== entry.id) return;
                if (other) select(other.id);
                else onClose();
              }}
            />
          )}
          {tab === 'contact' && <ContactTab show={show} entry={entry} />}
        </>
      )}
    </Modal>
  );
}

/** A rider's dialog opened from elsewhere (a rider's name on a start list): the show's entries
 *  loaded for it; a class opened from it is that class's page. */
export function RiderDialog({ show, entry, onClose }: { show: string; entry: number | null; onClose: () => void }) {
  const { version } = useChanges();
  const [current, setCurrent] = useState<number | null>(entry);
  useEffect(() => setCurrent(entry), [entry]);
  const entries = useLoad(() => (entry === null ? Promise.resolve([] as EntryRow[]) : call<EntryRow[]>('participants', { show })), [show, entry === null, version]);
  const row = entries.data?.find(e => e.id === current) ?? null;
  return (
    <EntryModal
      show={show}
      entry={entry === null ? null : row}
      entries={entries.data ?? []}
      onClose={onClose}
      select={setCurrent}
      peek={(classId, entry) => {
        onClose();
        window.location.hash = href({ show, view: 'classes', class: classId, entry });
      }}
    />
  );
}

/** The rider's entries as a table: a block per horse (« Changer de cheval… »), a line per class:
 *  where it starts, does not start, hors concours, the remarks (the rider's, the office's note: a
 *  speech bubble adds or changes it), and a menu for the rest. « Changer de cavalier… » above. */
function RiderEntries({ show, entry, mine, peek, removed }: { show: string; entry: EntryRow; mine: EntryRow[]; peek: (classId: string, entry: number) => void; removed: (entry: number) => void }) {
  const t = useT();
  const plural = usePlural();
  const language = useLanguage();
  const { change } = useChanges();
  const horses = byHorse(mine);
  const [changing, setChanging] = useState<{ kind: 'horse'; horse: HorseGroup } | { kind: 'rider' } | null>(null);
  const [note, setNote] = useState<EntryRow | null>(null);
  const [level, setLevel] = useState<EntryRow | null>(null);
  const [moving, setMoving] = useState<EntryRow | null>(null);
  const [phases, setPhases] = useState<EntryRow | null>(null);
  const [removing, setRemoving] = useState<EntryRow | null>(null);
  const toggle = (e: EntryRow, field: 'not_starting' | 'hors_concours', value: boolean) => change('edit_entry', { entry: e.id, [field]: value });
  const chip = 'h-7 rounded-md px-2 text-[11px] font-bold ring-1 transition-colors pointer-coarse:h-9 pointer-coarse:px-3';
  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-xs text-ink-3">
          {horses.length} {plural(horses.length, 'horses')} · {mine.length} {plural(mine.length, 'entries')}
        </span>
        <Button size="sm" icon={UserRound} onClick={() => setChanging({ kind: 'rider' })}>
          {t('change_rider_all')}…
        </Button>
      </div>
      {horses.map(h => (
        <section key={h.key} className="overflow-hidden rounded-xl border border-line">
          <header className="flex flex-wrap items-center gap-2 border-b border-line bg-raised px-3 py-2">
            <Horseshoe className="size-4 shrink-0 text-ink-4" />
            <span className="min-w-0 truncate text-[13px] font-semibold text-ink">{h.name}</span>
            {h.guest && <Pill tone="accent">{t('guest_horse')}</Pill>}
            <Button size="sm" variant="ghost" icon={ArrowRightLeft} onClick={() => setChanging({ kind: 'horse', horse: h })} className="ml-auto">
              {t('change_horse_of')}…
            </Button>
          </header>
          <ul className="divide-y divide-line">
            {h.entries.map(e => {
              const off = e.not_starting || e.se_status === 'abgemeldet';
              const targets = !e.team;
              return (
                <li key={e.id} className={`flex flex-wrap items-center gap-x-3 gap-y-2 px-3 py-2.5 ${e.id === entry.id ? 'bg-accent-soft/40' : ''}`}>
                  <span className={`grid size-9 shrink-0 place-items-center rounded-lg bg-sunken text-[13px] font-bold text-ink ring-1 ring-line tabular ${off ? 'opacity-60' : ''}`}>
                    {e.class_number}
                  </span>
                  <div className="min-w-0 flex-1 basis-48">
                    <div className={`flex min-w-0 flex-wrap items-center gap-1.5 text-[13px] ${off ? 'text-ink-3 line-through' : 'text-ink'}`}>
                      <span className="truncate font-semibold">
                        {e.discipline} · {e.category}
                      </span>
                      {e.levels > 1 && e.stage ? <Pill>{`${t('level')} ${e.stage}`}</Pill> : null}
                    </div>
                    <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-ink-3">
                      {e.starts.length === 0 && <span>{t('not_on_a_list')}</span>}
                      {e.starts.map(s => (
                        <button key={s.class} onClick={() => peek(s.class, e.id)} title={t('see_start_list')} className="inline-flex items-center gap-1 rounded hover:text-accent">
                          {s.phase && <span>{phaseName(t, s.phase)}</span>}
                          <span>{[formatDayShort(s.date, language), s.time].filter(Boolean).join(' ')}</span>
                          {s.position !== null && <span>· {s.position}.</span>}
                          {s.start_number !== null && <span className="font-semibold text-ink-2">#{s.start_number}</span>}
                          <Eye className="size-3" />
                        </button>
                      ))}
                      {e.se_status === 'abgemeldet' && <Pill>{t('withdrawn')}</Pill>}
                      {e.waiting_list && <Pill tone="warn">{t('waiting')}</Pill>}
                      {e.rider_changed && <Pill tone="warn">RW</Pill>}
                      {e.organiser_place && <Pill tone="info">{t('office_place')}</Pill>}
                      {e.team && <Pill tone="brand">{`${t('team')} · ${e.team}`}</Pill>}
                    </div>
                    {(e.remark || e.comment) && (
                      <div className="mt-1 space-y-0.5 text-xs leading-snug">
                        {e.remark && (
                          <div className="flex items-start gap-1.5 text-ink-2" title={t('remark_rider')}>
                            <MessageSquareText className="mt-px size-3.5 shrink-0 text-ink-4" />
                            <span className="min-w-0">{e.remark}</span>
                          </div>
                        )}
                        {e.comment && (
                          <button onClick={() => setNote(e)} className="flex items-start gap-1.5 text-left text-warn hover:underline" title={t('col_note')}>
                            <NotebookPen className="mt-px size-3.5 shrink-0" />
                            <span className="min-w-0 italic">{e.comment}</span>
                          </button>
                        )}
                      </div>
                    )}
                  </div>
                  <span className="flex shrink-0 items-center gap-1">
                    <button
                      onClick={() => toggle(e, 'not_starting', !e.not_starting)}
                      title={e.not_starting ? t('starts_again') : t('not_starting_long')}
                      aria-pressed={e.not_starting}
                      className={`${chip} ${e.not_starting ? 'bg-ink-2 text-card ring-ink-2' : 'text-ink-3 ring-line hover:bg-hover hover:text-ink'}`}
                    >
                      {t('not_starting')}
                    </button>
                    <button
                      onClick={() => toggle(e, 'hors_concours', !e.hors_concours)}
                      title={t('hors_concours')}
                      aria-pressed={e.hors_concours}
                      className={`${chip} ${e.hors_concours ? 'bg-warn text-white ring-warn' : 'text-ink-3 ring-line hover:bg-hover hover:text-ink'}`}
                    >
                      {t('hc')}
                    </button>
                    <IconButton icon={e.comment ? NotebookPen : MessageSquarePlus} label={e.comment ? t('col_note') : t('note_add')} size="sm" onClick={() => setNote(e)} />
                    <Menu trigger={<IconButton icon={Ellipsis} label={t('actions_more')} size="sm" />}>
                      {e.starts[0] && (
                        <MenuItem icon={Eye} onSelect={() => peek(e.starts[0].class, e.id)}>
                          {t('see_start_list')}
                        </MenuItem>
                      )}
                      {e.levels > 1 && (
                        <MenuItem icon={Layers} onSelect={() => setLevel(e)}>
                          {t('level_of_entry')}…
                        </MenuItem>
                      )}
                      {e.driving_phases.length > 0 && e.driving_horses.length > 0 && (
                        <MenuItem icon={Horseshoe} onSelect={() => setPhases(e)}>
                          {t('horses_by_phase')}…
                        </MenuItem>
                      )}
                      {targets && (
                        <MenuItem icon={ArrowRightLeft} onSelect={() => setMoving(e)}>
                          {t('move_to_class')}…
                        </MenuItem>
                      )}
                      {e.organiser_place && (
                        <>
                          <MenuSeparator />
                          <MenuItem icon={Trash2} danger onSelect={() => setRemoving(e)}>
                            {t('remove_entry')}
                          </MenuItem>
                        </>
                      )}
                    </Menu>
                  </span>
                </li>
              );
            })}
          </ul>
        </section>
      ))}
      {changing && (
        <ChangeFlow
          show={show}
          kind={changing.kind}
          from={changing.kind === 'horse' ? (changing.horse.entries.find(e => e.id === entry.id) ?? changing.horse.entries[0]) : entry}
          pair={changing.kind === 'horse' ? changing.horse.entries : mine}
          onClose={() => setChanging(null)}
        />
      )}
      {note && <NoteModal entry={note} onClose={() => setNote(null)} />}
      {level && <LevelModal entry={level} onClose={() => setLevel(null)} />}
      {moving && <MoveModal show={show} entry={moving} onClose={() => setMoving(null)} />}
      {phases && (
        <Modal
          open
          onClose={() => setPhases(null)}
          title={t('horses_by_phase')}
          description={`${phases.class_number} · ${phases.horse}`}
          icon={Horseshoe}
          size="md"
          footer={<Button onClick={() => setPhases(null)}>{t('close')}</Button>}
        >
          <HorsePhases entry={mine.find(e => e.id === phases.id) ?? phases} />
        </Modal>
      )}
      <ConfirmModal
        open={removing !== null}
        onClose={() => setRemoving(null)}
        title={t('remove_entry')}
        message={removing ? `${removing.rider} · ${removing.horse} · ${removing.class_number}` : ''}
        confirm={t('remove_entry')}
        danger
        icon={Trash2}
        onConfirm={async () => {
          const gone = removing;
          setRemoving(null);
          if (gone && (await change('remove_entry', { entry: gone.id }))) removed(gone.id);
        }}
      />
    </div>
  );
}

/** Another horse (for a horse's entries) or another rider (for the rider's): picked in a search,
 *  then, with several entries, which of them (one change; ⌘Z undoes it). */
function ChangeFlow({ show, kind, from, pair, onClose }: { show: string; kind: 'horse' | 'rider'; from: EntryRow; pair: EntryRow[]; onClose: () => void }) {
  const t = useT();
  const { change } = useChanges();
  const [horse, setHorse] = useState<HorsePick | null>(null);
  const [rider, setRider] = useState<RiderPick | null>(null);
  const horseArgs = (h: HorsePick) => ('horse' in h ? { horse: h.horse } : { guest: h.guest });
  const apply = (entries: number[]) =>
    kind === 'horse' && horse ? change('change_horse', { entries, ...horseArgs(horse) }) : rider ? change('change_rider', { entries, rider: { person: rider.person } }) : Promise.resolve(false);
  const picked = async (h: HorsePick | null, r: RiderPick | null) => {
    // One entry: at once.
    if (pair.length === 1) {
      const done = h ? await change('change_horse', { entries: [pair[0].id], ...horseArgs(h) }) : r ? await change('change_rider', { entries: [pair[0].id], rider: { person: r.person } }) : false;
      if (done) onClose();
      return;
    }
    setHorse(h);
    setRider(r);
  };
  if (horse || rider) {
    const to = horse ? ('horse' in horse ? horse.label : `${t('guest_as')} ${horse.guest}`) : (rider?.label ?? '');
    const horsesIn = new Set(pair.map(e => e.horse)).size;
    return (
      <PairChangeModal
        show={show}
        kind={kind}
        entry={from}
        pair={pair}
        to={to}
        hint={kind === 'rider' && horsesIn > 1 ? t('rider_which_hint').replace('{n}', String(pair.length)) : undefined}
        onClose={onClose}
        apply={apply}
      />
    );
  }
  return (
    <Modal
      open
      onClose={onClose}
      title={t(kind === 'horse' ? 'change_horse_of' : 'change_rider_all')}
      description={kind === 'horse' ? from.horse : from.rider}
      icon={kind === 'horse' ? Horseshoe : UserRound}
      size="md"
      footer={
        <Button variant="ghost" onClick={onClose}>
          {t('cancel')}
        </Button>
      }
    >
      {kind === 'horse' ? <HorseSearch show={show} entry={from.id} onPick={h => picked(h, null)} /> : <RiderSearch show={show} onPick={r => picked(null, r)} />}
      {kind === 'rider' && <p className="mt-3 text-xs text-ink-3">{t('change_rider_hint')}</p>}
    </Modal>
  );
}

/** The office's note on an entry: written, changed or cleared in a small dialog. */
function NoteModal({ entry, onClose }: { entry: EntryRow; onClose: () => void }) {
  const t = useT();
  const { change } = useChanges();
  const [text, setText] = useState(entry.comment);
  const save = async (comment: string) => (await change('edit_entry_details', { entry: entry.id, comment })) && onClose();
  return (
    <Modal
      open
      onClose={onClose}
      title={entry.comment ? t('col_note') : t('note_add')}
      description={`${entry.class_number} · ${entry.horse}`}
      icon={NotebookPen}
      size="sm"
      footer={
        <>
          {entry.comment && (
            <Button variant="danger" onClick={() => save('')} className="mr-auto">
              {t('note_clear')}
            </Button>
          )}
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button variant="primary" disabled={text.trim() === entry.comment.trim()} onClick={() => save(text)}>
            {t('save')}
          </Button>
        </>
      }
    >
      <textarea data-autofocus value={text} onChange={e => setText(e.target.value)} rows={3} className="field min-h-24 py-2" />
      {entry.remark && (
        <div className="mt-3 flex items-start gap-1.5 text-xs text-ink-3">
          <MessageSquareText className="mt-px size-3.5 shrink-0" />
          <span>
            {t('remark_rider')} : {entry.remark}
          </span>
        </div>
      )}
    </Modal>
  );
}

/** An entry's level, in a class with levels. */
function LevelModal({ entry, onClose }: { entry: EntryRow; onClose: () => void }) {
  const t = useT();
  const { change } = useChanges();
  const [level, setLevel] = useState(entry.stage ? String(entry.stage) : '');
  const ok = level.trim() === '' || /^[1-9]$/.test(level.trim());
  return (
    <Modal
      open
      onClose={onClose}
      title={t('level_of_entry')}
      description={`${entry.class_number} · ${entry.horse}`}
      icon={Layers}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button variant="primary" disabled={!ok} onClick={async () => (await change('edit_entry_details', { entry: entry.id, stage: Number(level) || 0 })) && onClose()}>
            {t('save')}
          </Button>
        </>
      }
    >
      <Field label={t('level_label')}>
        <input data-autofocus value={level} onChange={e => setLevel(e.target.value)} inputMode="numeric" className={`field w-24 text-right ${ok ? '' : 'border-accent'}`} />
      </Field>
    </Modal>
  );
}

/** An entry moved to another published class of the same discipline. */
function MoveModal({ show, entry, onClose }: { show: string; entry: EntryRow; onClose: () => void }) {
  const t = useT();
  const { change } = useChanges();
  const published = useLoad(() => call<PublishedClass[]>('published', { show }), [show]);
  const targets = (published.data ?? []).filter(p => p.discipline === entry.discipline && !p.teams && p.id !== entry.published);
  const [target, setTarget] = useState('');
  return (
    <Modal
      open
      onClose={onClose}
      title={t('move_to_class')}
      description={`${entry.class_number} · ${entry.horse}`}
      icon={ArrowRightLeft}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button variant="primary" icon={ArrowRightLeft} disabled={!target} onClick={async () => (await change('move_to_class', { entry: entry.id, published: Number(target) })) && onClose()}>
            {t('move')}
          </Button>
        </>
      }
    >
      <Field label={t('class')} hint={t('move_to_class_hint')}>
        <select value={target} onChange={e => setTarget(e.target.value)} className="field">
          <option value="">—</option>
          {targets.map(p => (
            <option key={p.id} value={p.id}>
              {p.number} · {p.category}
            </option>
          ))}
        </select>
      </Field>
    </Modal>
  );
}

const compare = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);

/** A pair entered in several classes gets another horse or rider: in which of them. All by
 *  default, but those already ridden; a final class cannot change. One change (⌘Z undoes it). */
function PairChangeModal({
  show,
  kind,
  entry,
  pair,
  to,
  hint,
  onClose,
  apply,
}: {
  show: string;
  kind: 'horse' | 'rider';
  entry: EntryRow;
  pair: EntryRow[];
  to: string;
  /** What the list is (default: a pair in several classes). */
  hint?: string;
  onClose: () => void;
  apply: (entries: number[]) => Promise<unknown>;
}) {
  const t = useT();
  const language = useLanguage();
  // Several horses (a rider's entries): each line says which.
  const horsesShown = new Set(pair.map(e => e.horse)).size > 1;
  // In the order they ride: by day (a class not set up yet: its published day), time, number.
  const published = useLoad(() => call<PublishedClass[]>('published', { show }), [show]).data;
  const day = (e: EntryRow) => e.starts[0]?.date ?? published?.find(p => p.id === e.published)?.date ?? '';
  const rows = [...pair].sort(
    (a, b) => compare(day(a) || '~', day(b) || '~') || compare(a.starts[0]?.time ?? '', b.starts[0]?.time ?? '') || a.class_number.localeCompare(b.class_number, undefined, { numeric: true }),
  );
  const open = pair.filter(e => !e.locked);
  const [picked, setPicked] = useState<Set<number>>(() => {
    const fresh = open.filter(e => !e.ridden).map(e => e.id);
    return new Set(fresh.length > 0 ? fresh : entry.locked ? [] : [entry.id]);
  });
  const [busy, setBusy] = useState(false);
  const n = picked.size;
  const all = open.length > 0 && open.every(e => picked.has(e.id)) ? true : n > 0 ? 'mixed' : false;
  const tick = (id: number, on: boolean) =>
    setPicked(before => {
      const next = new Set(before);
      if (on) next.add(id);
      else next.delete(id);
      return next;
    });
  const go = async () => {
    setBusy(true);
    const done = await apply([...picked]);
    setBusy(false);
    if (done) onClose();
  };
  return (
    <Modal
      open
      onClose={onClose}
      title={t(kind === 'horse' ? 'change_horse' : 'change_rider_label')}
      icon={kind === 'horse' ? Horseshoe : UserRound}
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button variant="primary" disabled={n === 0} loading={busy} onClick={go}>
            {n === 1 ? t('change_in_one') : t('change_in_many').replace('{n}', String(n))}
          </Button>
        </>
      }
    >
      <div className="grid gap-5">
        <div className="flex items-center gap-3 rounded-xl border border-line bg-raised px-4 py-3">
          <span className="min-w-0 flex-1 truncate text-[13px] text-ink-3 line-through decoration-ink-4">{kind === 'horse' ? entry.horse : entry.rider}</span>
          <ArrowRight className="size-4 shrink-0 text-accent" />
          <span className="min-w-0 flex-1 truncate text-right text-[14px] font-semibold text-ink">{to}</span>
        </div>
        <Field label={t('pair_which')} hint={hint ?? t('pair_which_hint').replace('{n}', String(pair.length))}>
          <div className="overflow-hidden rounded-xl border border-line">
            <div className="bg-raised">
              <CheckRow checked={all} disabled={open.length === 0} onChange={on => setPicked(new Set(on ? open.map(e => e.id) : []))}>
                <span className="text-[13px] font-semibold text-ink">{t('pair_all')}</span>
                <span className="ml-auto text-xs font-semibold text-ink-3 tabular">
                  {n}/{open.length}
                </span>
              </CheckRow>
            </div>
            {rows.map(e => {
              const when = [day(e) && formatDayShort(day(e), language), e.starts[0]?.time].filter(Boolean).join(' ');
              return (
                <CheckRow key={e.id} checked={picked.has(e.id)} disabled={e.locked} onChange={on => tick(e.id, on)}>
                  <span className={`grid size-9 shrink-0 place-items-center rounded-lg bg-sunken text-[13px] font-bold text-ink ring-1 ring-line tabular ${e.locked ? 'opacity-50' : ''}`}>
                    {e.class_number}
                  </span>
                  <span className={`min-w-0 flex-1 ${e.locked ? 'opacity-60' : ''}`}>
                    <span className="block truncate text-[13px] font-semibold text-ink">
                      {e.discipline} · {e.category}
                    </span>
                    {(when || horsesShown) && <span className="block truncate text-xs text-ink-3">{[horsesShown && e.horse, when].filter(Boolean).join(' · ')}</span>}
                  </span>
                  <span className="flex shrink-0 flex-wrap justify-end gap-1">
                    {e.id === entry.id && <Pill tone="info">{t('this_entry')}</Pill>}
                    {e.locked ? (
                      <Pill icon={Lock}>{t('status_final')}</Pill>
                    ) : (
                      e.ridden && (
                        <Pill tone="warn" dot>
                          {t('ridden')}
                        </Pill>
                      )
                    )}
                    {e.not_starting && <Pill>{t('not_starting')}</Pill>}
                    {e.se_status === 'abgemeldet' && <Pill>{t('withdrawn')}</Pill>}
                  </span>
                </CheckRow>
              );
            })}
          </div>
        </Field>
      </div>
    </Modal>
  );
}

/** A rider's address and contacts, as Swiss Equestrian delivered them: read only. */
function ContactTab({ show, entry }: { show: string; entry: EntryRow }) {
  const d = useLoad(() => call<Record<string, string>>('address', { show, person: entry.rider_id }), [show, entry.rider_id]).data;
  if (!d) return null;
  const line = (Icon: typeof Mail, text: string, href?: string) =>
    text && (
      <div className="flex items-center gap-3 border-t border-line px-4 py-3 first:border-t-0">
        <Icon className="size-4 shrink-0 text-ink-4" />
        {href ? (
          <a href={href} className="min-w-0 break-words text-[13px] text-ink hover:text-accent">
            {text}
          </a>
        ) : (
          <span className="min-w-0 break-words text-[13px]">{text}</span>
        )}
      </div>
    );
  return (
    <div className="overflow-hidden rounded-xl border border-line">
      {line(MapPin, [d.street, [d.postcode, d.town].filter(Boolean).join(' ')].filter(Boolean).join(', '))}
      {line(Mail, d.email, d.email ? `mailto:${d.email}` : undefined)}
      {line(Phone, d.phone_mobile, d.phone_mobile ? `tel:${d.phone_mobile.replace(/\s/g, '')}` : undefined)}
      {line(Phone, d.phone_private, d.phone_private ? `tel:${d.phone_private.replace(/\s/g, '')}` : undefined)}
    </div>
  );
}

/** Swiss Equestrian's name of a driving phase → the app's. */
const PHASE_OF_NAME: Record<string, PhaseKind> = { dressur: 'dressage', gelaende: 'marathon', hindernis: 'cones', derby: 'derby', offene: 'open', multitest: 'multitest' };

/** Driving: which horse starts in which phase (Rosson's horse × phase grid). */
function HorsePhases({ entry }: { entry: EntryRow }) {
  const t = useT();
  const { change } = useChanges();
  const toggle = (i: number, phase: string) => {
    const now = entry.driving_horses[i].phases;
    const phases = now.includes(phase) ? now.filter(p => p !== phase) : [...now, phase];
    change('set_horse_phases', { entry: entry.id, horse: i, phases });
  };
  return (
    <Field label={t('horses_by_phase')}>
      <div className="overflow-x-auto rounded-xl border border-line">
        <table className="w-full text-xs">
          <thead>
            <tr className="bg-raised">
              <th />
              {entry.driving_phases.map(p => (
                <th key={p} className="px-3 py-2 text-center font-semibold text-ink-3">
                  {PHASE_OF_NAME[p] ? phaseName(t, PHASE_OF_NAME[p]) : p}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {entry.driving_horses.map((h, i) => (
              <tr key={i} className="border-t border-line">
                <td className="max-w-[12rem] truncate px-3 py-2 font-semibold">{h.name}</td>
                {entry.driving_phases.map(p => (
                  <td key={p} className="px-3 py-2 text-center">
                    <input
                      type="checkbox"
                      checked={h.phases.includes(p)}
                      onChange={() => toggle(i, p)}
                      aria-label={`${h.name} · ${PHASE_OF_NAME[p] ? phaseName(t, PHASE_OF_NAME[p]) : p}`}
                      className="check pointer-coarse:size-6"
                    />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Field>
  );
}
