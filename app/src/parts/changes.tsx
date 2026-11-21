// Changing a show: every change goes through here. Views reload after it (`version`), the
// office sees what was done (a toast), ⌘Z undoes the last change and ⌘⇧Z (Ctrl+Y) brings back
// what was just undone.

import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from 'react';
import { Redo2, Undo2 } from 'lucide-react';
import { call } from '../api';
import { useChangeName, useT } from '../i18n';
import { useToast } from '../ui';

/** What a change command answers: what can be undone next, and what the command adds. */
export type ChangeAnswer = { undo?: string | null; unchanged?: boolean } & Record<string, unknown>;

interface Changes {
  /** Grows with every change (also changes from equievents.ch): views load again when it does. */
  version: number;
  /** Grows with every change made here: the sync sends them. */
  changed: number;
  /** Views load again (changes came from equievents.ch). */
  refresh: () => void;
  /** Run a change command: its answer, or false if it was refused (the reason is shown). */
  change: (command: string, args: Record<string, unknown>) => Promise<ChangeAnswer | false>;
  undo: () => void;
  redo: () => void;
}

const ChangesContext = createContext<Changes>({ version: 0, changed: 0, refresh: () => {}, change: async () => false, undo: () => {}, redo: () => {} });

export const useChanges = () => useContext(ChangesContext);

export function ChangesProvider({ show, children }: { show: string; children: ReactNode }) {
  const t = useT();
  const name = useChangeName();
  // `show` alone: it stays the same from one render to the next.
  const { show: note } = useToast();
  const [version, setVersion] = useState(0);
  const [changed, setChanged] = useState(0);
  const refresh = useCallback(() => setVersion(v => v + 1), []);

  // Undo and redo: each one's toast offers the other.
  const step = useCallback(
    async (how: 'undo' | 'redo') => {
      try {
        const answer = await call<{ undone?: string; redone?: string; undo: string | null; redo: string | null }>(how, { show });
        setVersion(v => v + 1);
        setChanged(c => c + 1);
        if (how === 'undo') {
          // Just undone: it can come back.
          note({ text: `${t('undone')}${t('colon')}${name(answer.undone ?? '')}`, icon: Undo2, action: { label: `${t('redo')} ⌘⇧Z`, title: '⌘⇧Z', onClick: () => step('redo') } });
        } else {
          note({ text: `${t('redone')}${t('colon')}${name(answer.redone ?? '')}`, icon: Redo2, action: { label: `${t('undo')} ⌘Z`, title: '⌘Z', onClick: () => step('undo') } });
        }
      } catch {
        note({ text: t(how === 'undo' ? 'nothing_to_undo' : 'nothing_to_redo'), tone: 'info' });
      }
    },
    [show, t, name, note],
  );
  const undo = useCallback(() => step('undo'), [step]);
  const redo = useCallback(() => step('redo'), [step]);

  const change = useCallback(
    async (command: string, args: Record<string, unknown>) => {
      try {
        const answer = await call<ChangeAnswer>(command, { show, ...args });
        // Nothing changed (a mark typed again as it was): nothing to say.
        if (answer.unchanged) return answer;
        setVersion(v => v + 1);
        setChanged(c => c + 1);
        note({ text: `${t('done')}${t('colon')}${name(answer.undo ?? command)}`, action: answer.undo ? { label: `${t('undo')} ⌘Z`, title: '⌘Z', onClick: undo } : undefined });
        return answer;
      } catch (e) {
        note({ text: String((e as Error)?.message ?? e), tone: 'error' });
        return false;
      }
    },
    [show, t, name, note, undo],
  );

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey) return;
      // In a field, the field's own undo.
      const target = e.target as HTMLElement | null;
      if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target?.isContentEditable) return;
      const k = e.key.toLowerCase();
      if (k === 'z' && !e.shiftKey) {
        e.preventDefault();
        undo();
      } else if ((k === 'z' && e.shiftKey) || (k === 'y' && e.ctrlKey && !e.metaKey)) {
        e.preventDefault();
        redo();
      }
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [undo, redo]);

  return <ChangesContext.Provider value={{ version, changed, refresh, change, undo, redo }}>{children}</ChangesContext.Provider>;
}
