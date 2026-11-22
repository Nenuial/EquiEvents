// Everyone entered, rider by rider (U2): searched and filtered; an entry opens in a dialog with
// everything the office does with it; a late entry too; a class's start list can be looked at,
// and reordered, without leaving (U3). Does not start (N) and hors concours (H) at hand.

import { Fragment, useEffect, useMemo, useRef, useState } from 'react';
import { ArrowRight, ListOrdered, Plus, Search, Users } from 'lucide-react';
import { call, type ClassRow, type EntryRow, type StartListView } from '../api';
import { href, type Place } from '../App';
import { formatDayShort, useLanguage, usePlural, useT } from '../i18n';
import { useChanges } from '../parts/changes';
import { ErrorNote, useLoad, useNarrow } from '../parts/common';
import { EntryModal, LateEntryModal } from '../parts/entries';
import { phaseName } from '../parts/overall';
import { StartList } from '../parts/startlist';
import { Button, Card, EmptyState, Kbd, Modal, PageHeader, Pill, Segmented, buttonClass, overlayOpen } from '../ui';
import { sectionsOf } from './ClassPage';

type Go = (place: Place) => void;
type Filter = 'all' | 'not_starting' | 'hc' | 'office' | 'changed';

export function Participants({ show, peek, classes, go }: { show: string; peek?: string; classes: ClassRow[]; go: Go }) {
  const t = useT();
  const plural = usePlural();
  const language = useLanguage();
  const { version, change } = useChanges();
  const entries = useLoad(() => call<EntryRow[]>('participants', { show }), [show, version]);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<Filter>('all');
  const [highlight, setHighlight] = useState<number | undefined>();
  const [open, setOpen] = useState<number | null>(null);
  const [late, setLate] = useState(false);
  const search = useRef<HTMLInputElement>(null);
  const narrow = useNarrow();

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (overlayOpen() || (e.target as HTMLElement).closest('input, textarea, select')) return;
      if (e.key === '/') {
        e.preventDefault();
        search.current?.focus();
      } else if (e.key === '+') {
        e.preventDefault();
        setLate(true);
      }
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, []);

  const rows = useMemo(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return (entries.data ?? []).filter(e => {
      if (filter === 'not_starting' && !e.not_starting) return false;
      if (filter === 'hc' && !e.hors_concours) return false;
      if (filter === 'office' && !e.organiser_place) return false;
      if (filter === 'changed' && !e.rider_changed) return false;
      if (!words.length) return true;
      const text = [e.rider, e.horse, e.club, e.town, e.class_number, e.category, e.team ?? '', ...e.starts.map(s => `${s.number} ${s.start_number ?? ''}`)].join(' ').toLowerCase();
      return words.every(w => text.includes(w));
    });
  }, [entries.data, query, filter]);
  const entry = entries.data?.find(e => e.id === open) ?? null;
  const riders = new Set(entries.data?.map(e => e.rider_id)).size;

  const toggle = (e: EntryRow, field: 'not_starting' | 'hors_concours') => change('edit_entry', { entry: e.id, [field]: !e[field] });
  const flags = (e: EntryRow) => (
    <>
      {e.se_status === 'abgemeldet' && <Pill>{t('withdrawn')}</Pill>}
      {e.waiting_list && <Pill tone="warn">{t('waiting')}</Pill>}
      {e.rider_changed && <Pill tone="warn">RW</Pill>}
      {e.organiser_place && <Pill tone="info">{t('office_place')}</Pill>}
      {e.guest_horse && <Pill tone="accent">{t('guest_horse')}</Pill>}
    </>
  );
  // Does not start, hors concours: a tap away, quiet until set (or hovered).
  const quick = (e: EntryRow) => (
    <span className="inline-flex gap-1" onClick={ev => ev.stopPropagation()}>
      {(['not_starting', 'hors_concours'] as const).map(f => (
        <button
          key={f}
          onClick={() => toggle(e, f)}
          title={f === 'not_starting' ? (e.not_starting ? t('starts_again') : t('not_starting_long')) : t('hors_concours')}
          className={`h-7 rounded-md px-2 text-[11px] font-bold transition-colors pointer-coarse:h-9 pointer-coarse:px-3 ${
            e[f]
              ? f === 'not_starting'
                ? 'bg-ink-2 text-card'
                : 'bg-warn text-white'
              : 'text-ink-4 opacity-0 ring-1 ring-line group-hover:opacity-100 hover:bg-hover hover:text-ink pointer-coarse:opacity-100'
          }`}
        >
          {f === 'not_starting' ? t('not_starting') : t('hc')}
        </button>
      ))}
    </span>
  );
  const starts = (e: EntryRow) => (
    <span className="flex flex-wrap gap-1" onClick={ev => ev.stopPropagation()}>
      {e.starts.map(s => (
        <button
          key={s.class}
          onClick={() => {
            setHighlight(e.id);
            go({ show, view: 'participants', peek: s.class });
          }}
          title={t('start_list')}
          className="inline-flex items-center gap-1 rounded-md bg-sunken px-1.5 py-0.5 text-[11px] text-ink-2 ring-1 ring-line hover:text-ink hover:ring-accent pointer-coarse:py-1.5"
        >
          <span className="font-bold text-ink">{s.number}</span>
          {s.phase && phaseName(t, s.phase)}
          <span className="text-ink-3">
            {[formatDayShort(s.date, language), s.time].filter(Boolean).join(' ')}
            {s.position !== null && ` · ${s.position}.`}
          </span>
        </button>
      ))}
    </span>
  );

  const th = 'sticky top-0 z-10 border-b border-line bg-raised px-3 py-2.5 text-left text-xs font-semibold text-ink-3';
  const td = 'px-3 py-2.5 align-top';

  return (
    <div className="pane">
      <div className="mx-auto max-w-[1400px] space-y-4 px-3 py-5 sm:px-6 sm:py-7">
        <PageHeader
          icon={Users}
          title={t('participants')}
          subtitle={entries.data ? `${entries.data.length} ${plural(entries.data.length, 'entries')} · ${riders} ${plural(riders, 'riders')}` : t('loading')}
          actions={
            <Button variant="primary" icon={Plus} onClick={() => setLate(true)} kbd="+">
              {t('late_entry_title')}
            </Button>
          }
        />
        <div className="flex flex-wrap items-center gap-3">
          <div className="relative min-w-0 flex-1 basis-72">
            <Search className="pointer-events-none absolute top-1/2 left-3.5 size-4 -translate-y-1/2 text-ink-4" />
            <input
              ref={search}
              value={query}
              onChange={e => setQuery(e.target.value)}
              onKeyDown={e => e.key === 'Escape' && (setQuery(''), (e.target as HTMLInputElement).blur())}
              placeholder={t('search')}
              className="field h-11 pl-10 text-sm"
            />
            <span className="pointer-events-none absolute top-1/2 right-3 hidden -translate-y-1/2 md:block">
              <Kbd>/</Kbd>
            </span>
          </div>
          <Segmented
            value={filter}
            onChange={setFilter}
            size="sm"
            className="max-w-full overflow-x-auto"
            options={[
              { value: 'all', label: t('filter_all') },
              { value: 'not_starting', label: t('not_starting') },
              { value: 'hc', label: t('hc') },
              { value: 'office', label: t('office_place') },
              { value: 'changed', label: 'RW' },
            ]}
          />
        </div>
        <ErrorNote error={entries.error} />
        <Card className="overflow-hidden">
          {entries.data && rows.length === 0 ? (
            <EmptyState icon={Search} title={t('nothing_found')} />
          ) : narrow ? (
            /* On a phone: rider by rider, each entry under the name. */
            <ul className="divide-y divide-line">
              {rows.map((e, i) => {
                const first = i === 0 || rows[i - 1].rider_id !== e.rider_id;
                return (
                  <li key={e.id} onClick={() => setOpen(e.id)} className={`group cursor-pointer px-4 py-3 active:bg-hover ${highlight === e.id ? 'bg-accent-soft' : ''}`}>
                    {first && (
                      <div className="mb-1">
                        <div className="font-semibold">{e.rider}</div>
                        <div className="truncate text-xs text-ink-3">{[e.club, e.town].filter(Boolean).join(' · ')}</div>
                      </div>
                    )}
                    <div className="flex items-start gap-2">
                      <div className="min-w-0 flex-1">
                        <div className={`truncate text-[13px] font-medium ${e.not_starting ? 'text-ink-3 line-through' : ''}`}>{e.horse}</div>
                        <div className="truncate text-xs text-ink-3">
                          <span className="font-semibold text-ink-2">{e.class_number}</span> {e.discipline} · {e.category}
                        </div>
                        <div className="mt-1.5 flex flex-wrap gap-1">{flags(e)}</div>
                        <div className="mt-1.5">{starts(e)}</div>
                      </div>
                      {quick(e)}
                    </div>
                  </li>
                );
              })}
            </ul>
          ) : (
            <table className="w-full table-fixed text-[13px]">
              <thead>
                <tr>
                  <th className={`${th} w-[24%]`}>{t('rider')}</th>
                  <th className={`${th} w-[22%]`}>{t('horse')}</th>
                  <th className={`${th} w-[16%]`}>{t('class')}</th>
                  <th className={th}>{t('starts_in')}</th>
                  <th className={`${th} w-28`} />
                </tr>
              </thead>
              <tbody className="tabular">
                {rows.map((e, i) => {
                  const first = i === 0 || rows[i - 1].rider_id !== e.rider_id;
                  return (
                    <Fragment key={e.id}>
                      <tr
                        tabIndex={0}
                        onClick={() => setOpen(e.id)}
                        onKeyDown={ev => {
                          if (ev.target !== ev.currentTarget) return;
                          if (ev.key === 'Enter') setOpen(e.id);
                          if (ev.key === 'n' || ev.key === 'N') toggle(e, 'not_starting');
                          if (ev.key === 'h' || ev.key === 'H') toggle(e, 'hors_concours');
                        }}
                        className={`group cursor-pointer outline-none hover:bg-hover focus-visible:bg-hover ${first ? 'border-t border-line' : ''} ${highlight === e.id ? 'bg-accent-soft' : ''}`}
                      >
                        <td className={`${td} max-w-0`}>
                          {first && (
                            <>
                              <div className="truncate font-semibold text-ink">{e.rider}</div>
                              <div className="truncate text-xs text-ink-3">{[e.club, e.town].filter(Boolean).join(' · ')}</div>
                            </>
                          )}
                        </td>
                        <td className={`${td} max-w-0`}>
                          <div className={`truncate font-medium ${e.not_starting ? 'text-ink-3 line-through' : 'text-ink'}`}>{e.horse}</div>
                          {e.comment && <div className="truncate text-xs text-warn italic">{e.comment}</div>}
                        </td>
                        <td className={`${td} max-w-0`}>
                          <div className="truncate">
                            <span className="font-bold">{e.class_number}</span>{' '}
                            <span className="text-xs text-ink-3">
                              {e.discipline} · {e.category}
                            </span>
                          </div>
                          {e.team && (
                            <div className="truncate text-xs text-ink-3">
                              {t('team')} · {e.team}
                            </div>
                          )}
                        </td>
                        <td className={td}>
                          <div className="flex flex-wrap items-center gap-1">
                            {flags(e)}
                            {starts(e)}
                          </div>
                        </td>
                        <td className={`${td} text-right`}>{quick(e)}</td>
                      </tr>
                    </Fragment>
                  );
                })}
              </tbody>
            </table>
          )}
        </Card>
      </div>
      <EntryModal
        show={show}
        entry={entry}
        entries={entries.data ?? []}
        select={setOpen}
        onClose={() => setOpen(null)}
        peek={(c, id) => {
          setHighlight(id);
          go({ show, view: 'participants', peek: c });
        }}
      />
      <LateEntryModal
        show={show}
        open={late}
        onClose={() => setLate(false)}
        done={id => {
          setLate(false);
          setHighlight(id);
          setTimeout(() => document.querySelector('.bg-accent-soft')?.scrollIntoView({ block: 'center' }), 300);
        }}
      />
      <Peek show={show} classId={peek} highlight={highlight} classes={classes} close={() => go({ show, view: 'participants' })} />
    </div>
  );
}

