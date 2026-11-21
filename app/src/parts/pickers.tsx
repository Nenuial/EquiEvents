// A class's scoring table (jumping) and dressage test, each picked from a searchable list (the
// user, 2026-11-15): in the class's settings, and where its results are entered.

import { call } from '../api';
import { useLanguage, useT, type Language } from '../i18n';
import { SearchSelect } from '../ui';
import { useLoad } from './common';

export interface ScoringTableChoice {
  id: number;
  code: string;
  name: string;
  name_fr?: string;
  name_it?: string;
}

const tableName = (s: ScoringTableChoice, language: Language) => (language === 'fr' && s.name_fr) || (language === 'it' && s.name_it) || s.name;

/** A class's scoring table, among the show's: changed only before it has results. */
export function ScoringTablePicker({ value, tables, onChange, disabled, id }: { value: number | null; tables: ScoringTableChoice[]; onChange: (id: number) => void; disabled?: boolean; id?: string }) {
  const t = useT();
  const language = useLanguage();
  return (
    <SearchSelect
      id={id}
      value={value}
      options={tables.map(s => ({ value: s.id, label: s.code, detail: tableName(s, language) }))}
      onChange={onChange}
      disabled={disabled}
      search={t('search_scoring')}
      empty={t('no_match')}
    />
  );
}

/** A class's dressage test, among those of its discipline: changed only before marks. */
export function ProgrammePicker({
  show,
  classId,
  value,
  onChange,
  disabled,
  id,
}: {
  show: string;
  classId: string;
  value: number | null;
  onChange: (id: number) => void;
  disabled?: boolean;
  id?: string;
}) {
  const t = useT();
  const language = useLanguage();
  const programmes = useLoad(() => call<{ id: number; name: string; title: string; titel: string; titolo: string }[]>('programmes', { show, class: classId }), [show, classId]);
  const title = (p: { title: string; titel: string; titolo: string }) => (language === 'de' ? p.titel : language === 'it' ? p.titolo || p.titel : p.title);
  return (
    <SearchSelect
      id={id}
      value={value}
      options={(programmes.data ?? []).map(p => ({ value: p.id, label: p.name, detail: title(p) }))}
      onChange={onChange}
      disabled={disabled}
      search={t('search_programme')}
      empty={t('no_match')}
    />
  );
}
