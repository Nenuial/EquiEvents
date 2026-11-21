// What every print shares: the page, its header (the event) and footer (when printed, page
// numbers), colours, and the parts of a class's head. The data is `/data.json`; the words are
// in `data.labels`, in the office's language.

#let data = json("/data.json")
#let l = data.labels

#let ink = rgb("#111827")
#let muted = rgb("#6b7280")
#let rule = rgb("#d1d5db")
#let shade = rgb("#f3f4f6")
#let brand = rgb("#00283c")
#let accent = rgb("#c2410c")

#let fonts = ("Helvetica Neue", "Segoe UI", "Arial", "DejaVu Sans", "Liberation Sans", "Libertinus Serif")

// Text that may be missing.
#let opt(value) = if value == none { "" } else { str(value) }

#let page-header = {
  let e = data.event
  grid(
    columns: (1fr, auto),
    gutter: 8pt,
    [
      #text(weight: "bold", fill: brand, size: 9.5pt)[#opt(e.title)] \
      #text(fill: muted, size: 8pt)[#opt(e.place) · #opt(e.dates)]
    ],
    align(right + bottom, text(fill: muted, size: 8pt)[#opt(e.number)]),
  )
  v(-4pt)
  line(length: 100%, stroke: 0.6pt + brand)
}

#let page-footer = context {
  set text(size: 7pt, fill: muted)
  grid(
    columns: (1fr, auto),
    [#l.printed #data.printed · #data.title · EquiEvents],
    [#l.page #counter(page).display() / #counter(page).final().first()],
  )
}

#let setup(body, flipped: false) = {
  set document(title: data.title)
  set text(font: fonts, size: 8.5pt, lang: data.language, fill: ink)
  set par(leading: 0.45em)
  set page(
    paper: "a4",
    flipped: flipped,
    margin: (top: 2.3cm, bottom: 1.6cm, x: 1.5cm),
    header: page-header,
    header-ascent: 25%,
    footer: page-footer,
  )
  set table(stroke: none, inset: (x: 4pt, y: 3.2pt))
  body
}

// The class's number in a tag at the top right, as on the start cards and style sheets.
#let tint = rgb("#eef3f7")
#let class-tag(number) = box(fill: tint, stroke: 0.7pt + brand, radius: 3pt, inset: (x: 4.5mm, y: 2.4mm), align(center, {
  set par(leading: 0pt, spacing: 0pt)
  text(size: 5.8pt, weight: "bold", tracking: 0.12em, fill: brand, upper(l.class))
  v(2.2mm)
  text(size: 20pt, weight: "bold", fill: ink, opt(number))
}))

// French quotes kept with what they quote (« Prix … »): never a line to themselves.
#let nb(name) = opt(name).replace("« ", "«\u{A0}").replace(" »", "\u{A0}»")

// One thing about the class: its label small above its value. Side by side, as many to a line
// as fit.
#let fact(label, value) = box(inset: (right: 5mm, bottom: 2.4mm), stack(
  spacing: 1.5mm,
  text(size: 5.8pt, weight: "bold", tracking: 0.1em, fill: muted, upper(label)),
  text(size: 9pt, weight: "bold", value),
))

