// How a class's start list is laid out: a plain list, blocks (each under a title), or start
// times with breaks; a plain list or blocks, ground care after the n-th starter (Rosson's
// « Entretien de la place après »: its line says « Entretien de la place » or what is typed). A dialog; applied as one change (undo brings the
// previous back).

import { useState } from 'react';
import { LayoutTemplate, Plus, X } from 'lucide-react';
import type { ClassRow, Layout, StartListSetup, StartListView } from '../api';
import { useT } from '../i18n';
import { Button, Callout, Field, IconButton, Modal, Segmented, Switch } from '../ui';
import { useChanges } from './changes';

const hhmm = (minutes: number) => `${String(Math.floor(minutes / 60) % 24).padStart(2, '0')}:${String(minutes % 60).padStart(2, '0')}`;
const minutesOf = (text: string): number | null => {
  const m = /^(\d{1,2})[:h.](\d{2})$/.exec(text.trim());
  if (!m || Number(m[1]) > 23 || Number(m[2]) > 59) return null;
  return Number(m[1]) * 60 + Number(m[2]);
};
type Kind = 'Plain' | 'Blocks' | 'StartTimes';
const kindOf = (l: Layout): Kind => (l === 'Plain' ? 'Plain' : 'Blocks' in l ? 'Blocks' : 'StartTimes');

/** The layouts a class can have, as Rosson offers them: blocks in dressage (classes and the
 *  dressage phases of eventing and driving); start times in dressage and the other phases of
 *  eventing and driving (jumping, cross, marathon, cones, derby, open — not the multitest); a
 *  plain list everywhere. A jumping class: a plain list, ground care after a starter. */
export function layoutKinds(row: ClassRow): Kind[] {
  const dressage = row.discipline === 'CD';
  const times = dressage || (row.phase !== null && row.phase !== 'multitest');
  return ['Plain', ...(dressage ? (['Blocks'] as Kind[]) : []), ...(times ? (['StartTimes'] as Kind[]) : [])];
}

/** What the layouts of a class are, said in a line (the menu's hint, the dialog's). */
export function layoutHint(row: ClassRow): 'layout_hint' | 'layout_hint_times' | 'layout_hint_plain' {
  const kinds = layoutKinds(row);
  return kinds.includes('Blocks') ? 'layout_hint' : kinds.includes('StartTimes') ? 'layout_hint_times' : 'layout_hint_plain';
}

