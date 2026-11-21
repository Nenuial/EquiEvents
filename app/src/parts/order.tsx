// A class's start order made again (Rosson's "Reihenfolge anpassen" and the orders after another
// class): sorted by rider, horse or winning points; as in another class; after another class's
// ranking (the worst first); or the same order starting from a chosen starter. As another class:
// the same order or the reverse, then grouped and spread as always (ponies by size, a rider's
// horses apart: reversed, a rider starts with the horse that ended the other class). A dialog: pick
// one way, set it, apply; each is one change (undo brings the previous order back). Start
// numbers stay with their riders.

import { useState } from 'react';
import { ArrowUpDown } from 'lucide-react';
import type { ClassRow, StartListView } from '../api';
import { useT } from '../i18n';
import { Button, Choice, Modal, Segmented } from '../ui';
import { useChanges } from './changes';

type Sort = 'name' | 'horse' | 'winning_points';
type Way = 'sort' | 'follow' | 'ranking' | 'from';

export function OrderEditor({ list, classes, done }: { list: StartListView; classes: ClassRow[]; done: () => void }) {
  const t = useT();
  const { change } = useChanges();
  const id = list.class.id;
  const others = classes.filter(c => c.id !== id);
  const ranked = others.filter(c => c.results > 0);
  const starters = list.lines.filter(l => l.kind === 'starter');
  const [way, setWay] = useState<Way>('sort');
  const [sort, setSort] = useState<Sort>('name');
  const [descending, setDescending] = useState(false);
  const [follow, setFollow] = useState(others[0]?.id ?? '');
  // Following another class: most often in the reverse order.
  const [reverse, setReverse] = useState(true);
  const [after, setAfter] = useState(ranked[0]?.id ?? '');
  const [from, setFrom] = useState(starters[1]?.kind === 'starter' ? String(starters[1].entry) : '');
  const [busy, setBusy] = useState(false);

  const args = (): Record<string, unknown> | null => {
    if (way === 'sort') return { by: sort, descending };
    if (way === 'follow') return follow ? { by: 'follow', other: follow, descending: reverse } : null;
    if (way === 'ranking') return after ? { by: 'ranking', other: after } : null;
    return from ? { by: 'from', entry: Number(from) } : null;
  };
  const run = async () => {
    const a = args();
    if (!a) return;
    setBusy(true);
    const ok = await change('reorder', { class: id, ...a });
    setBusy(false);
    if (ok) done();
  };
  const select = 'field field-sm w-auto min-w-0 max-w-full';
  const label = (c: ClassRow) => `${c.number} · ${c.title}`;

  return (
    <Modal
      open
      onClose={done}
      title={t('order')}
      description={t('order_hint')}
      icon={ArrowUpDown}
      size="md"
      focusFirst={false}
      footer={
        <>
          <Button variant="ghost" onClick={done}>
            {t('cancel')}
          </Button>
          <Button variant="primary" icon={ArrowUpDown} loading={busy} disabled={!args()} onClick={run}>
            {t('apply')}
          </Button>
        </>
      }
    >
      <div role="radiogroup" className="space-y-2.5">
        <Choice selected={way === 'sort'} onSelect={() => setWay('sort')} title={t('order_sort')}>
          <div className="flex flex-wrap gap-2">
            <select value={sort} onChange={e => setSort(e.target.value as Sort)} className={select}>
              <option value="name">{t('order_name')}</option>
              <option value="horse">{t('order_horse')}</option>
              <option value="winning_points">{t('order_points')}</option>
            </select>
            <select value={descending ? 'desc' : 'asc'} onChange={e => setDescending(e.target.value === 'desc')} className={select}>
              <option value="asc">{t('ascending')}</option>
              <option value="desc">{t('descending')}</option>
            </select>
          </div>
        </Choice>

        {others.length > 0 && (
          <Choice selected={way === 'follow'} onSelect={() => setWay('follow')} title={t('order_follow')} hint={t('order_follow_hint')}>
            <div className="grid gap-2">
              <select value={follow} onChange={e => setFollow(e.target.value)} className={`${select} w-full`}>
                {others.map(c => (
                  <option key={c.id} value={c.id}>
                    {label(c)}
                  </option>
                ))}
              </select>
              <Segmented
                size="sm"
                className="justify-self-start"
                value={reverse ? 'reverse' : 'same'}
                onChange={v => setReverse(v === 'reverse')}
                options={[
                  { value: 'reverse', label: t('order_reverse') },
                  { value: 'same', label: t('order_same') },
                ]}
              />
            </div>
          </Choice>
        )}

        {ranked.length > 0 && (
          <Choice selected={way === 'ranking'} onSelect={() => setWay('ranking')} title={t('order_ranking')} hint={t('order_ranking_hint')}>
            <select value={after} onChange={e => setAfter(e.target.value)} className={`${select} w-full`}>
              {ranked.map(c => (
                <option key={c.id} value={c.id}>
                  {label(c)}
                </option>
              ))}
            </select>
          </Choice>
        )}

        {starters.length > 1 && (
          <Choice selected={way === 'from'} onSelect={() => setWay('from')} title={t('order_from')} hint={t('order_from_hint')}>
            <select value={from} onChange={e => setFrom(e.target.value)} className={`${select} w-full`}>
              {starters.map(l =>
                l.kind === 'starter' ? (
                  <option key={l.entry} value={l.entry}>
                    {l.position}. {l.start_number ? `${l.start_number} · ` : ''}
                    {l.rider} – {l.horse}
                  </option>
                ) : null,
              )}
            </select>
          </Choice>
        )}
      </div>
    </Modal>
  );
}
