// A class's page: its head (what it is, where it stands, what to do next), then one part at a
// time in tabs: the start list (S), entering results (E), the ranking (R), its settings (D).
// Tools that are not the page's work open in dialogs: dividing into sections, the order, the
// layout, prizes, labels, deleting, reopening.

import { useEffect, useState, type ReactNode } from 'react';
import {
  ArrowLeft,
  ArrowUpDown,
  CircleCheck,
  ClipboardPen,
  CloudUpload,
  Ellipsis,
  FileSpreadsheet,
  Flag,
  Gavel,
  Hash,
  Keyboard,
  LayoutTemplate,
  ListOrdered,
  Lock,
  LockOpen,
  Medal,
  Network,
  Printer,
  Repeat,
  Rows3,
  Settings2,
  Shuffle,
  Split as SplitIcon,
  Tags,
  Trash2,
  Trophy,
} from 'lucide-react';
import { call, type ClassRow, type RankingView, type StartListView } from '../api';
import { href, type Place } from '../App';
import { formatDate, formatMillis, usePlural, useT, type Language } from '../i18n';
import { useChanges } from '../parts/changes';
import { ErrorNote, useLoad } from '../parts/common';
import { DayField, DetailsEditor } from '../parts/details';
import { LayoutEditor, layoutHint } from '../parts/layout';
import { OrderEditor } from '../parts/order';
import { ProgrammePicker, ScoringTablePicker, type ScoringTableChoice } from '../parts/pickers';
import { OverallPane, phaseName } from '../parts/overall';
import { LabelsModal, StartCardsModal, useExcel, usePrint } from '../parts/print';
import { PrizeEditor } from '../parts/prizes';
import { SectionsWorkshop, sizeWarning } from '../parts/sections';
import { StartList, Unplaced } from '../parts/startlist';
import { RankingTable } from '../parts/tables';
import { Button, Callout, Card, ConfirmModal, EmptyState, Field, IconButton, Kbd, Loading, Menu, MenuItem, MenuLabel, MenuSeparator, Modal, Pill, Tabs, Tooltip, overlayOpen } from '../ui';
import { ClassNumber, ClassStatus, SentMarks } from './Classes';
import DressageEntry from './DressageEntry';
import JumpingEntry from './JumpingEntry';
import MultitestEntry from './MultitestEntry';

type Go = (place: Place) => void;
type Tab = 'list' | 'entry' | 'ranking' | 'settings';

/** The other series of a divided class (eventing's and driving's phases are not series). */
export const sectionsOf = (classes: ClassRow[], row: ClassRow | undefined) =>
  row && !row.phase && row.phases.length === 0 ? classes.filter(c => c.published === row.published && c.id !== row.id && !c.phase && c.phases.length === 0) : [];

function storedTab(): Tab {
  const v = localStorage.getItem('class-tab');
  return v === 'entry' || v === 'ranking' || v === 'settings' ? v : 'list';
}

