// Driving's control sheets (Rosson's Kontrollblatt), a page each: the competitor's head; cones
// and derby: the class, the carriage's widths, each round's gates to tick and its sums; the
// marathon's and the derby's obstacle sheet: the rules' penalties with room to tick and add,
// the obstacle's times, the gates' order, the obstacle and the judge's signature.

#import "/common.typ": data, l, setup, ink, muted, rule, brand, shade, opt

#show: setup

#let blank(w: 100%) = box(width: w, height: 1.25em, stroke: (bottom: 0.5pt + ink))
#let caps(t) = text(size: 7pt, weight: "bold", fill: brand, tracking: 0.1em, upper(t))
#let field(label, value: none, w: 100%) = grid(
  columns: (auto, 1fr),
  column-gutter: 4pt,
  align: bottom,
  text(fill: muted)[#label],
  if value == none or value == "" { blank(w: w) } else { box(width: w, stroke: (bottom: 0.5pt + ink), inset: (bottom: 2pt), text(weight: "bold", value)) },
)

#let head(s) = {
  grid(
    columns: (1.6fr, 1fr, 1.4fr),
    column-gutter: 10pt,
    field(l.name, value: s.name),
    field(l.carriage, value: s.carriage),
    field(l.class, value: s.class),
  )
  v(8pt)
}

// A round's gates, 1 to 20, a box under each to tick.
#let gates(label) = {
  if label != "" { caps(label); v(3pt) }
  table(
    columns: (1fr,) * 10,
    stroke: 0.5pt + rule,
    align: center,
    ..range(1, 11).map(n => text(fill: muted, size: 7pt)[#n]),
    ..range(10).map(_ => box(height: 1.6em)),
    ..range(11, 21).map(n => text(fill: muted, size: 7pt)[#n]),
    ..range(10).map(_ => box(height: 1.6em)),
  )
  v(4pt)
}

// A round's sums: cones in penalties (the allowed time filled in), the derby in seconds.
#let sums(r, cones) = {
  let unit = if cones { l.penalties } else { l.seconds }
  grid(
    columns: (1fr, 1fr),
    column-gutter: 16pt,
    row-gutter: 7pt,
    field(l.driven + " (" + l.seconds + ")"),
    field(l.obstacle_faults),
    field(l.allowed + " (" + l.seconds + ")", value: r.allowed),
    field(l.other),
    ..if cones { (field(l.over + " (" + l.seconds + ")"), field(l.time_penalty)) },
    [], field(text(weight: "bold")[#l.total#if r.label != "" [ #r.label] (#unit)]),
  )
  v(12pt)
}

#let obstacle-sheet(s) = {
  head(s)
  caps(if data.kind == "derby" { l.obstacle_derby } else { l.obstacle_marathon })
  v(4pt)
  text(size: 8pt, data.info)
  v(3pt)
  set text(size: 7.6pt)
  table(
    columns: (1fr, 6.5em, 7em, 4em),
    stroke: (x, y) => (bottom: 0.4pt + rule),
    inset: (x: 4pt, y: 3pt),
    table.header(..(l.description, if data.kind == "derby" { l.penalty_seconds } else { l.penalties }, l.notes, l.total).map(t => text(weight: "bold", t))),
    ..data.catalogue.map(c => (c.text, align(center, c.penalty), [], [])).flatten(),
    table.cell(colspan: 3, text(weight: "bold", if data.kind == "derby" { l.total_seconds } else { l.total_points })), box(height: 1.3em, stroke: 0.6pt + ink, width: 100%),
  )
  v(4pt)
  text(size: 7pt, fill: muted, data.timing)
  v(6pt)
  set text(size: 8.5pt)
  grid(
    columns: (1fr, 1fr),
    column-gutter: 16pt,
    row-gutter: 7pt,
    field(l.clock_1), field(l.clock_2),
    field(l.course_time), field(l.neutralised),
    field(l.rounded), field(text(weight: "bold", l.total_time)),
  )
  v(8pt)
  caps(l.gates_order)
  v(3pt)
  table(columns: (1fr,) * 8, stroke: 0.5pt + rule, ..range(8).map(_ => box(height: 1.6em)))
  v(8pt)
  grid(
    columns: (1fr, 2fr),
    column-gutter: 16pt,
    field(l.obstacle, value: if s.obstacle == none { none } else { str(s.obstacle) }),
    field(l.signature),
  )
}

#for (i, s) in data.sheets.enumerate() {
  if i > 0 { pagebreak() }
  if data.kind == "marathon" {
    obstacle-sheet(s)
    continue
  }
  head(s)
  caps(l.discipline + " · " + if data.kind == "cones" { l.title_cones } else { l.title_derby })
  v(6pt)
  grid(
    columns: (1fr, 1fr, 1fr),
    column-gutter: 12pt,
    field(l.width, value: s.width), field(l.allowance), field(l.gate),
  )
  v(10pt)
  for r in s.rounds {
    gates(r.label)
    sums(r, data.kind == "cones")
  }
  if data.kind == "cones" and s.rounds.len() > 1 {
    field(text(weight: "bold", l.total_rounds + " (" + l.penalties + ")"), w: 8em)
    v(10pt)
  }
  field(l.signature)
  // The derby's obstacle sheet on its own page.
  if data.kind == "derby" {
    pagebreak()
    obstacle-sheet(s)
  }
}
