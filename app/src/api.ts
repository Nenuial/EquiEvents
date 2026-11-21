// The office core: one call, a command name and its arguments (crates/office, `Office::call`).
// Desktop: through Tauri. Browser: the online version on equievents.ch (`/app/api/<command>`,
// the account's office there), or office-dev-server in development; the addresses are relative
// to the page, so both work. Its refusals come in English and leave here in the office's
// language (refusals.ts).

import { refusal } from './refusals';

/** In the desktop app (Tauri), not a browser. */
export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (inTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    try {
      return await invoke<T>('call', { command, args });
    } catch (e) {
      throw new Error(refusal(String((e as Error)?.message ?? e)));
    }
  }
  let response: Response;
  try {
    response = await fetch(`api/${command}`, { method: 'POST', body: JSON.stringify(args), headers: { 'Content-Type': 'application/json' } });
  } catch {
    throw new Error(refusal('EquiEvents does not answer'));
  }
  // The online version, signed out on the site (or the session expired): its sign-in, back here after.
  if (response.status === 401) signInAgain();
  // `null` is an answer (a command that answers nothing); no JSON at all is not.
  const none = Symbol();
  const answer = await response.json().catch(() => none);
  if (answer === none) throw new Error(refusal(`EquiEvents does not answer (${response.status})`));
  if (!response.ok) throw new Error(refusal(answer?.error ?? response.statusText));
  return answer as T;
}

/** Staff's calls (the online version only: `/app/admin/<path>`, the site's session). A refusal
 *  throws its code (`not_staff`, `self`, `password`…), for the panel to say in the office's
 *  language. */
export async function staff<T>(path: string, method: 'GET' | 'POST' | 'DELETE' = 'GET', body?: unknown): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`admin/${path}`, { method, body: body === undefined ? undefined : JSON.stringify(body), headers: { 'Content-Type': 'application/json' } });
  } catch {
    throw new StaffRefusal('offline');
  }
  if (response.status === 401) signInAgain();
  const answer = await response.json().catch(() => null);
  if (!response.ok) throw new StaffRefusal(answer?.code ?? 'error');
  return answer as T;
}

export class StaffRefusal extends Error {
  constructor(public code: string) {
    super(code);
  }
}

/** A file the user picks, as a path the office reads: the system's dialog (desktop); in a browser
 *  (development) the browser's, the file then kept by office-dev-server. `null`: none picked. */
export async function chooseFile(extensions: string[], label: string): Promise<string | null> {
  if (inTauri) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const path = await open({ multiple: false, filters: [{ name: label, extensions }] });
    return typeof path === 'string' ? path : null;
  }
  const file = await pickInBrowser(extensions);
  if (!file) return null;
  return (await keep(file.name, file)).path;
}

/** A file the office makes, saved where the user chooses: the system's dialog (desktop); in a
 *  browser (development) the browser's save dialog, or its downloads where it has none. `write`
 *  makes the file at the path given. The name saved as, `null` when the user did not save. */
export async function saveFile(name: string, extension: string, write: (path: string) => Promise<unknown>): Promise<string | null> {
  if (inTauri) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const path = await save({ defaultPath: name, filters: [{ name: 'EquiEvents', extensions: [extension] }] });
    if (!path) return null;
    await write(path);
    return path;
  }
  // Asked first, while the click still lets the page open a dialog.
  const picker = (window as unknown as { showSaveFilePicker?: (o: unknown) => Promise<SaveHandle> }).showSaveFilePicker;
  let handle: SaveHandle | null = null;
  if (picker) {
    try {
      handle = await picker({ suggestedName: name, types: [{ description: name, accept: { 'application/octet-stream': [`.${extension}`] } }] });
    } catch (e) {
      if ((e as Error)?.name === 'AbortError') return null;
    }
  }
  const kept = await keep(name);
  await write(kept.path);
  const bytes = await fetch(kept.url).then(r => (r.ok ? r.blob() : Promise.reject(new Error(`${name}: ${r.status}`))));
  if (handle) {
    const out = await handle.createWritable();
    await out.write(bytes);
    await out.close();
    return handle.name;
  }
  const link = document.createElement('a');
  link.href = URL.createObjectURL(bytes);
  link.download = name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(link.href), 60_000);
  return name;
}

interface SaveHandle {
  name: string;
  createWritable: () => Promise<{ write: (data: Blob) => Promise<void>; close: () => Promise<void> }>;
}

/** The browser's dialog to pick a file. */
function pickInBrowser(extensions: string[]): Promise<File | null> {
  return new Promise(resolve => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = extensions.map(e => `.${e}`).join(',');
    input.addEventListener('change', () => resolve(input.files?.[0] ?? null), { once: true });
    input.addEventListener('cancel', () => resolve(null), { once: true });
    input.click();
  });
}

