// A button for a change that takes much away (results, start lists, a ranking split): asked
// twice, in place (no dialog; undo still brings everything back).

import { useState } from 'react';
import { useT } from '../i18n';

const tool = 'btn btn-sm btn-secondary pointer-coarse:py-2';

export function ConfirmButton({ label, ask, title, onConfirm, danger = false }: { label: string; ask: string; title?: string; onConfirm: () => void; danger?: boolean }) {
  const t = useT();
  const [asking, setAsking] = useState(false);
  if (!asking)
    return (
      <button onClick={() => setAsking(true)} title={title} className={`${tool} ${danger ? 'text-accent' : ''}`}>
        {label}
      </button>
    );
  return (
    <span className="inline-flex flex-wrap items-center gap-1 text-xs">
      {ask}
      <button
        autoFocus
        onClick={() => {
          setAsking(false);
          onConfirm();
        }}
        className="rounded-md bg-accent px-2 py-1 font-semibold text-on-accent hover:bg-accent-dark pointer-coarse:py-2"
      >
        {t('yes')}
      </button>
      <button onClick={() => setAsking(false)} className={tool}>
        {t('cancel')}
      </button>
    </span>
  );
}
