// The participants: every rider, alphabetically, with their horses and the classes each starts
// in (start numbers when given).

#import "/common.typ": *

#show: setup

#block(below: 8pt, text(size: 14pt, weight: "bold", fill: brand)[#l.participants])

#let rows = ()
#for p in data.riders {
  rows.push(table.cell(rowspan: p.horses.len(), who(p.rider, p.rider_sub)))
  for h in p.horses {
    rows.push(text(weight: "bold")[#h.horse])
    rows.push(text(size: 7.5pt)[#h.classes])
  }
}
#table(
  columns: (1fr, 1fr, 1.2fr),
  stroke: (x, y) => if y > 0 { (bottom: 0.3pt + rule) },
  head(l.rider, l.horse, l.classes),
  ..rows,
)
