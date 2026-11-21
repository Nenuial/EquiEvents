// Letters to the riders (Rosson's "Serienbrief"): one per rider, the address where a window
// envelope shows it (left or right), the office's text, the rider's entries (date, time, class,
// start number, horse, fee) and the total.

#import "/common.typ": data, l, fonts, ink, muted, rule, brand, opt

#set page(paper: "a4", margin: (top: 2cm, bottom: 2cm, x: 2cm))
#set text(font: fonts, size: 10pt, lang: data.language, fill: ink)
#set par(leading: 0.55em)

#for (i, letter) in data.letters.enumerate() {
  if i > 0 { pagebreak() }
  // The sender: the event.
  [#text(weight: "bold", fill: brand)[#data.event.title] \ #text(size: 8.5pt, fill: muted)[#data.event.place · #data.event.dates]]
  // The address, in the envelope's window (C5/DL: 45 mm from the top).
  place(
    top + left,
    dx: if data.window == "right" { 100mm } else { 0mm },
    dy: 2.5cm,
    box(width: 80mm, letter.address.join(linebreak())),
  )
  v(5cm)
  align(right, text(size: 9pt)[#data.place_date])
  v(6pt)
  if data.text != "" {
    data.text
    v(8pt)
  }
  table(
    columns: (auto, auto, auto, auto, 1fr, auto),
    stroke: (x, y) => if y > 0 { (bottom: 0.3pt + rule) },
    inset: (x: 4pt, y: 4pt),
    table.header(
      ..(l.date, l.time, l.class, l.number, l.horse, l.fee).map(h => text(size: 7.5pt, weight: "bold", fill: muted, upper(h))),
      table.hline(stroke: 0.6pt + ink),
    ),
    ..letter.entries.map(e => (e.date, opt(e.time), [*#e.class* #text(size: 8pt, fill: muted)[#e.category]], opt(e.number), e.horse, align(right, e.fee))).flatten(),
  )
  v(4pt)
  align(right, text(weight: "bold")[#l.total CHF #letter.total])
}
