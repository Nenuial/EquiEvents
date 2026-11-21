// Rankings: per class, rank, rider, horse and the class's result columns (penalties and times,
// or each judge's marks and the percentage). Placed riders' ranks in colour; teams on a shaded
// line, their riders under them.

#import "/common.typ": *

#show: setup

#for (i, c) in data.classes.enumerate() {
  if i > 0 { pagebreak() }
  class-head(c)
  let extra = c.columns.len()
  let widths = (3.4em, 1fr, 1.1fr) + range(extra).map(_ => auto)
  let rows = ()
  for line in c.lines {
    if line.at("kind", default: "line") != "line" {
      rows += full-row(3 + extra, line.kind, line.text)
      continue
    }
    let fill = if line.team { shade } else { none }
    let rank = if line.rank == none { [–] } else if line.placed { text(fill: accent, weight: "bold")[#line.rank] } else { [#line.rank] }
    rows.push(table.cell(fill: fill, align(right, rank)))
    rows.push(table.cell(fill: fill, who(line.rider, line.rider_sub)))
    rows.push(table.cell(fill: fill, who(line.horse, line.horse_sub)))
    for (j, v) in line.cells.enumerate() {
      let last = j == extra - 1
      rows.push(table.cell(fill: fill, align(right, if last { text(weight: "bold")[#v] } else { [#v] })))
    }
  }
  table(
    columns: widths,
    stroke: (x, y) => if y > 0 { (bottom: 0.3pt + rule) },
    head(l.rank, l.rider, l.horse, ..c.columns),
    ..rows,
  )
  if c.at("summary", default: none) != none {
    v(4pt)
    text(size: 7.5pt, fill: muted, c.summary)
  }
}