export function ClassPage({ show, classId, entry, classes, go, language }: { show: string; classId: string; entry?: number; classes: ClassRow[]; go: Go; language: Language }) {
  const t = useT();
  const plural = usePlural();
  const print = usePrint(show);
  const excel = useExcel(show);
  const { version, change } = useChanges();
  const list = useLoad(() => call<StartListView>('start_list', { show, class: classId }), [show, classId, version]);
  const ranking = useLoad(() => call<RankingView>('ranking', { show, class: classId }), [show, classId, version]);
  // An entry to show (come from a rider's dialog): on the start list.
  const [chosen, setChosen] = useState<Tab>(() => (entry ? 'list' : storedTab()));
  const [sections, setSections] = useState<{ parts?: number; rule?: string } | null>(null);
  const [dialog, setDialog] = useState<
    null | 'order' | 'layout' | 'prizes' | 'qualifier' | 'reopen' | 'lists' | 'results' | 'keys' | 'labels_prizes' | 'labels_judges' | 'renumber' | 'cards' | 'team_cards'
  >(null);

  const row = list.data?.class;
  const dressage = row?.discipline === 'CD';
  // Cross-country is entered as a jumping class (penalties and time); so are driving's marathon,
  // cones, derby and open (their own fields).
  const jumping = ['CS', 'CROSS', 'GE', 'HI', 'DE', 'OF'].includes(row?.discipline ?? '');
  // Driving's multitest: the judges' sheets on Rosson's programme.
  const multitest = row?.discipline === 'MT';
  const entryCapable = dressage || jumping || multitest;
  // Eventing: a class made of phases shows its phases and the overall ranking.
  const overall = row?.discipline === 'CC' && row.phases.length > 0;
  // Classes that can be this one's qualifier (U9): the other show-jumping classes.
  const qualifiers = classes.filter(c => c.discipline === 'CS' && c.id !== classId);
  const editable = !!row && row.status !== 'final';
  const nothingYet = !!row && row.results === 0;
  // Team classes: the order is set by teams.
  const teams = list.data?.lines.some(l => l.kind === 'team') ?? false;
  const series = sectionsOf(classes, row);
  const tab: Tab = chosen === 'entry' && !entryCapable ? 'list' : chosen;
  const choose = (v: Tab) => {
    localStorage.setItem('class-tab', v);
    setChosen(v);
  };

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      if (target.closest('input, textarea, select, [contenteditable]') || e.metaKey || e.ctrlKey || e.altKey || overlayOpen()) return;
      if (e.key === 's') choose('list');
      else if (e.key === 'r') choose('ranking');
      else if (e.key === 'd') choose('settings');
      else if (e.key === 'e' && entryCapable) choose('entry');
      else if (e.key === '?') setDialog('keys');
      // Unless something took it first (a row let go, a drag cancelled).
      else if (e.key === 'Escape' && !e.defaultPrevented) go({ show, view: 'classes' });
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  });

  if (!row) return <div className="pane">{list.error ? <ErrorNote error={list.error} /> : <Loading rows={10} />}</div>;

  const sendResults = () => change('send', { what: 'results', class: classId, language });
  const startCards = ['HI', 'DE', 'GE'].includes(row.discipline)
    ? { what: 'control_sheets', label: t('print_control_sheets') }
    : !overall && !['OF', 'MT'].includes(row.discipline)
      ? { what: 'start_cards', label: t('print_start_cards') }
      : null;

  const printMenu = (
    <Menu
      trigger={
        <Button icon={Printer} iconRight={undefined}>
          <span className="hidden sm:inline">{t('print')}</span>
        </Button>
      }
      width="w-72"
    >
      {!overall && (
        <>
          <MenuLabel>{t('start_list')}</MenuLabel>
          <MenuItem icon={ListOrdered} onSelect={() => print({ what: 'start_list', class: classId })}>
            {t('print_full')}
          </MenuItem>
          {jumping && (
            <MenuItem icon={Network} onSelect={() => print({ what: 'start_list', class: classId, pedigree: true })}>
              {t('print_pedigree')}
            </MenuItem>
          )}
          <MenuItem icon={Rows3} onSelect={() => print({ what: 'start_list', class: classId, variant: 'lite' })}>
            {t('print_lite')}
          </MenuItem>
          <MenuItem icon={Gavel} onSelect={() => print({ what: 'start_list', class: classId, variant: 'judges' })}>
            {t('print_for_judges')}
          </MenuItem>
          <MenuSeparator />
        </>
      )}
      {(row.results > 0 || overall) && (
        <MenuItem icon={Trophy} onSelect={() => print({ what: 'ranking', class: classId })}>
          {overall ? t('overall') : t('ranking')}
        </MenuItem>
      )}
      {/* Dressage: each judge's rank next to their points (Rosson's "Avec détail par juge"). */}
      {dressage && row.results > 0 && (
        <MenuItem icon={Trophy} onSelect={() => print({ what: 'ranking', class: classId, by_judge: true })}>
          {t('print_ranking_by_judge')}
        </MenuItem>
      )}
      {/* All judges in one document; optionally a memo sheet first for each judge. */}
      {dressage && (
        <>
          <MenuItem icon={ClipboardPen} onSelect={() => print({ what: 'judge_sheets', class: classId })}>
            {t('print_judge_sheets')} · {t('judge_sheets_all')}
          </MenuItem>
          <MenuItem icon={ClipboardPen} onSelect={() => print({ what: 'judge_sheets', class: classId, memo: true })}>
            {t('print_judge_sheets')} · {t('judge_sheets_memo')}
          </MenuItem>
          <MenuItem icon={Tags} onSelect={() => setDialog('labels_judges')}>
            {t('labels_judges')}…
          </MenuItem>
        </>
      )}
      {/* Driving: control sheets instead of start cards (cones, derby; marathon obstacles). */}
      {startCards && (
        // Start cards: how and whose, in a dialog; driving's control sheets straight away.
        <MenuItem icon={Flag} onSelect={() => (startCards.what === 'start_cards' ? setDialog('cards') : print({ what: startCards.what, class: classId }))}>
          {startCards.label}
          {startCards.what === 'start_cards' && '…'}
        </MenuItem>
      )}
      {teams && jumping && (
        <MenuItem icon={Flag} onSelect={() => setDialog('team_cards')}>
          {t('print_team_cards')}…
        </MenuItem>
      )}
      {jumping && /styl|stil/i.test(row.scoring) && (
        <MenuItem icon={ClipboardPen} onSelect={() => print({ what: 'style_sheets', class: classId })}>
          {t('print_style_sheets')}
        </MenuItem>
      )}
      <MenuItem icon={ListOrdered} onSelect={() => print({ what: 'entries', published: row.published })}>
        {t('print_entries')}
      </MenuItem>
      {row.results > 0 && (
        <MenuItem icon={Tags} onSelect={() => setDialog('labels_prizes')}>
          {t('labels_prizes')}…
        </MenuItem>
      )}
    </Menu>
  );

  const moreMenu = (
    <Menu trigger={<IconButton icon={Ellipsis} label={t('actions_more')} variant="secondary" />}>
      {row.results > 0 && (
        <MenuItem icon={CloudUpload} onSelect={sendResults} hint={row.results_changed ? t('changed_since') : undefined}>
          {row.results_sent ? t('resend') : t('send_results')}
        </MenuItem>
      )}
      <MenuItem icon={Medal} onSelect={() => setDialog('prizes')}>
        {t('prizes')}
      </MenuItem>
      <MenuSeparator />
      <MenuItem icon={FileSpreadsheet} onSelect={() => excel({ what: 'start_list', class: classId }, `${t('start_list')} ${row.number}`)}>
        Excel · {t('start_list')}
      </MenuItem>
      {row.results > 0 && (
        <MenuItem icon={FileSpreadsheet} onSelect={() => excel({ what: 'ranking', class: classId }, `${t('ranking')} ${row.number}`)}>
          Excel · {t('ranking')}
        </MenuItem>
      )}
      <MenuSeparator />
      <MenuItem icon={Keyboard} kbd="?" onSelect={() => setDialog('keys')}>
        {t('keyboard')}
      </MenuItem>
      {editable && (
        <>
          <MenuSeparator />
          {row.results > 0 && (
            <MenuItem icon={Trash2} danger onSelect={() => setDialog('results')}>
              {t('delete_results')}…
            </MenuItem>
          )}
          <MenuItem icon={Trash2} danger onSelect={() => setDialog('lists')}>
            {t('remove_start_lists')}…
          </MenuItem>
        </>
      )}
    </Menu>
  );

  // What to do next, in the head: close the class once results are in; send them; reopen.
  const next =
    row.status === 'final' ? (
      <>
        <Button icon={LockOpen} onClick={() => setDialog('reopen')}>
          <span className="hidden sm:inline">{t('reopen')}</span>
        </Button>
        {row.results > 0 && (!row.results_sent || row.results_changed) && (
          <Button variant="primary" icon={CloudUpload} onClick={sendResults}>
            {row.results_sent ? t('resend') : t('send_results')}
          </Button>
        )}
      </>
    ) : row.results > 0 ? (
      <Button variant="primary" icon={Lock} onClick={() => change('finalise', { class: classId })}>
        {t('finalise')}
      </Button>
    ) : null;

  const toolbar = (children: ReactNode, title: ReactNode) => (
    <div className="mb-3 flex flex-wrap items-center gap-2">
      <div className="mr-auto min-w-0 text-[13px] text-ink-3">{title}</div>
      {children}
    </div>
  );

  // Nobody entered (an overall ranking's own list, an empty class): nothing to draw or arrange.
  const noStarters = !!list.data && list.data.lines.every(l => l.kind !== 'starter');
  const listTab = list.data && (
    <>
      {toolbar(
        <>
          {editable && nothingYet && !noStarters && (
            <>
              <Button icon={Shuffle} onClick={() => change('draw', { class: classId })}>
                {t('draw')}
              </Button>
              {/* Numbers given already (printed, handed out, maybe sent): asked first. */}
              <Button icon={Hash} onClick={() => (row?.numbered ? setDialog('renumber') : change('number', { class: classId, first: 1 }))}>
                {t('numbers')}
              </Button>
            </>
          )}
          {/* Dividing into séries: a button of its own (the user, 2026-11-16). */}
          {editable && !noStarters && !teams && (
            <Tooltip label={t('sections_hint')}>
              <Button icon={SplitIcon} onClick={() => setSections({})}>
                {series.length > 0 ? t('sections_change') : t('sections_open')}
              </Button>
            </Tooltip>
          )}
          {editable && !noStarters && (
            <Menu trigger={<Button icon={ArrowUpDown}>{t('organise')}</Button>} width="w-80">
              {!teams && (
                <MenuItem icon={ArrowUpDown} onSelect={() => setDialog('order')} hint={t('order_hint')}>
                  {t('order')}
                </MenuItem>
              )}
              <MenuItem icon={LayoutTemplate} onSelect={() => setDialog('layout')} hint={t(layoutHint(row))}>
                {t('layout')}
              </MenuItem>
              {jumping && (
                <>
                  <MenuSeparator />
                  <MenuItem
                    icon={Medal}
                    onSelect={() => setDialog('qualifier')}
                    hint={row.qualifier ? `${t('qualifier')} : ${classes.find(c => c.id === row.qualifier)?.number ?? ''}` : t('carried_hint')}
                  >
                    {t('qualifier')}…
                  </MenuItem>
                  {row.qualifier && (
                    <MenuItem icon={Repeat} onSelect={() => change('reverse_order', { class: classId })}>
                      {t('reverse_order')}
                    </MenuItem>
                  )}
                </>
              )}
            </Menu>
          )}
        </>,
        <span>
          <span className="font-semibold text-ink">{row.starters}</span> {plural(row.starters, 'entered_count')}
          {row.not_starting > 0 && ` · ${row.not_starting} ${t('not_starting')}`}
          {series.length > 0 && (
            <span className="ml-2 inline-flex items-center gap-1 align-middle">
              · {t('sections')}
              {[row, ...series]
                .sort((a, b) => a.number.localeCompare(b.number))
                .map(s => (
                  <a
                    key={s.id}
                    href={href({ show, view: 'classes', class: s.id })}
                    className={`rounded-md px-1.5 py-0.5 text-xs font-bold ${s.id === row.id ? 'bg-accent-soft text-accent' : 'bg-sunken text-ink-2 hover:text-ink'}`}
                  >
                    {s.number}
                  </a>
                ))}
            </span>
          )}
        </span>,
      )}
      <div className="space-y-3">
        {row.divide_into && (
          <Callout
            tone={row.must_divide ? 'warn' : 'info'}
            title={`${row.starters} ${plural(row.starters, 'entries')}`}
            actions={
              editable &&
              nothingYet && (
                <Button variant="primary" size="sm" icon={SplitIcon} onClick={() => setSections({ parts: row.divide_into ?? 2, rule: 'balanced' })}>
                  {t('divide_into').replace('{n}', String(row.divide_into))}
                </Button>
              )
            }
          >
            {sizeWarning(row, t)}
          </Callout>
        )}
        <Unplaced list={list.data} sections={series} />
        <Card className="overflow-hidden">
          {noStarters ? (
            <EmptyState icon={ListOrdered} title={t('no_starters')}>
              {t('no_starters_hint')}
            </EmptyState>
          ) : (
            <StartList show={show} list={list.data} highlight={entry} sections={series} />
          )}
        </Card>
      </div>
    </>
  );

  const rankingTab = ranking.data && (
    <>
      {toolbar(
        <>
          <Button icon={Medal} onClick={() => setDialog('prizes')}>
            {t('prizes')}
          </Button>
        </>,
        <span>
          <span className="font-semibold text-ink">{ranking.data.placed_count}</span> {t('placed')}
          {row.results > 0 && ` · ${row.results} ${t('results')}`}
        </span>,
      )}
      <Card className="overflow-hidden">
        <RankingTable ranking={ranking.data} classId={classId} editable={editable} />
      </Card>
    </>
  );

  return (
    <div className="flex h-full flex-col phone:h-auto">
      <div className="shrink-0 border-b border-line bg-card/80 backdrop-blur-sm">
        <div className="mx-auto max-w-[1400px] px-3 pt-3 sm:px-6 sm:pt-4">
          <div className="mb-2 flex items-center gap-2 text-[13px]">
            <a href={href({ show, view: 'classes' })} className="inline-flex items-center gap-1.5 font-semibold text-ink-3 hover:text-ink">
              <ArrowLeft className="size-4" />
              {t('classes')}
            </a>
            {row.phase && row.phase_of && (
              <>
                <span className="text-ink-4">/</span>
                <a href={href({ show, view: 'classes', class: row.phase_of })} className="font-semibold text-ink-3 hover:text-ink">
                  {t('overall')}
                </a>
              </>
            )}
          </div>
          <div className="flex flex-wrap items-center gap-x-4 gap-y-3">
            <div className="flex min-w-0 flex-1 basis-[22rem] items-center gap-3">
              <ClassNumber number={row.number} size="lg" />
              <div className="min-w-0">
                <div className="flex min-w-0 items-center gap-2">
                  {row.phase && <Pill tone="info">{phaseName(t, row.phase)}</Pill>}
                  <h1 className="line-clamp-2 text-xl leading-tight font-bold tracking-tight sm:truncate sm:text-[22px]">{row.title}</h1>
                </div>
                <div className="mt-0.5 truncate text-[13px] text-ink-3">
                  <span className="capitalize">{formatDate(row.date, language)}</span>
                  {[row.time, `${row.discipline} · ${row.category}`, row.scoring && !row.title.includes(row.scoring) ? row.scoring : '']
                    .filter(Boolean)
                    .map(x => ` · ${x}`)
                    .join('')}
                </div>
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <ClassStatus row={row} />
              <SentMarks row={row} language={language} />
              {next}
              {printMenu}
              {moreMenu}
            </div>
          </div>
          {!overall && (
            <div className="mt-2 -mb-px flex items-center">
              <Tabs
                value={tab}
                onChange={choose}
                className="flex-1"
                compact
                items={[
                  { value: 'list', label: t('start_list'), icon: ListOrdered, kbd: 'S', count: row.starters },
                  ...(entryCapable ? [{ value: 'entry' as Tab, label: t('entry_mode'), icon: ClipboardPen, kbd: 'E' }] : []),
                  { value: 'ranking', label: t('ranking'), icon: Trophy, kbd: 'R', count: row.results || null },
                  { value: 'settings', label: t('class_settings'), icon: Settings2, kbd: 'D' },
                ]}
              />
            </div>
          )}
        </div>
      </div>
      <ErrorNote error={list.error ?? (tab === 'ranking' ? ranking.error : null)} />
      <div className="min-h-0 flex-1 overflow-auto phone:overflow-visible">
        {overall ? (
          <div className="mx-auto max-w-[1400px] px-3 py-4 sm:px-6">
            <Card className="overflow-hidden">
              <OverallPane show={show} classId={classId} open={phase => go({ show, view: 'classes', class: phase })} />
            </Card>
          </div>
        ) : tab === 'entry' && dressage ? (
          <DressageEntry show={show} classId={classId} language={language} />
        ) : tab === 'entry' && jumping ? (
          <JumpingEntry show={show} classId={classId} />
        ) : tab === 'entry' && multitest ? (
          <MultitestEntry show={show} classId={classId} />
        ) : (
          <div className="mx-auto max-w-[1400px] px-3 py-4 sm:px-6 sm:py-5">
            {tab === 'list' && listTab}
            {tab === 'ranking' && rankingTab}
            {tab === 'settings' && <ClassSettings show={show} row={row} editable={editable} />}
          </div>
        )}
      </div>

      {/* The dialogs. */}
      {sections && editable && !teams && (
        <SectionsWorkshop
          show={show}
          published={row.published}
          startParts={sections.parts}
          startRule={sections.rule}
          title={series.length > 0 ? t('sections_change') : t('sections_open')}
          done={next => {
            setSections(null);
            if (next) go({ show, view: 'classes', class: next });
          }}
        />
      )}
      {dialog === 'order' && list.data && <OrderEditor list={list.data} classes={classes} done={() => setDialog(null)} />}
      {dialog === 'layout' && list.data && <LayoutEditor list={list.data} row={row} done={() => setDialog(null)} />}
      {dialog === 'prizes' && (
        <PrizeEditor show={show} published={row.published} title={`${t('prizes').replace(/…$/, '')} · ${row.number}`} placed={ranking.data?.placed_count ?? 0} done={() => setDialog(null)} />
      )}
      <Modal
        open={dialog === 'qualifier'}
        onClose={() => setDialog(null)}
        title={t('qualifier')}
        icon={Medal}
        description={t('carried_hint')}
        size="sm"
        footer={<Button onClick={() => setDialog(null)}>{t('close')}</Button>}
      >
        <Field label={t('qualifier')}>
          <select value={row.qualifier ?? ''} onChange={e => change('edit_class', { class: classId, qualifier: e.target.value || null })} className="field">
            <option value="">{t('qualifier_none')}</option>
            {qualifiers.map(c => (
              <option key={c.id} value={c.id}>
                {c.number} · {c.category}
              </option>
            ))}
          </select>
        </Field>
      </Modal>
      <ConfirmModal
        open={dialog === 'reopen'}
        onClose={() => setDialog(null)}
        title={`${t('reopen')} · ${row.number}`}
        message={t('reopen_hint')}
        reason={t('reopen_reason')}
        confirm={t('reopen')}
        icon={LockOpen}
        onConfirm={async reason => {
          if (await change('reopen', { class: classId, reason })) setDialog(null);
        }}
      />
      <ConfirmModal
        open={dialog === 'lists'}
        onClose={() => setDialog(null)}
        title={t('remove_start_lists')}
        message={
          <>
            <p>{t('remove_start_lists_ask')}</p>
            <p className="mt-2 text-xs text-ink-3">{t('remove_start_lists_hint')}</p>
          </>
        }
        confirm={t('remove_start_lists')}
        danger
        icon={Trash2}
        onConfirm={async () => {
          setDialog(null);
          if (await change('remove_start_lists', { published: row.published })) go({ show, view: 'classes' });
        }}
      />
      <ConfirmModal
        open={dialog === 'renumber'}
        onClose={() => setDialog(null)}
        title={t('renumber_title')}
        message={
          <>
            <p>{t('renumber_class').replace('{class}', row.number)}</p>
            {row.start_lists_sent && <p className="mt-2 font-semibold text-danger">{t('renumber_sent').replace('{date}', formatMillis(row.start_lists_sent, language))}</p>}
          </>
        }
        confirm={t('renumber_confirm')}
        danger
        icon={Hash}
        onConfirm={async () => {
          setDialog(null);
          await change('number', { class: classId, first: 1 });
        }}
      />
      <ConfirmModal
        open={dialog === 'results'}
        onClose={() => setDialog(null)}
        title={t('delete_results')}
        message={t('delete_results_ask')}
        confirm={t('delete_results')}
        danger
        icon={Trash2}
        onConfirm={async () => {
          setDialog(null);
          await change('delete_results', { class: classId });
        }}
      />
      <StartCardsModal
        show={show}
        classId={classId}
        open={dialog === 'cards'}
        onClose={() => setDialog(null)}
        starters={(list.data?.lines ?? []).flatMap(l =>
          l.kind === 'starter' && !l.not_starting ? [{ entry: l.entry, label: [l.start_number, l.rider].filter(Boolean).join(' · '), detail: l.horse }] : [],
        )}
      />
      <StartCardsModal show={show} classId={classId} teams open={dialog === 'team_cards'} onClose={() => setDialog(null)} starters={teamChoices(list.data?.lines ?? [])} />
      <LabelsModal show={show} args={{ kind: 'prizes', class: classId }} title={t('labels_prizes')} open={dialog === 'labels_prizes'} onClose={() => setDialog(null)} />
      <LabelsModal show={show} args={{ kind: 'judges', class: classId }} title={t('labels_judges')} open={dialog === 'labels_judges'} onClose={() => setDialog(null)} />
      <Modal open={dialog === 'keys'} onClose={() => setDialog(null)} title={t('keyboard')} icon={Keyboard} size="sm">
        <dl className="grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-2.5 text-[13px]">
          {[
            ['S', t('start_list')],
            ...(entryCapable ? [['E', t('entry_mode')]] : []),
            ['R', t('ranking')],
            ['D', t('class_settings')],
            ['Esc', t('keys_escape')],
            ['⌥ ↑↓', t('keys_move_starter')],
            ['0–9', t('type_place')],
            ['N', t('not_starting')],
            ['H', t('hors_concours')],
            ['⌘Z', t('undo')],
            ['⌘⇧Z', t('redo')],
          ].map(([k, what]) => (
            <div key={k} className="contents">
              <dt>
                <Kbd>{k}</Kbd>
              </dt>
              <dd className="text-ink-2">{what}</dd>
            </div>
          ))}
        </dl>
      </Modal>
    </div>
  );
}