/** The online version's sign-in on the site, in the office's language, back to the app after. */
function signInAgain(): never {
  const german = (() => {
    try {
      return localStorage.getItem('language') === 'de';
    } catch {
      return false;
    }
  })();
  window.location.href = `${german ? '/anmelden' : '/connexion'}?next=${encodeURIComponent(window.location.pathname)}`;
  throw new Error(refusal('EquiEvents does not answer'));
}

/** A file kept by the online version or office-dev-server (empty: one the office is to write). */
async function keep(name: string, body?: Blob): Promise<{ path: string; url: string }> {
  const r = await fetch(`files/${encodeURIComponent(name)}`, { method: 'POST', body: body ?? '' });
  if (r.status === 401) signInAgain();
  const answer = await r.json().catch(() => null);
  if (!r.ok || !answer?.path) throw new Error(refusal(answer?.error ?? `EquiEvents does not answer (${r.status})`));
  return answer;
}

/** Make a print and open it: in the system's PDF viewer (desktop), in a new tab (browser). */
export async function openPrint(args: Record<string, unknown>): Promise<void> {
  if (inTauri) {
    const answer = await call<{ path: string }>('print', args);
    const { openPath } = await import('@tauri-apps/plugin-opener');
    await openPath(answer.path);
    return;
  }
  // Opened at once (a browser lets a click open a tab, not a later answer), filled when ready.
  const tab = window.open('', '_blank');
  try {
    const answer = await call<{ pdf: string; name: string }>('print', { ...args, inline: true });
    const bytes = Uint8Array.from(atob(answer.pdf), c => c.charCodeAt(0));
    // Served under its name by office-dev-server: the browser saves "Feuilles de juge 15.pdf",
    // not a blob's random number.
    const { url } = await keep(answer.name, new Blob([bytes], { type: 'application/pdf' }));
    if (tab) tab.location.href = url;
    else window.open(url, '_blank');
  } catch (e) {
    tab?.close();
    throw e;
  }
}

/** The desktop app's own version and whether equievents.ch has a newer one (app/src-tauri/src/main.rs).
 *  `on` false: a build that does not update itself (development). */
export interface UpdateState {
  current: string;
  on: boolean;
  version?: string;
  date?: string;
}

export type UpdateProgress = { done?: number; total?: number | null; installing?: boolean };

/** Ask for a newer version (the browser: none, it is not an installed app). */
export async function checkUpdate(): Promise<UpdateState | null> {
  if (!inTauri) return null;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<UpdateState>('update_check');
}

/** Download and install the version found, then start again: the promise only ends on failure. */
export async function installUpdate(progress: (p: UpdateProgress) => void): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');
  const stop = await listen<UpdateProgress>('update-progress', e => progress(e.payload));
  try {
    await invoke('update_install');
  } finally {
    stop();
  }
}

/** Swiss Equestrian's registry on this computer (crates/office/src/registry.rs). */
export interface RegistryAbout {
  persons: number;
  horses: number;
  persons_at: number | null;
  horses_at: number | null;
  simulated: boolean;
}

/** A rider to pick: `in_show`, the show has them; `guest`, they hold a guest licence. */
export interface RiderFound {
  person: number;
  name: string;
  town: string;
  club: string;
  licences: string;
  guest: boolean;
  in_show: boolean;
}

/** A horse to pick: `paid`, Swiss Equestrian lets it start in the show's year. */
export interface HorseFound {
  horse: number;
  name: string;
  detail: string;
  in_show: boolean;
  paid: boolean;
}

// What the screens show (crates/office/src/views.rs).

export interface EventView {
  id: number;
  number: string;
  title: string[];
  place: string;
  from: string;
  to: string;
  organiser: string;
  disciplines: string[];
  fixed_numbers: boolean;
  numbers_assigned: boolean;
}

export interface ShowSummary {
  id: string;
  event: EventView | null;
  entries: number;
  classes: number;
  results: number;
  rehearsal: boolean;
  /** A training show (the practice event, Swiss Equestrian's demo event): never synced. */
  training: boolean;
  /** On equievents.ch (else on this computer only), and one's role there. */
  online: boolean;
  role: 'owner' | 'collaborator' | null;
  /** In the archives: listed there (only its responsable decides, for everyone). */
  archived: boolean;
}

