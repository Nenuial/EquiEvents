// The show's documents in one place: what is printed for the whole show (the timetable, the
// participants, the entries, each day's start lists), labels and letters, and the Excel files.
// Each is a tile: a click makes it. A class's own prints are on its page.

import { useState, type ComponentType, type ReactNode } from 'react';
import { ArrowRight, CalendarDays, FileSpreadsheet, ListChecks, Mail, Printer, Tags, Users } from 'lucide-react';
import type { ShowView } from '../api';
import { formatDate, useLanguage, usePlural, useT } from '../i18n';
import { LabelsModal, LettersModal, useExcel, usePrint } from '../parts/print';
import { PageHeader } from '../ui';

type Icon = ComponentType<{ className?: string }>;

/** A group of documents: its name, then its tiles. */
function Section({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <section className="space-y-3">
      <div className="px-1">
        <h2 className="text-[15px] font-semibold tracking-tight text-ink">{title}</h2>
        {hint && <p className="text-xs text-ink-3">{hint}</p>}
      </div>
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">{children}</div>
    </section>
  );
}

/** One document, as a tile: what it is, what a click does. */
function Doc({ icon: Icon, title, hint, verb, verbIcon: Verb, tone, onClick }: { icon: Icon; title: string; hint?: string; verb: string; verbIcon: Icon; tone: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="group flex flex-col gap-3 rounded-2xl sm:min-h-36 border border-line bg-card p-4 text-left shadow-card transition-[border-color,box-shadow,transform] duration-150 hover:-translate-y-0.5 hover:border-line-strong hover:shadow-lift focus-visible:ring-2 focus-visible:ring-info/60 focus-visible:outline-none active:translate-y-0"
    >
      <div className="flex items-start gap-3">
        <div className={`grid size-11 shrink-0 place-items-center rounded-xl ${tone}`}>
          <Icon className="size-5" />
        </div>
        <div className="min-w-0 flex-1 pt-0.5">
          <div className="text-[14px] leading-snug font-semibold text-ink">{title}</div>
          {hint && <div className="mt-0.5 text-xs leading-snug text-ink-3">{hint}</div>}
        </div>
      </div>
      <div className="mt-auto flex items-center gap-1.5 text-xs font-semibold text-ink-3 transition-colors group-hover:text-accent">
        <Verb className="size-3.5" />
        {verb}
        <ArrowRight className="size-3.5 -translate-x-1 opacity-0 transition-[opacity,transform] group-hover:translate-x-0 group-hover:opacity-100" />
      </div>
    </button>
  );
}

const tones = {
  show: 'bg-info/12 text-info',
  day: 'bg-accent-soft text-accent',
  post: 'bg-warn/14 text-warn',
  excel: 'bg-ok/12 text-ok',
};

export function Prints({ show }: { show: ShowView }) {
  const t = useT();
  const plural = usePlural();
  const language = useLanguage();
  const print = usePrint(show.id);
  const excel = useExcel(show.id);
  const [labels, setLabels] = useState(false);
  const [letters, setLetters] = useState(false);
  const days = show.days.map(day => ({ day, ready: day.classes.filter(c => c.starters > 0).map(c => c.id) })).filter(d => d.ready.length > 0);
  return (
    <div className="pane">
      <div className="mx-auto max-w-[1100px] space-y-7 px-3 py-5 sm:px-6 sm:py-7">
        <PageHeader icon={Printer} title={t('prints_tab')} subtitle={t('prints_hint')} />
        <Section title={t('prints_show')}>
          <Doc icon={CalendarDays} tone={tones.show} title={t('print_schedule')} hint={t('print_schedule_hint')} verb={t('print')} verbIcon={Printer} onClick={() => print({ what: 'schedule' })} />
          <Doc icon={Users} tone={tones.show} title={t('participants')} hint={t('print_participants_hint')} verb={t('print')} verbIcon={Printer} onClick={() => print({ what: 'participants' })} />
          <Doc icon={ListChecks} tone={tones.show} title={t('print_entries')} hint={t('print_entries_hint')} verb={t('print')} verbIcon={Printer} onClick={() => print({ what: 'entries' })} />
        </Section>
        {days.length > 0 && (
          <Section title={t('print_day_start_lists')} hint={t('print_day_start_lists_hint')}>
            {days.map(({ day, ready }) => (
              <Doc
                key={day.date}
                icon={CalendarDays}
                tone={tones.day}
                title={formatDate(day.date, language)}
                hint={`${ready.length} ${plural(ready.length, 'classes_selected')}`}
                verb={t('print')}
                verbIcon={Printer}
                onClick={() => print({ what: 'start_list', classes: ready })}
              />
            ))}
          </Section>
        )}
        <Section title={t('prints_post')}>
          <Doc icon={Tags} tone={tones.post} title={t('labels_addresses')} hint={t('labels_addresses_hint')} verb={t('labels_choose')} verbIcon={Tags} onClick={() => setLabels(true)} />
          <Doc icon={Mail} tone={tones.post} title={t('letters')} hint={t('letters_hint')} verb={t('letters_write')} verbIcon={Mail} onClick={() => setLetters(true)} />
        </Section>
        <Section title="Excel" hint={t('excel_hint')}>
          <Doc
            icon={FileSpreadsheet}
            tone={tones.excel}
            title={t('participants')}
            hint={t('excel_participants_hint')}
            verb={t('export_menu')}
            verbIcon={FileSpreadsheet}
            onClick={() => excel({ what: 'participants' }, t('participants'))}
          />
          <Doc
            icon={FileSpreadsheet}
            tone={tones.excel}
            title={t('entries_label')}
            hint={t('excel_entries_hint')}
            verb={t('export_menu')}
            verbIcon={FileSpreadsheet}
            onClick={() => excel({ what: 'entries' }, t('entries_label'))}
          />
        </Section>
      </div>
      <LabelsModal show={show.id} args={{ kind: 'addresses' }} title={t('labels_addresses')} open={labels} onClose={() => setLabels(false)} />
      <LettersModal show={show.id} open={letters} onClose={() => setLetters(false)} />
    </div>
  );
}
