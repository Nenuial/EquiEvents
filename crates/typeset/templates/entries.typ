// The entries in detail (Rosson's "Detail Nennungen"): rider by rider, with their phones; under
// each horse its entries: class, start number, fees, what was paid (in bold when less than the
// fee), the rider's remark and the office's note (in italics).

#import "/common.typ": *

#show: setup.with(flipped: true)

#block(below: 8pt, text(size: 14pt, weight: "bold", fill: brand)[#data.title])

#let rows = ()
#for p in data.riders {
  rows.push(table.cell(colspan: 6, inset: (top: 7pt, bottom: 2pt, x: 4pt))[
    #text(weight: "bold", size: 9.5pt)[#p.rider] #h(6pt) #text(fill: muted)[#p.town]
    #if p.phones != "" [ #h(10pt) #text(size: 7.5pt)[#p.phones] ]
  ])
  for h in p.horses {
    rows.push(table.cell(colspan: 6, inset: (top: 2pt, bottom: 1pt, x: 10pt), text(weight: "bold")[#h.horse]))
    for e in h.entries {
      rows.push(pad(left: 10pt)[#e.class])
      rows.push[#e.number]
      rows.push(align(right)[#e.fee])
      rows.push(align(right)[#e.fee2])
      rows.push(align(right, if e.underpaid { text(weight: "bold", fill: accent)[#e.paid] } else [#e.paid]))
      rows.push[#e.remarks#if e.comment != "" [ #text(style: "italic", fill: muted)[#e.comment]]]
    }
  }
}
#table(
  columns: (1.2fr, 0.8fr, 0.8fr, 0.8fr, 0.8fr, 4fr),
  stroke: (x, y) => if y > 0 { (bottom: 0.2pt + rule) },
  head(l.class, l.number, l.fee, l.fee2, l.paid, l.remarks),
  ..rows,
)
