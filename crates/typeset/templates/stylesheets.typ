// Style judges' sheets (jumping): an A5 page per rider and judge, in start order — licence
// classes judge 1's set, then judge 2's; E and ECav classes one judge. Gestion Concours' layout
// in EquiEvents' colours: the sheet's title and the class's logo; place, date, rider, horse and
// judge, the class and start number in a tag; the scale of marks; the criteria with their
// guiding ideas, a box for the mark and room for remarks; the total; the final result (the
// judges' points less the faults), time (licence classes) and rank.

#import "/common.typ": data, l, fonts, ink, muted, rule, brand, opt

#set document(title: data.title)
#set text(font: fonts, size: 8.6pt, fill: ink, lang: data.language, number-type: "lining")
#set par(leading: 0.45em)
#set page(paper: "a5", margin: (x: 10mm, top: 8mm, bottom: 7mm))

#let tint = rgb("#eef3f7")
#let hair = rgb("#cfd8e0")
#let licence = data.kind == "licence"
#let label(t) = text(size: 7.6pt, fill: muted)[#t]
#let caps(t, size: 6.8pt, fill: brand) = text(size: size, weight: "bold", tracking: 0.12em, fill: fill, upper(t))
#let blank(w) = box(width: w, height: 5.5mm, stroke: (bottom: 0.7pt + ink))

#let sheet(p) = {
  // Title and the class's logo.
  grid(
    columns: (1fr, auto),
    column-gutter: 4mm,
    align: (left + horizon, right + horizon),
    [
      #caps[#l.sheet#if licence [ · #p.judge_label]]
      #v(-4pt)
      #text(size: 12.5pt, weight: "bold", fill: brand)[#data.heading]
    ],
    if data.logo != none { image(data.logo, height: 13mm, width: 28mm, fit: "contain") },
  )
  v(1mm)
  line(length: 100%, stroke: 1pt + brand)
  v(1.5mm)

  // Place, date, rider, horse, judge; the class and start number in one tag.
  let part(title, value) = align(center, {
    set text(top-edge: "cap-height", bottom-edge: "baseline")
    set par(spacing: 0pt, leading: 0pt)
    caps(title, size: 5.8pt)
    v(2.4mm)
    text(size: 22pt, weight: "bold", fill: ink)[#value]
  })
  let tag = block(width: 100%, fill: tint, radius: 3pt, stroke: 0.7pt + brand, inset: (x: 1mm, top: 2.4mm, bottom: 2.4mm), grid(
    columns: (1fr, auto, 1fr),
    align: horizon,
    part(l.class, data.class_number),
    line(angle: 90deg, length: 11mm, stroke: (paint: brand, thickness: 0.7pt, dash: (2pt, 1.5pt))),
    part(l.start, p.number),
  ))
  grid(
    columns: (1fr, 38mm),
    column-gutter: 4mm,
    align: (left + horizon, right + horizon),
    grid(
      columns: (auto, 1fr),
      column-gutter: 2.5mm,
      row-gutter: 2.2mm,
      align: (right + horizon, left + horizon),
      label(l.place), text(weight: "bold")[#data.place],
      label(l.date), [#data.date],
      label(l.rider), text(size: 10.5pt, weight: "bold")[#p.rider #text(size: 7.6pt, weight: "regular", fill: muted)[#p.rider_id]],
      label(l.horse), text(size: 10.5pt, weight: "bold")[#p.horse #text(size: 7.6pt, weight: "regular", fill: muted)[#p.horse_id]],
      label(l.judge), [#if p.judge == "" { blank(45mm) } else { text(weight: "bold")[#p.judge] } #h(2mm) #text(size: 7.6pt, weight: "bold", fill: brand)[#p.flag]],
    ),
    tag,
  )
  v(2.5mm)

  // The scale of marks.
  block(fill: tint, inset: (x: 3mm, y: 2mm), radius: 2pt, width: 100%, {
    set text(size: 7.2pt)
    caps(l.scale, fill: ink)
    if not licence { h(2mm); text(size: 7pt, fill: muted)[#l.halves] }
    v(-3pt)
    grid(
      columns: (1fr,) * 4,
      row-gutter: 1.4mm,
      ..data.scale.map(m => [#box(width: 4.5mm, align(right, text(weight: "bold")[#m.at(0)])) #h(1mm) #m.at(1)]),
    )
  })
  v(2.5mm)

  // The criteria: a box for the mark, room for remarks.
  table(
    columns: (25mm, 1fr, 12mm, 33mm),
    stroke: 0.6pt + hair,
    inset: (x: 2mm, y: 1.6mm),
    table.header(..(l.criterion, l.ideas, l.mark, l.remarks).map(h => caps(h, fill: ink))),
    ..data.criteria.map(c => (
      text(weight: "bold", size: 8pt)[#c.name],
      [#text(size: 7.4pt, fill: ink, style: "italic", list(marker: [–], indent: 0pt, body-indent: 0.45em, spacing: 0.42em, ..c.ideas)) #v(if c.ideas.len() == 1 { 5mm } else { 1.5mm })],
      table.cell(stroke: 0.9pt + ink)[],
      [],
    )).flatten(),
    table.cell(colspan: 2, stroke: none, align(right, text(weight: "bold")[#l.total])),
    table.cell(stroke: 1.2pt + ink, block(height: 5.5mm)),
    table.cell(stroke: none)[],
  )
  v(1fr)

  // The result: licence classes add the two judges' points; E and ECav one judge, no time.
  set text(size: 8.4pt)
  if licence {
    grid(
      columns: (auto,) * 9,
      column-gutter: 1.6mm,
      row-gutter: 1.3mm,
      align: horizon,
      text(weight: "bold")[#l.final], blank(14mm), [+], blank(14mm), [−], blank(12mm), [=], blank(14mm), [#l.points],
      [], align(center, label(l.judge_1)), [], align(center, label(l.judge_2)), [], align(center, label(l.faults)), [], [], [],
    )
    v(2mm)
    grid(
      columns: (auto, auto, 8mm, auto, auto),
      column-gutter: 2mm,
      align: horizon,
      text(weight: "bold")[#l.time], blank(22mm), [], text(weight: "bold")[#l.rank], blank(14mm),
    )
  } else {
    grid(
      columns: (auto,) * 7 + (1fr, auto, auto),
      column-gutter: 1.6mm,
      row-gutter: 1.3mm,
      align: horizon,
      text(weight: "bold")[#l.final], blank(16mm), [−], blank(14mm), [=], blank(16mm), [#l.points], [], text(weight: "bold")[#l.rank], blank(13mm),
      [], align(center, label(l.judge_points)), [], align(center, label(l.faults)), [], align(center, label(l.total_points)), [], [], [], [],
    )
  }
}

#for (i, p) in data.pages.enumerate() {
  if i > 0 { pagebreak() }
  sheet(p)
}