export interface ClassRow {
  id: string;
  published: number;
  number: string;
  /** The class's own name (a prize's), as typed: often empty. */
  name: string;
  /** What lists call the class: its name, or else its category and scoring table. */
  title: string;
  date: string;
  time: string;
  discipline: string;
  category: string;
  scoring: string;
  starters: number;
  not_starting: number;
  results: number;
  status: 'planned' | 'running' | 'final';
  start_lists_sent: number | null;
  results_sent: number | null;
  /** The class whose penalties the riders bring (U9). */
  qualifier: string | null;
  /** Changed since last sent to Swiss Equestrian. */
  results_changed: boolean;
  start_lists_changed: boolean;
  /** Entries of the published class on none of its start lists. */
  /** Its riders have start numbers (renumbering asks first). */
  numbered: boolean;
  unplaced: number;
  /** Too many entries: the sections proposed (jumping over 90; dressage over 35, hors concours not counted). */
  divide_into: number | null;
  /** The number exceeded: 90 (jumping), 35 or 40 (dressage). */
  over: number;
  /** It has to be divided (jumping over 90, dressage over 40); else two series are the rule (dressage, 36 to 40). */
  must_divide: boolean;
  /** The most starters, if the office set one. */
  max_starters: number | null;
  /** Eventing: this class is a phase of `phase_of`. */
  phase: PhaseKind | null;
  phase_of: string | null;
  /** Eventing: the class made of phases, its phases in tie-break order. */
  phases: string[];
}

export interface PublishedRow {
  id: number;
  number: string;
  date: string;
  discipline: string;
  category: string;
  scoring: string;
  entries: number;
}

export interface ShowView {
  id: string;
  event: EventView | null;
  days: { date: string; classes: ClassRow[] }[];
  not_set_up: PublishedRow[];
  entries: number;
  riders: number;
  horses: number;
  combined: CombinedRow[];
}

export interface StartRef {
  class: string;
  number: string;
  order: number;
  start_number: number | null;
  /** The class's day (ISO date). */
  date: string;
  /** The start time with start times, else the class's time ("08:42"; empty: none). */
  time: string;
  /** The position printed on the start list. */
  position: number | null;
  /** A phase of a class made of phases: which. */
  phase: PhaseKind | null;
}

export interface EntryRow {
  id: number;
  rider_id: number;
  rider: string;
  town: string;
  club: string;
  nationality: string;
  licences: string;
  youth: string;
  horse_id: number | null;
  horse: string;
  guest_horse: boolean;
  published: number;
  class_number: string;
  category: string;
  discipline: string;
  team: string | null;
  not_starting: boolean;
  hors_concours: boolean;
  waiting_list: boolean;
  organiser_place: boolean;
  se_status: string;
  fee_paid: string | null;
  rider_changed: boolean;
  comment: string;
  /** The rider's remark made with the entry. */
  remark: string;
  stage: number | null;
  /** How many levels its class has (more than one: the entry's level matters). */
  levels: number;
  starts: StartRef[];
  /** Driving: the class's phases (Swiss Equestrian's names, in order). */
  driving_phases: string[];
  /** Driving: the entry's horses with the phases each starts in. */
  driving_horses: { name: string; phases: string[] }[];
  /** A class it starts in is final: it cannot change before that class is reopened. */
  locked: boolean;
  /** It has a result or marks already. */
  ridden: boolean;
}

/** A published class (for late entries and moving entries). */
export interface PublishedClass {
  id: number;
  number: string;
  discipline: string;
  category: string;
  date: string;
  teams: boolean;
}

export type StartLine =
  | {
      kind: 'starter';
      entry: number;
      /** The rider: a rider's other horses in the class have the same. */
      person: number;
      position: number;
      start_number: number | null;
      time: string | null;
      rider: string;
      town: string;
      club: string;
      horse: string;
      not_starting: boolean;
      hors_concours: boolean;
      /** Penalties brought from the qualifier (U9). */
      carried: string | null;
      carried_out: boolean;
      /** What more can be shown in columns (only what is filled; see the start list's columns). */
      more?: Record<string, string>;
    }
  | { kind: 'team'; team: string; name: string; time: string | null; not_starting: boolean }
  | { kind: 'text'; text: string; level?: string; pause?: [string, string] };

export interface StartListView {
  class: ClassRow;
  published_by_name: boolean;
  start_numbers: boolean;
  start_times: boolean;
  lines: StartLine[];
  qualifier: string | null;
  setup: StartListSetup;
  /** Dressage: the test's duration as written on it ("4'00"). */
  programme_time: string | null;
  /** The test's longest duration in whole minutes (dressage): the time between starts. */
  programme_minutes: number | null;
  /** Entries of the class on none of its start lists (delivered after it was set up). */
  unplaced: { entry: number; rider: string; horse: string }[];
}

/** How a start list is laid out (times in minutes after midnight). */
export type Layout = 'Plain' | { Blocks: { size: number; headings: string[]; move_up: boolean } } | { StartTimes: { first: number; minutes: number; breaks: [number, number][]; move_up: boolean } };
export interface StartListSetup {
  layout: Layout;
  line_after: [number, string] | null;
}

