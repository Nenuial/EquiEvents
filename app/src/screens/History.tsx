// The show's history: every change, latest first, who made it, when, on which device; runs of
// the same change (an import, a file) on one line. Searchable. Read only: undo is ⌘Z.

import { useEffect, useState, type ComponentType } from 'react';
import { CloudDownload, CloudUpload, Gavel, History as HistoryIcon, Layers, ListOrdered, Lock, LockOpen, PenLine, Search, SlidersHorizontal, UserRound, Users } from 'lucide-react';
import { call } from '../api';
import { formatMillis, useLanguage, useT } from '../i18n';
import { useChanges } from '../parts/changes';
import { ErrorNote } from '../parts/common';
import { Card, EmptyState, Kbd, PageHeader, Pill } from '../ui';

interface Line {
  millis: number;
  author: string;
  device: string;
  kind: string;
  text: string;
  count: number;
}

/** What a change is about, as an icon and a colour: received from Swiss Equestrian, sent to it,
 *  participants, a class's settings, start lists, officials, results, closing a class… */
function look(kind: string): [ComponentType<{ className?: string }>, string] {
  if (/Delivered|Imported|LookedUp/.test(kind)) return [CloudDownload, 'bg-info/12 text-info'];
  if (kind === 'Uploaded') return [CloudUpload, 'bg-info/12 text-info'];
  if (kind === 'ClassFinalised') return [Lock, 'bg-ok/12 text-ok'];
  if (kind === 'ClassReopened') return [LockOpen, 'bg-warn/14 text-warn'];
  if (/Result|Sheet|RankSet/.test(kind)) return [PenLine, 'bg-accent-soft text-accent'];
  if (kind === 'StartsSet') return [ListOrdered, 'bg-sunken text-ink-2'];
  if (/Assignment/.test(kind)) return [Gavel, 'bg-sunken text-ink-2'];
  if (/Team/.test(kind)) return [Users, 'bg-sunken text-ink-2'];
  if (/Combined/.test(kind)) return [Layers, 'bg-sunken text-ink-2'];
  if (/Entry|Person|Horse|Address/.test(kind)) return [UserRound, 'bg-sunken text-ink-2'];
  return [SlidersHorizontal, 'bg-sunken text-ink-2'];
}

/** The day a change was made, for the timeline's headings. */
const dayOf = (millis: number, language: string) => new Date(millis).toLocaleDateString(`${language}-CH`, { weekday: 'long', day: 'numeric', month: 'long' });
const timeOf = (millis: number, language: string) => new Date(millis).toLocaleTimeString(`${language}-CH`, { hour: '2-digit', minute: '2-digit' });

export default function History({ show }: { show: string }) {
  const t = useT();
  const language = useLanguage();
  const { version } = useChanges();
  const [query, setQuery] = useState('');
  const [lines, setLines] = useState<Line[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    call<Line[]>('history', { show, language, query, limit: 400 })
      .then(l => live && (setLines(l), setError(null)))
      .catch(e => live && setError(String(e.message ?? e)));
    return () => {
      live = false;
    };
  }, [show, language, query, version]);
  // Grouped by day.
  const days: { day: string; lines: Line[] }[] = [];
  for (const l of lines ?? []) {
    const day = dayOf(l.millis, language);
    if (days[days.length - 1]?.day !== day) days.push({ day, lines: [] });
    days[days.length - 1].lines.push(l);
  }
  return (
    <div className="h-full overflow-auto">
      <div className="mx-auto max-w-4xl space-y-5 px-3 py-5 sm:px-6 sm:py-7">
        <PageHeader
          icon={HistoryIcon}
          title={t('history')}
          subtitle={
            <span className="inline-flex items-center gap-1.5">
              {t('history_note')} <Kbd>⌘Z</Kbd>
            </span>
          }
        />
        <div className="relative">
          <Search className="pointer-events-none absolute top-1/2 left-3.5 size-4 -translate-y-1/2 text-ink-4" />
          <input value={query} onChange={e => setQuery(e.target.value)} placeholder={t('history_search')} className="field h-11 pl-10 text-sm" />
        </div>
        <ErrorNote error={error} />
        {lines && lines.length === 0 && (
          <Card>
            <EmptyState icon={HistoryIcon} title={t('nothing_found')} />
          </Card>
        )}
        {days.map(d => (
          <Card key={d.day} className="overflow-hidden">
            <div className="border-b border-line bg-raised px-5 py-2.5 text-xs font-semibold text-ink-2 capitalize">{d.day}</div>
            <ol className="relative">
              {/* The timeline's thread, through the icons. */}
              <span className="absolute top-0 bottom-0 left-[93px] w-px bg-line sm:left-[97px]" aria-hidden />
              {d.lines.map((l, i) => {
                const [Icon, tone] = look(l.kind);
                return (
                  <li key={i} title={formatMillis(l.millis, language)} className="relative flex items-center gap-4 px-4 py-2 sm:px-5">
                    <span className="w-12 shrink-0 text-xs font-semibold text-ink-3 tabular">{timeOf(l.millis, language)}</span>
                    <span className={`relative grid size-7 shrink-0 place-items-center rounded-full ring-4 ring-card ${tone}`} aria-hidden>
                      <Icon className="size-3.5" />
                    </span>
                    <span className="min-w-0 flex-1 text-[13px] text-ink">
                      {l.text}
                      {l.count > 1 && (
                        <span className="ml-2">
                          <Pill>×{l.count}</Pill>
                        </span>
                      )}
                    </span>
                    <span className="hidden shrink-0 text-xs text-ink-3 sm:inline">
                      {l.author}
                      {l.device && l.device !== 'dev' ? ` · ${l.device}` : ''}
                    </span>
                  </li>
                );
              })}
            </ol>
          </Card>
        ))}
      </div>
    </div>
  );
}
