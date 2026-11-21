// Prints and Excel files: made by the office core and opened (prints) or saved where the office
// chooses (Excel). From a menu or a button; what is under way and what went wrong in a toast.
// Labels and letters ask a little first, in a dialog.

import { useCallback, useState } from 'react';
import { FileSpreadsheet, Flag, Mail, Pencil, Printer, Tags } from 'lucide-react';
import { call, openPrint, saveFile } from '../api';
import { useLanguage, useT } from '../i18n';
import { Button, Choice, Field, IconButton, Modal, SearchSelect, Segmented, useToast } from '../ui';
import { useLoad } from './common';
import { LabelSheetEditor, type LabelSheet } from './labelsheets';

/** Make a print and open it; under way and errors in a toast. */
export function usePrint(show: string) {
  const t = useT();
  const language = useLanguage();
  const toast = useToast();
  return useCallback(
    async (args: Record<string, unknown>) => {
      toast.show({ text: t('printing'), tone: 'busy' });
      try {
        await openPrint({ show, language, ...args });
        toast.clear();
      } catch (e) {
        toast.show({ text: String((e as Error).message ?? e), tone: 'error' });
      }
    },
    [show, language, toast, t],
  );
}

/** Make an Excel file and save it where the office chooses. */
export function useExcel(show: string) {
  const t = useT();
  const language = useLanguage();
  const toast = useToast();
  return useCallback(
    async (args: Record<string, unknown>, file: string) => {
      try {
        const saved = await saveFile(`${file}.xlsx`, 'xlsx', path => {
          toast.show({ text: t('excel_making'), tone: 'busy' });
          return call('export_excel', { show, language, path, ...args });
        });
        if (saved) toast.show({ text: `${t('excel_saved')} · ${saved.split(/[\\/]/).pop()}`, tone: 'done' });
      } catch (e) {
        toast.show({ text: String((e as Error).message ?? e), tone: 'error' });
      }
    },
    [show, language, toast, t],
  );
}

/** A button for one print. */
export function PrintButton({ show, args, label, size = 'md' }: { show: string; args: Record<string, unknown>; label: string; size?: 'sm' | 'md' }) {
  const print = usePrint(show);
  return (
    <Button size={size} icon={Printer} onClick={() => print(args)}>
      {label}
    </Button>
  );
}

/** A button for one Excel file. */
export function ExcelButton({ show, args, file, label, size = 'md' }: { show: string; args: Record<string, unknown>; file: string; label: string; size?: 'sm' | 'md' }) {
  const excel = useExcel(show);
  return (
    <Button size={size} icon={FileSpreadsheet} onClick={() => excel(args, file)}>
      {label}
    </Button>
  );
}

/** Labels on a sheet: which sheet (Zweckform 3475 or 3484, or the office's own), how many places
 *  of it are used already. */
/** Who can have a start card: a starter (its entry, "12 · Rider", the horse) or, for team
 *  cards, a team (its id, "1 · Team", its riders). */
export interface CardStarter {
  entry: number | string;
  label: string;
  detail: string;
}

/** Start cards: the whole class or one starter's (a late entry, a card lost); two to an A4 page,
 *  to cut in two, or one to an A5 page (kept as the office last chose). */
export function StartCardsModal({
  show,
  classId,
  starters,
  open,
  onClose,
  teams = false,
}: {
  show: string;
  classId: string;
  starters: CardStarter[];
  open: boolean;
  onClose: () => void;
  /** Team cards: a page a team (A4, or the same card on A5); every team or one. */
  teams?: boolean;
}) {
  const t = useT();
  const print = usePrint(show);
  const [paper, setPaper] = useState<'a4' | 'a5'>(() => {
    try {
      return localStorage.getItem('card-paper') === 'a5' ? 'a5' : 'a4';
    } catch {
      return 'a4';
    }
  });
  const [who, setWho] = useState<'all' | 'one'>('all');
  const [entry, setEntry] = useState<number | string | null>(null);
  const choosePaper = (p: 'a4' | 'a5') => {
    setPaper(p);
    try {
      localStorage.setItem('card-paper', p);
    } catch {
      /* a preference only */
    }
  };
  const one = who === 'one';
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t(teams ? 'print_team_cards' : 'print_start_cards')}
      icon={Flag}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button
            variant="primary"
            icon={Printer}
            disabled={one && entry === null}
            onClick={() => {
              onClose();
              const which = one && entry !== null ? (teams ? { team: entry } : { entry }) : {};
              print({ what: teams ? 'team_cards' : 'start_cards', class: classId, paper, ...which });
            }}
          >
            {t('print')}
          </Button>
        </>
      }
    >
      <div className="grid gap-5">
        <Field label={t('cards_paper')} group>
          <div className="grid gap-2 sm:grid-cols-2">
            <Choice selected={paper === 'a4'} onSelect={() => choosePaper('a4')} title="A4" hint={t(teams ? 'cards_a4_team_hint' : 'cards_a4_hint')} />
            <Choice selected={paper === 'a5'} onSelect={() => choosePaper('a5')} title="A5" hint={t(teams ? 'cards_a5_team_hint' : 'cards_a5_hint')} />
          </div>
        </Field>
        <Field label={t('cards_who')} group>
          <div className="grid gap-2">
            <Choice selected={!one} onSelect={() => setWho('all')} title={t('cards_all')} hint={t('cards_all_hint').replace('{n}', String(starters.length))} />
            <Choice selected={one} onSelect={() => setWho('one')} title={t(teams ? 'cards_one_team' : 'cards_one')}>
              <SearchSelect
                value={entry}
                options={starters.map(s => ({ value: s.entry, label: s.label, detail: s.detail }))}
                onChange={v => {
                  setEntry(v);
                  setWho('one');
                }}
                placeholder={t(teams ? 'cards_pick_team' : 'cards_pick')}
                search={t(teams ? 'cards_search_team' : 'cards_search')}
                empty={t('no_match')}
              />
            </Choice>
          </div>
        </Field>
      </div>
    </Modal>
  );
}