/** Eventing's and driving's phases. */
export type PhaseKind = 'dressage' | 'jumping' | 'cross' | 'marathon' | 'cones' | 'derby' | 'open' | 'multitest' | 'presentation';

export interface RankingLine {
  id: string;
  team: boolean;
  rank: number | null;
  manual: boolean;
  /** A Tag der Jugend final: tied in ranks 1–3 after the tie movements — lots are drawn, the rank is then set by hand. */
  draw: boolean;
  placed: boolean;
  rider: string;
  horse: string;
  club: string;
  faults: string | null;
  time: string | null;
  percentage: string | null;
  judges: Record<string, string>;
  /** Freestyle and Children: parts A and B, in percent. */
  part_a: string | null;
  part_b: string | null;
  /** Team cones (OKV Fahrcup): the team's rank points. */
  rank_points: number | null;
  /** Out of the class: why (Swiss Equestrian's name, `useReason`). */
  out: string | null;
}

export interface RankingView {
  class: ClassRow;
  placed_count: number;
  lines: RankingLine[];
  /** Dressage: the judges' positions in the class's order. */
  judge_order: string[];
}

export interface EventInFile {
  id: string;
  number: string;
  title: string;
  from: string;
  to: string;
  entries: number;
  results: number;
}

export interface HorseRow {
  id: number;
  name: string;
  signalement: string;
  owner: string;
}

// Show-jumping results (crates/office/src/views.rs, `results`).
export interface ResultLine {
  entry: number;
  position: number;
  start_number: number | null;
  rider: string;
  horse: string;
  not_starting: boolean;
  hors_concours: boolean;
  /** By scoring-table field: `strafpunkte_initial_1`, `zeit_initial_1`, … */
  values: Record<string, string>;
  out: number | null;
  /** Cones: a round over its time limit, eliminated by the rules (not by the office). */
  over_maximum: boolean;
  has_result: boolean;
  rank: number | null;
  manual_rank: boolean;
  placed: boolean;
  faults: string | null;
  time: string | null;
  carried: string | null;
}

export interface ResultsView {
  class: ClassRow;
  fields: string[];
  two_phase: boolean;
  /** Scored on points, the most first: `strafpunkte_*` fields hold points, not faults. */
  points: boolean;
  reasons: { code: number; name: string }[];
  qualifier: string | null;
  placed_count: number;
  ranking_error: string | null;
  /** Cones in Wertung B: the faults are entered in seconds (5 s a ball). */
  faults_in_seconds: boolean;
  lines: ResultLine[];
}

// Rankings over several classes (U8).
export type PenaltiesTie = 'none' | 'faults' | 'result';
export interface CombinedRow {
  id: string;
  name: string;
  method: 'penalties' | 'ranks' | 'percentages';
  /** What breaks ties: penalties added by nothing (ex aequo), or the penalties or the result in a
   *  class (`tie_break`); ranks by the rank in `tie_break`. */
  tie: PenaltiesTie | 'rank' | 'percentage';
  tie_break: string | null;
  classes: { id: string; number: string; date: string }[];
  /** Swiss Equestrian's number for its results (Rosson: the first class's with ".1"). */
  number: string;
}

export interface CombinedView {
  ranking: CombinedRow;
  lines: {
    rank: number | null;
    rider: string;
    club: string;
    horse: string;
    /** `missing`: not in this class. */
    cells: { missing: boolean; rank: number | null; faults: string | null; time: string | null; percentage: string | null; out: string | null }[];
    total: string | null;
    tie_break: string | null;
    placed: boolean;
  }[];
  /** Sent to Swiss Equestrian (when); a class's results changed since. */
  sent: number | null;
  changed: boolean;
}

// Swiss Equestrian (crates/office/src/connection.rs, views.rs `sending`).
export type Server = 'simulation' | 'stage' | 'production';
export interface Connection {
  /** A training show: it sends to the simulation only. */
  training: boolean;
  /** A rehearsal: it sends to the simulation only. */
  rehearsal: boolean;
  /** A show of equievents.ch: it sends from there. */
  online: boolean;
  /** Where its sends go (equievents.ch's choice for a show there; null: it does not answer). */
  server: Server | null;
  has_key: boolean;
  key_end: string | null;
}
export interface SentView {
  millis: number;
  author: string;
}
export interface SendRow {
  class: ClassRow;
  results_sent: SentView | null;
  results_changed: boolean;
  lists_here: boolean;
  start_lists_sent: SentView | null;
  start_lists_changed: boolean;
}
export interface SendingView {
  event_sent: SentView | null;
  rows: SendRow[];
}
export interface Received {
  millis: number;
  method: string;
  path: string;
  query: [string, string][];
  body: unknown;
  status: number;
  answer: unknown;
}
