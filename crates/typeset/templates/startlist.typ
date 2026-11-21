// Start lists: one class after the other, each with its head and its starters in order; levels
// and teams as headings, breaks and notes across the table. A starter who does not start is
// struck through.

#import "/common.typ": *

#show: setup

#let variant = data.at("variant", default: "full")

// The short list: one line a starter.
#let lite(c) = {
  let cols = c.columns
  let widths = (2.6em, 1.1fr, 0.9fr, 2.8em, 1.3fr, 1fr)
  let heads = (l.number, l.horse, l.signalement, l.points, l.rider, l.owner)
  if cols.time { widths.insert(0, 3.2em); heads.insert(0, l.time) }
  let n = widths.len()
  let rows = ()
  for line in c.lines {
    if line.kind != "starter" {
      rows += full-row(n, line.kind, line.text)
      continue
    }
    let st = if line.not_starting { it => text(fill: muted, strike(it)) } else { it => it }
    if cols.time { rows.push(st(text(weight: "bold")[#opt(line.time)])) }
    rows.push(st(text(weight: "bold")[#opt(line.number) #text(fill: accent, size: 7pt)[#line.mark]]))
    rows.push(st(text(weight: "bold")[#line.horse]))
    rows.push(st(text(size: 7.5pt)[#line.signalement]))
    rows.push(st(align(right)[#line.points]))
    rows.push(st[#line.rider#if line.town != "" [, #line.town]])
    rows.push(st(text(size: 7.5pt)[#line.owner]))
  }
  set text(size: 8pt)
  table(columns: widths, inset: (x: 3pt, y: 2.5pt), stroke: (x, y) => if y > 0 { (bottom: 0.3pt + rule) }, head(..heads), ..rows)
}

// For the judges, as Rosson's (Swiss Equestrian's office looks for them there): the class a line
// each on the left; on the same lines to the right, what the judge fills in — from … to …,
// starters without and with hc, finished, placed, the judge's visa — on a short line. Below, set
// apart: the class's own lines (sponsors) and how many are entered, hors concours, not starting.
#let judges-head(c) = block(breakable: false, below: 10pt, {
  let f = data.at("judges_fields", default: (from: "", to: "", counts: ()))
  let write(w) = box(width: w, height: 0.85em, stroke: (bottom: 0.5pt + ink))
  set text(size: 8.5pt)
  // The list's name and the class's; its number in the tag at the top right, as on the other lists.
  grid(
    columns: (1fr, auto),
    column-gutter: 6mm,
    align: (left + top, right + top),
    {
      text(size: 7.5pt, weight: "bold", fill: muted, tracking: 0.04em, upper(data.at("list_title", default: "")))
      v(3pt)
      text(size: 12pt, weight: "bold", fill: brand)[#nb(c.name)]
    },
    class-tag(c.number),
  )
  v(3mm)
  let details = c.at("details", default: c.info)
  let cells = ()
  // From … to … on the first line, then the counts and the visa.
  for i in range(calc.max(details.len(), f.counts.len() + 1)) {
    let d = details.at(i, default: none)
    let k = if i > 0 { f.counts.at(i - 1, default: none) }
    cells += (
      if d != none { text(fill: muted)[#d.at(0):] } else [],
      if d != none { text(weight: "bold", d.at(1)) } else [],
    )
    cells += if i == 0 {
      (grid.cell(colspan: 2, align(right)[#f.from #write(20mm) #h(6pt) #f.to #write(20mm)]),)
    } else {
      (if k != none [#k] else [], if k != none { write(100%) } else [])
    }
  }
  grid(columns: (auto, 1fr, auto, 26mm), column-gutter: 8pt, row-gutter: 5pt, align: left + bottom, ..cells)
  let extra = c.at("extra", default: ())
  let counts = c.at("counts", default: ())
  block(above: 7pt, inset: (y: 4pt), width: 100%, stroke: (top: 0.6pt + ink, bottom: 0.6pt + ink), grid(
    columns: (1fr, auto),
    column-gutter: 16pt,
    {
      set text(size: 7.5pt)
      grid(columns: (auto, 1fr), column-gutter: 8pt, row-gutter: 3pt, ..extra.map(p => (text(fill: muted)[#p.at(0):], p.at(1))).flatten())
    },
    grid(columns: (auto, 2em), column-gutter: 10pt, row-gutter: 3pt, ..counts.map(p => ([#p.at(0):], align(right, str(p.at(1))))).flatten()),
  ))
  // All the class's officials (the other lists name only the judges at a position).
  let officials = c.at("all_officials", default: c.at("officials", default: ()))
  if officials.len() > 0 {
    v(3pt)
    text(size: 7.5pt, fill: muted, officials.map(o => [#o.at(0): #text(fill: ink)[#o.at(1)]]).join([ #h(3pt)·#h(3pt) ]))
  }
})

#for (i, c) in data.classes.enumerate() {
  if i > 0 { pagebreak() }
  if variant == "judges" { judges-head(c) } else { class-head(c) }
  if variant == "lite" {
    lite(c)
    continue
  }
  let cols = c.columns
  // No place column: the start number says it (before numbers, the list is by name).
  let widths = ()
  let heads = ()
  if cols.time { widths.push(3.2em); heads.push(l.time) }
  if cols.number { widths.push(2.6em); heads.push(l.number) }
  let licence = cols.at("licence", default: false)
  let width = cols.at("width", default: false)
  if licence { widths.push(4.6em); heads.push(l.licence) }
  widths += (1fr, 1.15fr)
  heads += (l.rider, l.horse)
  if width { widths.push(3.6em); heads.push(l.width) }
  if cols.carried { widths.push(3.4em); heads.push(l.carried) }
  widths.push(2.4em)
  heads.push("")
  if variant == "judges" { widths.push(26%); heads.push(l.result) }
  let n = widths.len()

  let rows = ()
  for line in c.lines {
    if line.kind != "starter" {
      rows += full-row(n, line.kind, line.text)
      continue
    }
    let strike = if line.not_starting { it => text(fill: muted, strike(it)) } else { it => it }
    if cols.time {
      // The marathon: phase B's start under A's.
      let b = line.at("time_b", default: none)
      rows.push(if b == none { text(weight: "bold")[#opt(line.time)] } else { stack(spacing: 2pt, text(weight: "bold")[#opt(line.time)], text(size: 7pt, fill: muted, b)) })
    }
    if cols.number { rows.push(text(weight: "bold")[#opt(line.number)]) }
    if licence { rows.push(box(text(size: 7.5pt, hyphenate: false)[#line.at("licence", default: "").replace("-", "\u{2011}")])) }
    rows.push(strike(who(line.rider, line.rider_sub)))
    rows.push(strike(who(line.horse, line.horse_sub)))
    if width { rows.push(align(right)[#line.at("width", default: "")]) }
    if cols.carried { rows.push(align(right, text(weight: "bold")[#opt(line.carried)])) }
    rows.push(align(right, text(weight: "bold", fill: accent)[#line.mark]))
    // The judge writes the result here: two boxes (Rosson's).
    if variant == "judges" { rows.push(grid(columns: (1fr, 1fr), rows: 2.3em, stroke: 0.6pt + ink, [], [])) }
  }

  table(
    columns: widths,
    stroke: (x, y) => if y > 0 { (bottom: 0.3pt + rule) },
    head(..heads),
    ..rows,
  )
  if c.at("note", default: none) != none {
    v(4pt)
    text(size: 7.5pt, fill: muted, c.note)
  }
}