export function LabelsModal({ show, args, title, open, onClose }: { show: string; args: Record<string, unknown>; title: string; open: boolean; onClose: () => void }) {
  const t = useT();
  const print = usePrint(show);
  const [sheet, setSheet] = useState(() => {
    try {
      return localStorage.getItem('label-sheet') ?? '3475';
    } catch {
      return '3475';
    }
  });
  const [skip, setSkip] = useState('0');
  // The sheets: Zweckform's and the office's own; one being described.
  const [round, setRound] = useState(0);
  const sheets = useLoad(() => (open ? call<{ builtin: boolean; sheet: LabelSheet }[]>('label_sheets') : Promise.resolve(null)), [open, round]);
  const [editing, setEditing] = useState<LabelSheet | null>(null);
  const choose = (id: string) => {
    setSheet(id);
    try {
      localStorage.setItem('label-sheet', id);
    } catch {
      /* a preference only */
    }
  };
  const current = sheets.data?.find(s => s.sheet.id === sheet);
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      icon={Tags}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button
            variant="primary"
            icon={Printer}
            onClick={() => {
              onClose();
              print({ what: 'labels', sheet, skip: Number(skip) || 0, ...args });
            }}
          >
            {t('print')}
          </Button>
        </>
      }
    >
      <div className="grid gap-4">
        <Field label={t('label_sheet')}>
          <div className="flex gap-2">
            <select
              value={current ? sheet : '3475'}
              onChange={e => {
                if (e.target.value === '+') {
                  const base = (current ?? sheets.data?.[0])?.sheet;
                  if (base) setEditing({ ...base, id: '', name: '' });
                } else choose(e.target.value);
              }}
              className="field"
            >
              {sheets.data?.map(s => (
                <option key={s.sheet.id} value={s.sheet.id}>
                  {s.sheet.name}
                </option>
              ))}
              <option value="+">{t('sheet_own_new')}</option>
            </select>
            {current && !current.builtin && <IconButton icon={Pencil} label={t('sheet_own')} variant="secondary" onClick={() => setEditing(current.sheet)} />}
          </div>
        </Field>
        <Field label={t('labels_skip')}>
          <input value={skip} onChange={e => setSkip(e.target.value)} inputMode="numeric" className="field w-28 text-right" />
        </Field>
      </div>
      {editing && (
        <LabelSheetEditor
          start={editing}
          done={saved => {
            setEditing(null);
            setRound(r => r + 1);
            if (saved) choose(saved.id);
          }}
        />
      )}
    </Modal>
  );
}

/** Letters to the riders: the office's text (remembered on this computer), the envelope's window. */
export function LettersModal({ show, open, onClose }: { show: string; open: boolean; onClose: () => void }) {
  const t = useT();
  const print = usePrint(show);
  const key = `letter-text:${show}`;
  const [text, setText] = useState(() => {
    try {
      return localStorage.getItem(key) ?? '';
    } catch {
      return '';
    }
  });
  const [window, setWindow] = useState<'left' | 'right'>('right');
  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t('letters')}
      icon={Mail}
      size="md"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('cancel')}
          </Button>
          <Button
            variant="primary"
            icon={Printer}
            onClick={() => {
              onClose();
              print({ what: 'letters', text, window });
            }}
          >
            {t('print')}
          </Button>
        </>
      }
    >
      <div className="grid gap-4">
        <Field label={t('letters_text')}>
          <textarea
            value={text}
            onChange={e => {
              setText(e.target.value);
              try {
                localStorage.setItem(key, e.target.value);
              } catch {
                /* remembered only when the browser lets us */
              }
            }}
            rows={7}
            className="field"
          />
        </Field>
        <Field label={t('letters_window')}>
          <Segmented
            value={window}
            onChange={setWindow}
            options={[
              { value: 'left', label: t('window_left') },
              { value: 'right', label: t('window_right') },
            ]}
          />
        </Field>
      </div>
    </Modal>
  );
}