/** A class's settings: its name, day and time, then its details (placed, sponsor, lines…). The
 *  number is the app's (the published class's, a letter per section). */
function ClassSettings({ show, row, editable }: { show: string; row: ClassRow; editable: boolean }) {
  const t = useT();
  const { change, version } = useChanges();
  const details = useLoad(
    () =>
      call<{
        days: string[];
        planned_days: string[];
        scoring_table: number | null;
        scoring_tables: ScoringTableChoice[];
        has_results: boolean;
        programme: number | null;
        uses_programme: boolean;
        programme_locked: boolean;
      }>('class_details', { show, class: row.id }),
    [show, row.id, version],
  ).data;
  const [name, setName] = useState(row.name);
  const [date, setDate] = useState(row.date);
  const [time, setTime] = useState(row.time);
  const [table, setTable] = useState<number | null>(null);
  const [programme, setProgramme] = useState<number | null>(null);
  useEffect(() => {
    setName(row.name);
    setDate(row.date);
    setTime(row.time);
  }, [row.name, row.date, row.time]);
  useEffect(() => setTable(details?.scoring_table ?? null), [details?.scoring_table]);
  useEffect(() => setProgramme(details?.programme ?? null), [details?.programme]);
  const tableChanged = table !== null && table !== (details?.scoring_table ?? null);
  const programmeChanged = programme !== null && programme !== (details?.programme ?? null);
  const changed = name.trim() !== row.name || date !== row.date || time !== row.time || tableChanged || programmeChanged;
  // Without a name, lists call the class by its category and scoring table.
  const unnamed = row.name === '' ? row.title : [row.category, row.scoring].filter(Boolean).join(' · ');
  const tables = details?.scoring_tables ?? [];
  const save = () =>
    change('edit_class', {
      class: row.id,
      name: name.trim(),
      time,
      ...(date !== row.date ? { date } : {}),
      ...(tableChanged ? { scoring_table: table } : {}),
      ...(programmeChanged ? { programme } : {}),
    });
  return (
    <div className="mx-auto max-w-4xl space-y-4">
      {!editable && (
        <Callout tone="info" title={t('status_final')}>
          {t('final_locked')}
        </Callout>
      )}
      <Card padded>
        <form
          className="grid gap-5"
          onSubmit={e => {
            e.preventDefault();
            if (editable && changed) save();
          }}
        >
          {/* The class's own name (a prize's), apart from what it is. */}
          <Field label={t('class_name')} hint={name.trim() === '' ? t('class_name_empty').replace('{title}', unnamed) : t('class_name_hint')}>
            <input value={name} disabled={!editable} onChange={e => setName(e.target.value)} placeholder={t('class_name_placeholder')} className="field" />
          </Field>
          {/* What the class is: its category (Swiss Equestrian's) and scoring table. */}
          <div className="grid gap-4 sm:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
            <Field label={t('class_category')}>
              <div className="field flex items-center bg-sunken text-ink-2" title={`${row.discipline} · ${row.category}`}>
                <span className="truncate">
                  {row.discipline} · {row.category}
                </span>
              </div>
            </Field>
            {tables.length > 0 ? (
              <Field label={t('pub_scoring_table')} hint={details?.has_results ? t('pub_scoring_locked') : undefined}>
                <ScoringTablePicker value={table} tables={tables} onChange={setTable} disabled={!editable || details?.has_results} />
              </Field>
            ) : details?.uses_programme ? (
              <Field label={t('programme')} hint={details.programme_locked ? t('programme_locked') : undefined}>
                <ProgrammePicker show={show} classId={row.id} value={programme} onChange={setProgramme} disabled={!editable || details.programme_locked} />
              </Field>
            ) : (
              row.scoring && (
                <Field label={t('pub_scoring_table')}>
                  <div className="field flex items-center bg-sunken text-ink-2" title={row.scoring}>
                    <span className="truncate">{row.scoring}</span>
                  </div>
                </Field>
              )
            )}
          </div>
          {/* When. */}
          <div className="grid items-start gap-4 sm:grid-cols-[13rem_8rem_1fr]">
            <Field label={t('class_day')}>
              <DayField value={date} days={details?.days ?? [row.date]} planned={details?.planned_days ?? []} disabled={!editable} onChange={setDate} />
            </Field>
            <Field label={t('time')}>
              <input value={time} disabled={!editable} onChange={e => setTime(e.target.value)} placeholder="08h00" className="field" />
            </Field>
            <div className="flex sm:justify-end sm:pt-[22px]">
              <Button type="submit" variant="primary" icon={CircleCheck} disabled={!editable || !changed}>
                {t('save')}
              </Button>
            </div>
          </div>
        </form>
      </Card>
      {editable && (
        <Card className="overflow-hidden">
          <DetailsEditor show={show} classId={row.id} />
        </Card>
      )}
    </div>
  );
}

/** A team class's teams, for their start cards: "1 · Team", its riders. */
function teamChoices(lines: StartListView['lines']) {
  const out: { entry: string; label: string; detail: string }[] = [];
  for (const l of lines) {
    if (l.kind === 'team') out.push({ entry: l.team, label: `${out.length + 1} · ${l.name}`, detail: '' });
    else if (l.kind === 'starter' && out.length > 0) {
      const team = out[out.length - 1];
      team.detail = team.detail ? `${team.detail} · ${l.rider}` : l.rider;
    }
  }
  return out;
}
