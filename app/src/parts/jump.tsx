// The start-number quick jump of the results screens: type a start number (or, without
// numbers, a place in the start order), Enter; ⌘J / Ctrl+J reaches it from anywhere.

import { useEffect, useRef, useState } from 'react';
import { useT } from '../i18n';

/** A row to jump to: by its start number, else by its place in the start order. */
export function findStarter<R extends { start_number: number | null; position: number }>(rows: R[], n: number): R | undefined {
  return rows.find(r => r.start_number === n) ?? (rows.some(r => r.start_number !== null) ? undefined : rows.find(r => r.position === n));
}

export function QuickJump({ onJump }: { onJump: (n: number) => boolean }) {
  const t = useT();
  const [text, setText] = useState('');
  const [miss, setMiss] = useState(false);
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'j') {
        e.preventDefault();
        field.current?.focus();
        field.current?.select();
      }
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, []);
  return (
    <input
      ref={field}
      value={text}
      inputMode="numeric"
      enterKeyHint="go"
      placeholder={t('jump_to')}
      title={`${t('jump_hint')} (⌘J)`}
      aria-label={t('jump_hint')}
      onChange={e => {
        setText(e.target.value.replace(/\D/g, ''));
        setMiss(false);
      }}
      onKeyDown={e => {
        if (e.key === 'Enter' && text) {
          e.preventDefault();
          const found = onJump(Number(text));
          setMiss(!found);
          if (found) setText('');
        } else if (e.key === 'Escape') {
          setText('');
          setMiss(false);
        }
      }}
      className={`field field-sm w-28 tabular ${miss ? 'border-danger bg-danger/8' : ''}`}
    />
  );
}