export function LayoutEditor({ list, row, done }: { list: StartListView; row: ClassRow; done: () => void }) {
  const t = useT();
  const { change } = useChanges();
  const setup = list.setup;
  const blocks = typeof setup.layout === 'object' && 'Blocks' in setup.layout ? setup.layout.Blocks : { size: 10, headings: [], move_up: true };
  // New start times: every so many minutes as the dressage test lasts (rounded up), else 6.
  const times = typeof setup.layout === 'object' && 'StartTimes' in setup.layout ? setup.layout.StartTimes : { first: 8 * 60, minutes: list.programme_minutes ?? 6, breaks: [], move_up: false };
  const [kind, setKind] = useState<Kind>(kindOf(setup.layout));
  // The class's own, and the one it has (made elsewhere: it can be left).
  const kinds = layoutKinds(row);
  const offered = (['Plain', 'Blocks', 'StartTimes'] as Kind[]).filter(k => kinds.includes(k) || k === kindOf(setup.layout));
  const [size, setSize] = useState(String(blocks.size));
  const [headings, setHeadings] = useState<string[]>(blocks.headings);
  const [moveUpBlocks, setMoveUpBlocks] = useState(blocks.move_up);
  const [first, setFirst] = useState(hhmm(times.first));
  const [every, setEvery] = useState(String(times.minutes));
  const [breaks, setBreaks] = useState<[string, string][]>(times.breaks.map(([a, b]) => [hhmm(a), hhmm(b)]));
  const [moveUpTimes, setMoveUpTimes] = useState(times.move_up);
  const [after, setAfter] = useState(setup.line_after ? String(setup.line_after[0]) : '');
  const [text, setText] = useState(setup.line_after?.[1] ?? '');
  const [busy, setBusy] = useState(false);

  const starters = list.lines.filter(l => l.kind === 'starter' && !(moveUpBlocks && l.not_starting)).length;
  const blockCount = Math.max(1, Math.ceil(starters / Math.max(1, Number(size) || 1)));

  const built = (): StartListSetup | null => {
    let layout: Layout = 'Plain';
    if (kind === 'Blocks') {
      const n = Number(size);
      if (!Number.isInteger(n) || n < 1) return null;
      layout = { Blocks: { size: n, headings: headings.slice(0, blockCount), move_up: moveUpBlocks } };
    } else if (kind === 'StartTimes') {
      const start = minutesOf(first);
      const minutes = Number(every);
      const pauses = breaks.map(([a, b]) => [minutesOf(a), minutesOf(b)] as [number | null, number | null]);
      if (start === null || !Number.isInteger(minutes) || minutes < 1 || pauses.some(([a, b]) => a === null || b === null || a >= b)) return null;
      layout = { StartTimes: { first: start, minutes, breaks: pauses as [number, number][], move_up: moveUpTimes } };
    }
    // Start times have their breaks (by the clock): no ground care after a starter.
    const n = Number(after);
    const line: [number, string] | null = kind !== 'StartTimes' && after.trim() && Number.isInteger(n) && n > 0 ? [n, text.trim() || t('ground_care')] : null;
    return { layout, line_after: line };
  };
  const ready = built();
  const apply = async () => {
    if (!ready) return;
    setBusy(true);
    const ok = await change('set_layout', { class: list.class.id, setup: ready });
    setBusy(false);
    if (ok) done();
  };

  const input = 'field field-sm tabular';
  const bad = 'border-danger ring-1 ring-danger/30';
  return (
    <Modal
      open
      onClose={done}
      title={t('layout')}
      description={t(layoutHint(row))}
      icon={LayoutTemplate}
      size="lg"
      focusFirst={false}
      footer={
        <>
          <Button variant="ghost" onClick={done}>
            {t('cancel')}
          </Button>
          <Button variant="primary" loading={busy} disabled={!ready} onClick={apply}>
            {t('apply')}
          </Button>
        </>
      }
    >
      <div className="space-y-4 text-[13px]">
        {offered.length > 1 && (
          <Segmented value={kind} onChange={setKind} options={offered.map(k => ({ value: k, label: t(k === 'Plain' ? 'layout_plain' : k === 'Blocks' ? 'layout_blocks' : 'layout_times') }))} />
        )}

        {kind === 'Blocks' && (
          <div className="space-y-4 rounded-xl border border-line bg-raised p-4">
            <label className="flex items-center gap-2.5">
              <input value={size} onChange={e => setSize(e.target.value)} inputMode="numeric" className={`${input} w-16 text-right ${Number(size) >= 1 ? '' : bad}`} />
              <span className="text-ink-2">{t('per_block')}</span>
            </label>
            <div className="space-y-1.5">
              <div className="text-xs font-semibold text-ink-2">{t('block_titles')}</div>
              <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
                {Array.from({ length: blockCount }, (_, i) => (
                  <input key={i} value={headings[i] ?? ''} placeholder={`${i + 1}`} onChange={e => setHeadings(h => Object.assign([...h], { [i]: e.target.value }))} className={input} />
                ))}
              </div>
            </div>
            <Switch checked={moveUpBlocks} onChange={setMoveUpBlocks} label={t('move_up_blocks')} />
          </div>
        )}

        {kind === 'StartTimes' && (
          <div className="space-y-4 rounded-xl border border-line bg-raised p-4">
            <div className="flex flex-wrap items-end gap-3">
              <Field label={t('first_start')}>
                <input value={first} onChange={e => setFirst(e.target.value)} className={`${input} w-20 text-center ${minutesOf(first) === null ? bad : ''}`} />
              </Field>
              <Field label={t('interval_min')}>
                <input value={every} onChange={e => setEvery(e.target.value)} inputMode="numeric" className={`${input} w-20 text-right ${Number(every) >= 1 ? '' : bad}`} />
              </Field>
              {list.programme_minutes !== null && String(list.programme_minutes) !== every.trim() && (
                <Button size="sm" variant="subtle" onClick={() => setEvery(String(list.programme_minutes))}>
                  {t('every_test').replace('{n}', String(list.programme_minutes))}
                </Button>
              )}
              {list.programme_time && (
                <span className="pb-2 text-xs text-ink-3">
                  {t('programme_lasts')} {list.programme_time}
                </span>
              )}
            </div>
            <div className="space-y-1.5">
              <div className="text-xs font-semibold text-ink-2">{t('breaks')}</div>
              <div className="flex flex-wrap items-center gap-2">
                {breaks.map(([a, b], i) => {
                  const wrong = minutesOf(a) === null || minutesOf(b) === null || (minutesOf(a) ?? 0) >= (minutesOf(b) ?? 0);
                  return (
                    <span key={i} className={`inline-flex items-center gap-1 rounded-lg border bg-card py-0.5 pr-0.5 pl-1.5 ${wrong ? 'border-danger' : 'border-line'}`}>
                      <input value={a} onChange={e => setBreaks(bs => bs.map((x, j) => (j === i ? [e.target.value, x[1]] : x)))} className="w-12 bg-transparent text-center tabular outline-none" />–
                      <input value={b} onChange={e => setBreaks(bs => bs.map((x, j) => (j === i ? [x[0], e.target.value] : x)))} className="w-12 bg-transparent text-center tabular outline-none" />
                      <IconButton icon={X} label={t('remove')} size="sm" onClick={() => setBreaks(bs => bs.filter((_, j) => j !== i))} />
                    </span>
                  );
                })}
                <Button size="sm" icon={Plus} onClick={() => setBreaks(bs => [...bs, ['10:00', '10:30']])}>
                  {t('add_break').replace(/^\+\s*/, '')}
                </Button>
              </div>
            </div>
            <Switch checked={moveUpTimes} onChange={setMoveUpTimes} label={t('move_up_times')} />
          </div>
        )}

        {kind !== 'StartTimes' && (
          <Field label={t('line_after')}>
            <div className="flex gap-2">
              <input value={after} onChange={e => setAfter(e.target.value)} inputMode="numeric" className={`${input} w-16 text-right`} />
              <input value={text} onChange={e => setText(e.target.value)} placeholder={t('ground_care')} className={`${input} min-w-0 flex-1`} />
            </div>
          </Field>
        )}
        {!list.start_numbers && <Callout tone="info">{t('layout_note')}</Callout>}
      </div>
    </Modal>
  );
}
