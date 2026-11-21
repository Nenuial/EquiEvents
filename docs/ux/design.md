# The office app's design

How the office app looks and is organised, so every screen feels like the same app. It answers
U1 (modals, not panels; one thing per screen), U10 (phones) and U14 (style, light and dark) of
docs/ux/brief.md. The building blocks are in `app/src/ui` (one import: `from '../ui'`); the
colours and shadows in `app/src/index.css`.

## Principles

1. **One thing per screen, full width.** A screen's parts are tabs, not a long page with
   everything on it: a class has *Liste de départ*, *Saisie*, *Classement*, *Réglages*; the
   classes screen *Programme*, *À préparer*, *Classements sur plusieurs épreuves*; the officials
   *Officiels du concours*, then one tab per show day. A tab is remembered.
2. **Dialogs, never panels.** Details, settings and every "do this" open in a dialog over the
   screen: an entry, a start list peeked from the participants, dividing a class, its order, its
   layout, its prizes, an official, the access key, a confirmation. Nothing slides in from a side
   and nothing opens under a row. On a phone a dialog is a sheet from the bottom.
3. **Main actions only, and large.** A screen shows the actions that matter there (one of them
   coral: the main one); the others are in one menu (⋯, *Organiser*, *Imprimer*). Buttons are
   at least 32 px high with the mouse, 40–44 px on a touch screen.
4. **A dialog has one button that does it.** The main action on the right of the footer, *Annuler*
   beside it, a destructive one (*Retirer*, *Oublier la clé*) alone on the left. When there are
   several ways to do one thing (sort, follow a class, after a ranking, start from…), they are
   choices (radio cards) and the one picked is applied — never a button per way.
5. **States are pills, with a colour and an icon:** *prévue*, *en cours*, *clôturée*,
   *envoyé*, *modifié depuis*, *pas envoyé*… The same words and colours everywhere.
6. **Keyboard first at the desk.** Frequent actions have a key, shown in tooltips and in the
   menus; each screen's keys are listed in a *Raccourcis clavier* dialog (`?`). Typing in a
   field never triggers a key of the screen.
7. **Every screen works on a phone** (U10): the frame becomes a header and a bottom bar, tables
   become lists or scroll sideways with their first column kept, headers wrap rather than
   squeeze.
8. **Ordering is drag and drop** (touch and keyboard). In a start list the place can also be
   typed: a click on the number (or a digit on the selected row), then Enter.

## The frame

- **Computer** (≥ 1024 px): a navy sidebar — the show (number, name, place, dates), its sections
  (*Épreuves* ⌘1, *Participants* ⌘2, *Officiels* ⌘3, *Impressions* ⌘4, *Swiss Equestrian* ⌘5,
  *Historique* ⌘6), then where the show lives (on this computer, or online and its sync state),
  the settings, the theme and the language. It folds to its icons (the button by the logo, or
  ⌘B; kept on this computer): the show becomes a tile with its first day, each icon says its
  name and shortcut on hover, the language is a menu.
- **Phone and tablet:** a navy header (back to the shows, the show, its sync state) and a
  bottom bar with four sections and *Plus* (the others, the settings, theme, language).
- **The shows** (library): a navy band with the app's name and the import menu; each show a
  card (number, disciplines, rehearsal, name, place, dates; entries, classes, results; *Ouvrir*
  and a ⋯ menu: export, *Archiver*, delete). The archived shows have a page of their own
  (*Archives* beside *Importer* once there is one; `#/archives`), by year, the same cards.
  Archiving is the responsable's (others see the item greyed, saying so); the account's dialog
  sets automatic archiving, some weeks after a show's last day.

## Colours and type

Tokens are Tailwind colours named by role (`bg-card`, `text-ink-3`, `border-line`…), defined as
CSS variables that the theme switches; a component never uses a raw colour for a surface or a
text.

