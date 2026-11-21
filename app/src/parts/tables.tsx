import { useState } from 'react';
import type { RankingLine, RankingView } from '../api';
import { useReason, useT } from '../i18n';
import { Pill } from '../ui';
import { useChanges } from './changes';

const th = 'sticky top-0 z-[1] border-b border-line bg-raised px-2 py-2.5 text-left text-xs font-semibold text-ink-3 sm:px-3';
const td = 'border-b border-line px-2 py-2 align-middle sm:px-3';

/** The podium, then the placed (who win a prize), then the others. */
const medal = (rank: number | null, placed: boolean) =>
  rank === 1
    ? 'bg-[#f2c94c] text-[#3d2c00] ring-[#d9ad2c]'
    : rank === 2
      ? 'bg-[#d5dde6] text-[#243040] ring-[#b6c2cf]'
      : rank === 3
        ? 'bg-[#e0a06a] text-[#3a1d04] ring-[#c7874f]'
        : placed
          ? 'bg-accent-soft text-accent ring-accent/25'
          : 'text-ink-2 ring-transparent';

/** A rank set by hand (a jump-off, a jury decision): click the rank, type it, Enter; empty
 *  gives the computed rank back. */
function RankCell({ line, classId, editable }: { line: RankingLine; classId?: string; editable: boolean }) {
  const t = useT();
  const { change } = useChanges();
  const [editing, setEditing] = useState(false);
  const shown = (
    <span
      className={`inline-flex h-7 min-w-7 items-center justify-center rounded-full px-1.5 text-[13px] font-bold ring-1 ring-inset ${medal(line.rank, line.placed)}`}
      title={line.manual ? t('rank_set_by_hand') : undefined}
    >
      {line.rank ?? '–'}
      {line.manual && '*'}
    </span>
  );
  if (!editable || !classId) return shown;
  if (!editing)
    return (
      <button onClick={() => setEditing(true)} title={t('rank_by_hand')} className="rounded-full hover:ring-2 hover:ring-accent/40 pointer-coarse:min-h-9">
        {shown}
      </button>
    );
  const save = async (text: string) => {
    const rank = text.trim() === '' ? null : Number(text.trim());
    if (rank !== null && (!Number.isInteger(rank) || rank < 1)) return;
    const who = line.team ? { team: line.id } : { entry: Number(line.id) };
    await change('set_rank', { class: classId, ...who, rank });
    setEditing(false);
  };
  return (
    <input
      autoFocus
      defaultValue={line.manual && line.rank !== null ? String(line.rank) : ''}
      placeholder={line.rank === null ? '' : String(line.rank)}
      inputMode="numeric"
      aria-label={t('rank_by_hand')}
      onKeyDown={e => {
        if (e.key === 'Enter') save(e.currentTarget.value);
        if (e.key === 'Escape') setEditing(false);
      }}
      onBlur={() => setEditing(false)}
      className="field field-sm w-14 text-right"
    />
  );
}

export function RankingTable({ ranking, classId, editable = false }: { ranking: RankingView; classId?: string; editable?: boolean }) {
  const t = useT();
  const reason = useReason();
  const dressage = ranking.class.discipline === 'CD' || ranking.lines.some(l => l.percentage !== null);
  const order = ranking.judge_order.length ? ranking.judge_order.join('') : 'EHCMB';
  const judges = [...new Set(ranking.lines.flatMap(l => Object.keys(l.judges)))].sort((a, b) => order.indexOf(a) - order.indexOf(b));
  // Team classes: a rider's row under the team is never "out" by itself.
  const teams = ranking.lines.some(l => l.team);
  // Freestyle and Children: parts A and B.
  const parts = ranking.lines.some(l => l.part_a !== null || l.part_b !== null);
  // Team cones (OKV Fahrcup): the teams' rank points.
  const points = ranking.lines.some(l => l.rank_points !== null);
  return (
    <table className="w-full text-[13px]">
      <thead>
        <tr>
          <th className={`${th} w-14 text-right`}>{t('rank')}</th>
          <th className={th}>{t('rider')}</th>
          <th className={`${th} hidden sm:table-cell`}>{t('horse')}</th>
          {dressage ? (
            <>
              {judges.map(j => (
                <th key={j} className={`${th} hidden w-16 text-right sm:table-cell`}>
                  {j}
                </th>
              ))}
              {parts && (
                <>
                  <th className={`${th} hidden w-16 text-right sm:table-cell`} title={t('part_a')}>
                    A %
                  </th>
                  <th className={`${th} hidden w-16 text-right sm:table-cell`} title={t('part_b')}>
                    B %
                  </th>
                </>
              )}
              <th className={`${th} w-20 text-right`}>{t('percent')}</th>
            </>
          ) : (
            <>
              <th className={`${th} w-16 text-right`}>{t('faults')}</th>
              <th className={`${th} w-20 text-right`}>{t('seconds')}</th>
            </>
          )}
          {points && <th className={`${th} w-14 text-right`}>{t('rank_points_short')}</th>}
        </tr>
      </thead>
      <tbody className="tabular">
        {ranking.lines.map(l => (
          <tr key={`${l.team}-${l.id}`} className={`${l.team ? 'bg-raised font-semibold' : ''} hover:bg-hover`}>
            <td className={`${td} text-right`}>
              {/* A rider under their team has the team's rank: set on the team. */}
              <RankCell line={l} classId={classId} editable={editable && (l.team || !teams)} />
              {l.draw && (
                <div className="mt-1">
                  <Pill tone="warn" title={t('draw_lots_hint')}>
                    {t('draw_lots')}
                  </Pill>
                </div>
              )}
            </td>
            <td className={`${td} max-w-0`}>
              <div className="truncate font-semibold text-ink">{l.rider}</div>
              {l.club && <div className="truncate text-xs text-ink-3">{l.club}</div>}
              {l.horse && <div className="truncate text-xs sm:hidden">{l.horse}</div>}
            </td>
            <td className={`${td} hidden max-w-0 truncate font-medium sm:table-cell`}>{l.horse}</td>
            {dressage ? (
              <>
                {judges.map(j => (
                  <td key={j} className={`${td} hidden text-right sm:table-cell`}>
                    {l.judges[j] ?? ''}
                  </td>
                ))}
                {parts && (
                  <>
                    <td className={`${td} hidden text-right sm:table-cell`}>{l.part_a ?? ''}</td>
                    <td className={`${td} hidden text-right sm:table-cell`}>{l.part_b ?? ''}</td>
                  </>
                )}
                <td className={`${td} text-right text-[14px] font-bold`}>
                  {l.percentage ?? (l.rank === null && !(teams && !l.team) ? <Pill tone="warn">{l.out ? reason(l.out) : t('out')}</Pill> : '')}
                </td>
              </>
            ) : (
              <>
                <td className={`${td} text-right text-[14px] font-bold`}>{l.rank === null && l.faults === null ? <Pill tone="warn">{l.out ? reason(l.out) : t('out')}</Pill> : (l.faults ?? '')}</td>
                <td className={`${td} text-right text-ink-2`}>{l.time === null ? '' : Number(l.time).toFixed(2)}</td>
              </>
            )}
            {points && <td className={`${td} text-right font-semibold`}>{l.rank_points ?? ''}</td>}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