/** A class's start list in front of the participants, changed in place (U3). */
function Peek({ show, classId, highlight, classes, close }: { show: string; classId?: string; highlight?: number; classes: ClassRow[]; close: () => void }) {
  const t = useT();
  const plural = usePlural();
  const { version } = useChanges();
  const list = useLoad(() => (classId ? call<StartListView>('start_list', { show, class: classId }) : Promise.resolve(null)), [show, classId, version]);
  const row = list.data?.class;
  return (
    <Modal
      open={!!classId}
      onClose={close}
      title={row ? `${row.number} · ${row.title}` : ''}
      description={row ? `${row.time ? `${row.time} · ` : ''}${row.category} · ${row.starters} ${plural(row.starters, 'entered_count')}` : undefined}
      icon={ListOrdered}
      size="xl"
      bodyClass="px-0 sm:px-0 pb-0"
      footer={
        classId && (
          <a href={href({ show, view: 'classes', class: classId, entry: highlight })} onClick={close} className={buttonClass('primary')}>
            {t('open_class')}
            <ArrowRight className="size-4" />
          </a>
        )
      }
    >
      <ErrorNote error={list.error} />
      {list.data && <StartList show={show} list={list.data} highlight={highlight} sections={sectionsOf(classes, row)} />}
    </Modal>
  );
}
