// A sheet of sticky labels the office describes (Rosson's custom templates), with a live A4
// preview: how many across and down, each label's size, the page's margins, the gaps between
// labels, the space inside, the text's size and alignment. Kept with this computer's library.

import { useState } from 'react';
import { call } from '../api';
import { useT } from '../i18n';
import { Drawer } from './online';

export interface LabelSheet {
  id: string;
  name: string;
  cols: number;
  rows: number;
  width_mm: number;
  height_mm: number;
  top_mm: number;
  left_mm: number;
  gap_x_mm: number;
  gap_y_mm: number;
  padding_mm: number;
  font_pt: number;
  align: 'left' | 'center' | 'right';
}

const numeric: (keyof LabelSheet)[] = ['cols', 'rows', 'width_mm', 'height_mm', 'top_mm', 'left_mm', 'gap_x_mm', 'gap_y_mm', 'padding_mm', 'font_pt'];

/** The editor, in a panel; `done` with the sheet saved (or null: closed, or removed). */
export function LabelSheetEditor({ start, done }: { start: LabelSheet; done: (saved: LabelSheet | null) => void }) {
  const t = useT();
  const own = start.id.startsWith('own-');
  const [form, setForm] = useState<Record<string, string>>(() => Object.fromEntries(Object.entries(start).map(([k, v]) => [k, String(v)])));
  const [error, setError] = useState<string | null>(null);
  const sheet = {
    ...start,
    ...Object.fromEntries(numeric.map(k => [k, Number(String(form[k]).replace(',', '.'))])),
    name: form.name,
    align: form.align as LabelSheet['align'],
    id: own ? start.id : '',
  } as LabelSheet;
  const valid = numeric.every(k => Number.isFinite(sheet[k] as number)) && form.name.trim() !== '';
  const across = sheet.left_mm + sheet.cols * sheet.width_mm + (sheet.cols - 1) * sheet.gap_x_mm;
  const down = sheet.top_mm + sheet.rows * sheet.height_mm + (sheet.rows - 1) * sheet.gap_y_mm;
  const fits = across <= 210.5 && down <= 297.5;

  const save = async () => {
    setError(null);
    try {
      done(await call<LabelSheet>('save_label_sheet', { sheet }));
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
    }
  };
  const remove = async () => {
    await call('remove_label_sheet', { id: start.id });
    done(null);
  };

  const field = (key: keyof LabelSheet, label: string, unit = 'mm') => (
    <label className="flex items-center justify-between gap-2 text-sm">
      <span className="text-ink-2">{label}</span>
      <span className="flex items-center gap-1">
        <input value={form[key]} onChange={e => setForm(f => ({ ...f, [key]: e.target.value }))} inputMode="decimal" className="field field-sm w-16 text-right tabular" />
        <span className="w-6 text-xs text-ink-3">{unit}</span>
      </span>
    </label>
  );

  // The preview: an A4 page at scale, its labels.
  const scale = 0.85;
  return (
    <Drawer title={t('sheet_own')} close={() => done(null)}>
      <div className="grid gap-4">
        <div className="justify-self-center">
          <div className="relative overflow-hidden border border-line bg-white shadow-sm" style={{ width: 210 * scale, height: 297 * scale }} aria-hidden="true">
            {valid &&
              Array.from({ length: Math.min(sheet.cols * sheet.rows, 600) }, (_, i) => {
                const col = i % sheet.cols;
                const row = Math.floor(i / sheet.cols);
                return (
                  <div
                    key={i}
                    className="absolute overflow-hidden rounded-[2px] border border-accent/60 bg-accent/5 text-[5px] leading-tight text-ink-3"
                    style={{
                      left: (sheet.left_mm + col * (sheet.width_mm + sheet.gap_x_mm)) * scale,
                      top: (sheet.top_mm + row * (sheet.height_mm + sheet.gap_y_mm)) * scale,
                      width: sheet.width_mm * scale,
                      height: sheet.height_mm * scale,
                      padding: sheet.padding_mm * scale,
                      textAlign: sheet.align,
                    }}
                  >
                    <b>Marie Exemple</b>
                    <br />
                    Rue du Lac 1
                  </div>
                );
              })}
          </div>
          <div className={`mt-1 text-center text-xs ${fits ? 'text-ink-3' : 'font-semibold text-accent-dark'}`}>
            {Math.round(across)} × {Math.round(down)} mm {fits ? '' : `· ${t('sheet_too_big')}`}
          </div>
        </div>
        <div className="grid content-start gap-2">
          <label className="grid gap-1 text-sm font-semibold">
            {t('sheet_name')}
            <input value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} className="field" />
          </label>
          {field('cols', t('sheet_cols'), '')}
          {field('rows', t('sheet_rows'), '')}
          {field('width_mm', t('sheet_width'))}
          {field('height_mm', t('sheet_height'))}
          {field('top_mm', t('sheet_top'))}
          {field('left_mm', t('sheet_left'))}
          {field('gap_x_mm', t('sheet_gap_x'))}
          {field('gap_y_mm', t('sheet_gap_y'))}
          {field('padding_mm', t('sheet_padding'))}
          {field('font_pt', t('sheet_font'), 'pt')}
          <label className="flex items-center justify-between gap-2 text-sm">
            <span className="text-ink-2">{t('sheet_align')}</span>
            <select value={form.align} onChange={e => setForm(f => ({ ...f, align: e.target.value }))} className="rounded border border-line bg-page px-1.5 py-1">
              <option value="left">{t('sheet_align_left')}</option>
              <option value="center">{t('sheet_align_center')}</option>
              <option value="right">{t('sheet_align_right')}</option>
            </select>
          </label>
        </div>
      </div>
      {error && <p className="mt-3 text-sm font-semibold text-accent-dark">{error}</p>}
      <div className="mt-4 flex flex-wrap gap-2">
        <button disabled={!valid || !fits} onClick={save} className="btn btn-md btn-primary disabled:opacity-50">
          {t('sheet_save')}
        </button>
        {own && (
          <button onClick={remove} className="btn btn-md btn-secondary">
            {t('sheet_remove')}
          </button>
        )}
      </div>
    </Drawer>
  );
}