| Token | Use |
| --- | --- |
| `page`, `card`, `raised`, `sunken` | Surfaces in layers: the page, a card on it, a header strip or a panel inside a dialog, a field or a track |
| `line`, `line-strong` | Borders, separators; outlined buttons |
| `ink`, `ink-2`, `ink-3`, `ink-4` | Text: main, secondary, hints, disabled and icons at rest |
| `brand` (navy #00283c) | The frame, page-header icons, a few strong tiles |
| `accent` (coral by default) | The one main action, the active tab and navigation item, selections; `on-accent` for text on it |
| `mark` | The logo's tile: the accent, the same in both themes |
| `info` (blue) | Focus rings, information, things received from or sent to Swiss Equestrian |
| `ok`, `warn`, `danger` | Done (final, sent); needs attention (running, changed since); destructive or wrong |

- **Dark theme:** deep navy, never black; the same roles, lifted colours.
- **Theme choice:** *Système* (the computer's), *Clair*, *Sombre*, in the frame and in the
  settings, remembered on the device (`localStorage.theme`), set as `data-theme` on `<html>`.
- **Accent choice** (the user, 2026-11-15): coral stays the default; *Pétrole*, *Bordeaux*,
  *Laiton* and *Cobalt* are offered in the settings, remembered on the device
  (`localStorage.accent`), set as `data-accent` on `<html>`. Every accent passes 4.5 : 1 for
  its text in both themes except coral's white button text (3.9 : 1, kept as chosen); in the dark
  theme the other accents take navy text (`on-accent`). Laiton moves `warn` to orange and cobalt
  moves `info` to cyan, so the accent is never a state colour.
- **Logo** (the user, 2026-11-15): the E, a horseshoe on its side with a bar, white on the
  accent (`ui/Logo.tsx`); the desktop icon in coral on navy, the favicon in coral (app/README.md).
- **Shadows:** `shadow-card` (cards), `shadow-lift` (hover, page-header icons), `shadow-pop`
  (menus, dialogs, popovers).
- **Type:** Inter (variable, with optical sizes); numbers with tabular figures (`tabular`).
  Page titles 22–24 px bold; card titles 15 px semibold; body 13–14 px; hints 12 px.
- **Series of a divided class** have their own colours (a sky, b amber, c emerald, d violet,
  e rose, f slate), white letters on them, the same in both themes.

## Building blocks (`app/src/ui`)

- **Button** — `primary` (coral, the main action), `secondary` (outlined), `ghost`, `subtle`,
  `danger` (outlined red), `danger-solid`, `brand`; sizes `sm`, `md`, `lg`; an icon, a key
  (`kbd`), `loading`. **IconButton** has a tooltip and an accessible label. **Segmented** is a
  choice among a few (theme, server, number of sections). Never hide a `Button` with a
  `hidden` class (its `inline-flex` wins): wrap it.
- **Modal** — title, description, an icon tile, sizes `sm` to `full`, a footer for the buttons;
  the first field gets the cursor (`focusFirst`), Esc closes. **ConfirmModal** asks before
  something big, optionally with a reason. `overlayOpen()` tells a screen's keys to stay quiet
  while a dialog or a menu is open.
- **Menu** — `MenuItem` (icon, a second line, a key, danger), `MenuCheck`, `MenuLabel`,
  `MenuSeparator`, `SubMenu`; places itself where there is room.
- **Tabs** — icon, a count, a dot for "needs attention", a key.
- **Surface** — `PageHeader` (title, subtitle, actions; wraps on phones), `Card` and
  `CardHeader`, `Pill`, `Callout`, `EmptyState` (icon, what it means, the action to take),
  `Field`, `Switch`, `Kbd`, `Stat`, `Loading` (the screen's shape while it loads), `Avatar`
  (initials, a colour per name), `Choice` (one way among several, as a radio card), `CheckRow`
  and `Tick` (things to tick in a list, with a row ticking them all).
- **Toasts** (`useToast`) — what was done, with *Annuler ⌘Z* when it can be undone; after an
  undo, *Rétablir ⌘⇧Z* (Ctrl+Y too) until the next change; errors.
- **Tooltip**, **Logo**, `icons.tsx` (what lucide lacks: `Horseshoe`).

## Patterns

- **A list of things that open:** rows with a leading tile (a class's number, an avatar, an
  icon), two lines of text, pills, a chevron; the whole row is the button.
- **Documents (Impressions):** tiles in a grid, coloured by group; a click makes the document.
- **Sending to Swiss Equestrian:** per class and list, a state pill (and when, by whom) and one
  action: *Envoyer* (not sent), *Renvoyer* in coral (changed since), or a ⋯ menu (resend,
  withdraw) when up to date. Production asks first.
- **A start list:** the few columns that matter, a search, a sort by any column (a click on
  its head: up, down, back to the start order). Sorting or searching changes what is shown,
  never the order, and drag and drop waits until the start order is shown again; a typed place
  works in any view. More columns (Rosson's: licence, passport, signalement, winning points…,
  and the horse's owner, breeder, pedigree) are ticked in *Colonnes*, only those the class has,
  remembered on the device.
- **Officials' days:** one key types a role in the selected cell; a click (a tap) opens the
  roles at the cell, with their keys; Shift extends along the row. An official leaves a day by
  *Retirer de la journée* (a confirmation). A judge with marks entered keeps the role (a dot on
  the cell; only the position may change): removing is refused in a dialog that names the
  classes, each a link to its results.
- **Officials' titles** (Juge national de saut…) are Swiss Equestrian's, in the office's
  language and in its order (`se_api::titles`, from info.swiss-equestrian.ch); the search
  finds people by title too.
- **History:** a timeline per day, an icon per kind of change.
- **A participant (a rider):** a dialog with two tabs, *Inscriptions* and *Contact*. The entries
  are a table, a block per horse (*Changer de cheval…*), a line per class: where it starts (a
  click shows the start list), NP and HC, the rider's remark and the office's note (a speech
  bubble writes it), a ⋯ menu (level, horses by phase, move to another class, remove an office
  entry). *Changer de cavalier…* above the table.
- **A class's settings (*Réglages*):** first what lists call it — *Nom de l’épreuve* (a prize's,
  « Prix … », as Rosson's: empty until typed, the lists then say its category and table), its
  category (Swiss Equestrian's, read only) and its *Barème* (jumping, before results), its day
  and time; one *Enregistrer* (Enter) for all, one ⌘Z. The name never says the table: they are
  printed and sent apart. Then the class's details (placed, sponsor, extra lines, logo…).
- **Importing a Rosson export:** the real show, as it ended, with all its data; one event opens
  at once; a file with several lists them ticked (those already here not), *Importer (n)*.
- **Training shows** (settings › *Exercice*): the practice event (*Créer*), Swiss Equestrian's
  demo events (*Télécharger*); made, the show opens. An « Exercice » pill on its card; its sync
  and *Serveur* panels only say it stays on this computer and sends to the simulation.
- **Files** (imports, exports, the timekeeper's file, Excel): the system's dialogs to open and
  to save, never a typed path. In a browser (development) the browser's own dialogs.
- **A pair entered in several classes gets another horse or rider:** after the pick, a dialog
  lists the pair's classes to tick (*Toutes ses épreuves* at the top), all ticked but those
  already ridden; a final class is locked. One change, one ⌘Z.

## Not like this

- No panel sliding in from the right or the bottom, no form opening under a row.
- No row of coral buttons: one main action per screen or dialog.
- No raw grey boxes and 1990s tables: surfaces, spacing, pills, icons.
- No small text links as buttons, no ✕ characters as icons (`IconButton` with `X`).
