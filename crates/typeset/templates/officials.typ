// The officials' plan: every official, alphabetically, with their roles at the event and, day
// by day, the classes they work in and in what role.

#import "/common.typ": *

#show: setup

#block(below: 8pt, text(size: 14pt, weight: "bold", fill: brand)[#l.officials_plan])

#let rows = ()
#for o in data.officials {
  rows.push(who(o.name, o.town))
  rows.push(text(size: 8pt)[#o.event_roles])
  rows.push(o.days.map(d => [#text(weight: "bold")[#d.date] #h(4pt) #d.classes]).join(linebreak()))
}
#table(
  columns: (1fr, 1fr, 1.6fr),
  stroke: (x, y) => if y > 0 { (bottom: 0.3pt + rule) },
  head(l.official, l.event_roles, l.classes),
  ..rows,
)
