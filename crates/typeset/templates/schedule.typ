// The timetable: day by day, the classes in their order with their time, what they are, how
// many start, and their judges.

#import "/common.typ": *

#show: setup

#block(below: 8pt, text(size: 14pt, weight: "bold", fill: brand)[#l.schedule])

#for day in data.days {
  block(above: 10pt, below: 4pt, text(size: 10pt, weight: "bold", fill: brand)[#day.date])
  let rows = ()
  for c in day.classes {
    rows.push(text(weight: "bold")[#opt(c.time)])
    rows.push(text(weight: "bold", size: 10pt)[#c.number])
    rows.push(who(c.name, c.sub))
    rows.push(align(right)[#c.starters])
    rows.push(text(size: 7.5pt)[#c.judges])
  }
  table(
    columns: (3.4em, 2.6em, 1fr, auto, 1fr),
    stroke: (x, y) => if y > 0 { (bottom: 0.3pt + rule) },
    head(l.time, l.class, "", l.starters, l.judges),
    ..rows,
  )
}