// A class's head: its name, then what it is (date, time, category, scoring…) as labelled
// facts; its number in a tag at the top right (with its logo); below a hairline, the class's own
// lines (sponsors, prizes); its officials.
#let class-head(c) = block(breakable: false, below: 9pt, {
  let logo = c.at("logo", default: none)
  grid(
    columns: (1fr, auto),
    column-gutter: 6mm,
    align: (left + top, right + top),
    {
      text(size: 13pt, weight: "bold", fill: brand)[#nb(c.name)]
      v(2.6mm)
      c.at("details", default: c.info).map(p => fact(p.at(0), p.at(1))).join()
    },
    grid(
      columns: if logo != none { (auto, auto) } else { (auto,) },
      column-gutter: 3mm,
      align: horizon,
      ..if logo != none { (image(logo, height: 14mm, width: 28mm, fit: "contain"),) },
      class-tag(c.number),
    ),
  )
  let extra = c.at("extra", default: ())
  if extra.len() > 0 {
    block(width: 100%, above: 0.6mm, inset: (top: 2.2mm), stroke: (top: 0.4pt + rule), {
      set text(size: 7.5pt)
      grid(columns: (auto, 1fr), column-gutter: 4mm, row-gutter: 1.5mm, ..extra.map(p => (text(fill: muted, p.at(0)), p.at(1))).flatten())
    })
  }
  if c.at("officials", default: ()).len() > 0 {
    block(above: 2.4mm, text(size: 7.5pt, fill: muted, c.officials.map(o => [#o.at(0): #text(fill: ink)[#o.at(1)]]).join([ #h(3pt)·#h(3pt) ])))
  }
})

// A line across a table: a heading (a level, a team) or a note (a break).
#let full-row(columns, kind, body) = {
  if kind == "heading" {
    (table.cell(colspan: columns, inset: (top: 8pt, bottom: 3pt, x: 4pt), text(weight: "bold", size: 9pt, fill: brand, body)),)
  } else {
    (table.cell(colspan: columns, fill: shade, text(style: "italic", fill: muted, body)),)
  }
}

// A table's head row.
#let head(..cells) = table.header(
  ..cells.pos().map(c => text(size: 7pt, weight: "bold", fill: muted, upper(c))),
  table.hline(stroke: 0.6pt + ink),
)

// Who and on what horse, on two lines.
#let who(name, sub) = [#text(weight: "bold")[#name]#if sub != "" [ \ #text(size: 7pt, fill: muted)[#sub]]]

// Start cards (single and team): a box to write in, its label small in the corner (bold edge
// for a total).
#let write-box(it, w, bh, strong: false) = box(
  width: w,
  height: bh,
  stroke: if strong { 1.3pt + ink } else { 0.5pt + ink },
  inset: (x: 1.5pt, y: 1.3pt),
  align(left + top, text(size: 5.5pt, fill: muted, it)),
)

// A round's obstacles: a numbered box each, `w` wide.
#let obstacle-boxes(n, w, bh) = stack(dir: ltr, spacing: 0.8mm, ..range(n).map(i => write-box(str(i + 1), w, bh)))

// What the jury adds up in a round: each sum's parts (penalties: carried over, obstacles, time
// penalties; time: carried over, time, correction), "+" between them — or "−" when the parts
// are taken from the first (points less time penalties; the style note less the penalties).
#let part-boxes(groups, w, bh) = {
  let op(sym) = box(width: 4mm, height: bh, align(center + horizon, text(size: 10pt, weight: "bold", fill: muted, sym)))
  let parts = ()
  for g in groups.filter(g => g.boxes.len() > 0) {
    if parts.len() > 0 { parts.push(box(width: 6mm)) }
    for (i, b) in g.boxes.enumerate() {
      if i > 0 { parts.push(op(if g.at("minus", default: false) { "−" } else { "+" })) }
      parts.push(write-box(b, w, bh))
    }
  }
  stack(dir: ltr, ..parts)
}

// A round's totals side by side, in the same place on every card (to rank the cards): penalties
// or points, then time; a sum the round does not have leaves its place empty. `team`: the team's.
#let total-boxes(groups, w, bh, team: false) = {
  let slot(s) = {
    let g = groups.find(g => g.slot == s)
    if g == none { box(width: w, height: bh) } else { write-box(if team { g.team } else { g.total }, w, bh, strong: true) }
  }
  stack(dir: ltr, spacing: 1.5mm, slot(0), slot(1))
}

// How wide the part boxes can be in `width` (at most 17 mm): the round with the most fits.
#let sum-width(rounds, width) = calc.min(17mm, ..rounds.map(r => {
  let gs = r.groups.filter(g => g.boxes.len() > 0)
  let boxes = gs.map(g => g.boxes.len()).sum(default: 0)
  if boxes == 0 { 17mm } else { (width - (boxes - gs.len()) * 4mm - calc.max(0, gs.len() - 1) * 6mm) / boxes }
}))

// The tallest box height of `candidates` with which `body(bh)` fits in `height`.
#let fitting(candidates, height, width, body) = {
  let found = candidates.find(bh => measure(width: width, body(bh)).height <= height)
  if found == none { candidates.last() } else { found }
}
