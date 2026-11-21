// A ranking over several classes: each class's result, the total, what broke ties.

#import "/common.typ": *

#show: setup

#let r = data.ranking
#block(below: 8pt)[
  #text(size: 14pt, weight: "bold", fill: brand)[#r.name] \
  #text(fill: muted)[#r.rule]
]

#let k = r.classes.len()
#let rows = ()
#for line in r.lines {
  let rank = if line.rank == none { [–] } else { text(weight: "bold")[#line.rank] }
  rows.push(align(right, rank))
  rows.push(who(line.rider, line.rider_sub))
  rows.push(who(line.horse, ""))
  for cell in line.cells {
    rows.push(align(right)[#cell.at(0) #if cell.at(1) != "" [ \ #text(size: 7pt, fill: muted)[#cell.at(1)]]])
  }
  rows.push(align(right, text(weight: "bold")[#opt(line.total)]))
  rows.push(align(right, text(fill: muted)[#opt(line.tie_break)]))
}
#table(
  columns: (3.4em, 1fr, 1fr) + range(k).map(_ => auto) + (auto, auto),
  stroke: (x, y) => if y > 0 { (bottom: 0.3pt + rule) },
  head(l.rank, l.rider, l.horse, ..r.classes, l.total, l.tie_break),
  ..rows,
)
