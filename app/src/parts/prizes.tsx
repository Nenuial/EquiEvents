// A published class's prizes (Rosson's "Preisgeld"): an amount and a prize in kind per rank,
// typed by hand. In show jumping only, computed first from the first prize as Rosson does
// (rounded up to 5 CHF, 80 % for the next rank, never below the minimum: (fee − 18 CHF tax) × 2):
// the other disciplines have no such scale. On the prize labels, never on the ranking. A dialog.

import { useEffect, useState } from 'react';
import { Calculator, Medal } from 'lucide-react';
import { call } from '../api';
import { useT } from '../i18n';
import { Button, Field, Modal } from '../ui';
import { useChanges } from './changes';

interface Prizes {
  minimum: string;
  amounts: string[];
  natural: string[];
}

const input = 'field field-sm tabular';
const bad = 'border-danger ring-1 ring-danger/30';
const isAmount = (s: string) => s.trim() === '' || /^\d+([.,]\d{1,2})?$/.test(s.trim());

export function PrizeEditor({ show, published, title, placed, done }: { show: string; published: number; title: string; placed: number; done: () => void }) {
  const t = useT();
  const { change } = useChanges();
  const [loaded, setLoaded] = useState<{ set: boolean; fee: string | null; scale: boolean } | null>(null);
  const [minimum, setMinimum] = useState('0');
  const [first, setFirst] = useState('');
  const [ranks, setRanks] = useState(String(Math.max(placed, 3)));
  const [amounts, setAmounts] = useState<string[]>([]);
  const [natural, setNatural] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    call<{ prizes: Prizes; set: boolean; fee: string | null; scale: boolean }>('prizes', { show, published }).then(p => {
      setLoaded({ set: p.set, fee: p.fee, scale: p.scale });
      setMinimum(p.prizes.minimum);
      setAmounts(p.prizes.amounts);
      setNatural(p.prizes.natural);
      if (p.prizes.amounts.length) setRanks(String(p.prizes.amounts.length));
      setFirst(p.prizes.amounts[0] ?? '');
    });
  }, [show, published]);
  const n = Math.min(30, Math.max(1, Number(ranks) || 1));
  const rows = Array.from({ length: n }, (_, i) => i);
  const valid = isAmount(minimum) && amounts.every(isAmount);
  const ranksField = (
    <Field label={t('prize_ranks')}>
      <input value={ranks} onChange={e => setRanks(e.target.value)} inputMode="numeric" className={`${input} w-16 text-right`} />
    </Field>
  );
  const compute = async () => setAmounts(await call<string[]>('prize_scale', { show, first: first.replace(',', '.'), minimum: minimum.replace(',', '.'), ranks: n }));
  const save = async (prizes: Prizes | null) => {
    setBusy(true);
    const ok = await change('set_prizes', { published, prizes });
    setBusy(false);
    if (ok) done();
  };
  return (
    <Modal
      open
      onClose={done}
      title={title}
      icon={Medal}
      size="lg"
      focusFirst={false}
      footer={
        loaded && (
          <>
            {loaded.set && (
              <Button variant="danger" className="mr-auto" disabled={busy} onClick={() => save(null)}>
                {t('prize_none')}
              </Button>
            )}
            <Button variant="ghost" onClick={done}>
              {t('cancel')}
            </Button>
            <Button
              variant="primary"
              loading={busy}
              disabled={!valid}
              onClick={() =>
                save({
                  minimum: minimum.trim().replace(',', '.') || '0',
                  amounts: Array.from({ length: n }, (_, i) => (amounts[i] ?? '').trim().replace(',', '.') || '0'),
                  natural: Array.from({ length: n }, (_, i) => natural[i] ?? ''),
                })
              }
            >
              {t('save')}
            </Button>
          </>
        )
      }
    >
      {!loaded ? (
        <div className="h-40 animate-pulse rounded-xl bg-sunken" />
      ) : (
        <div className="space-y-4 text-[13px]">
          {loaded.scale ? (
            // Show jumping's scale: from the first prize, down to the minimum.
            <div className="space-y-3 rounded-xl border border-line bg-raised p-4">
              <div className="flex flex-wrap items-end gap-3">
                <Field label={t('prize_first')}>
                  <input value={first} onChange={e => setFirst(e.target.value)} inputMode="decimal" className={`${input} w-24 text-right ${isAmount(first) ? '' : bad}`} />
                </Field>
                <Field label={t('prize_minimum')}>
                  <input value={minimum} onChange={e => setMinimum(e.target.value)} inputMode="decimal" className={`${input} w-24 text-right ${isAmount(minimum) ? '' : bad}`} />
                </Field>
                {ranksField}
                <Button icon={Calculator} disabled={!first.trim() || !isAmount(first) || !isAmount(minimum)} onClick={compute}>
                  {t('prize_compute')}
                </Button>
              </div>
              <p className="text-xs text-ink-3">
                {t('prize_rule')}
                {loaded.fee && ` ${t('prize_fee')} ${loaded.fee}.`}
              </p>
            </div>
          ) : (
            // Other disciplines: how many ranks, each prize typed.
            ranksField
          )}
          {/* Rank by rank: the amount, a prize in kind. */}
          <div className="overflow-hidden rounded-xl border border-line">
            {rows.map(i => (
              <div key={i} className="grid grid-cols-[3rem_7rem_1fr] items-center gap-2 border-t border-line px-3 py-1.5 first:border-t-0">
                <span className="text-right text-xs font-semibold text-ink-3 tabular">{i + 1}.</span>
                <input
                  value={amounts[i] ?? ''}
                  onChange={e => setAmounts(a => Object.assign([...a], { [i]: e.target.value }))}
                  inputMode="decimal"
                  aria-label={`${i + 1}.`}
                  className={`${input} text-right ${isAmount(amounts[i] ?? '') ? '' : bad}`}
                />
                <input value={natural[i] ?? ''} onChange={e => setNatural(a => Object.assign([...a], { [i]: e.target.value }))} placeholder={t('prize_natural')} className={`${input} min-w-0`} />
              </div>
            ))}
          </div>
        </div>
      )}
    </Modal>
  );
}
