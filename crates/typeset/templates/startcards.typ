// Start cards: one per starter, two to an A4 page (cut in two: A5 landscape), or one to an A5
// page (`paper`: "a5"; the whole class or one starter, the office's choice). The class and the
// starter's number on top; who rides what (licence, club, address, phone; passport,
// signalement, pedigree, owner, winning points); then each round the class has (its scoring
// table: round or phase 1, round 2 or phase 2, jump-offs, winning round): a numbered box per
// obstacle, below them what the jury adds up — penalties: (carried over +) obstacles + time
// penalties; time: (carried over +) time + correction (table C: time + penalty seconds +
// correction) — and at the right the round's totals side by side, penalties (or points) and
// time, in the same place on every card to rank them. The boxes as tall as the card allows. At
// the top right, as on the style sheets, one tag: the start number, and the rank to write in.

#import "/common.typ": data, l, fonts, ink, muted, rule, brand, accent, opt, write-box, obstacle-boxes, part-boxes, total-boxes, sum-width, fitting

#let a5 = data.at("paper", default: "a4") == "a5"
// A5 landscape is the top half of an A4 page.
#set page(width: 210mm, height: if a5 { 148.5mm } else { 297mm }, margin: 0pt)
#set text(font: fonts, size: 8.5pt, lang: data.language, fill: ink)
#set par(leading: 0.4em)

#let c = data.class
#let max-obstacles = calc.max(..data.rounds.map(r => r.obstacles))
#let label(it) = text(size: 6.5pt, fill: muted, upper(it))
#let tint = rgb("#eef3f7")
#let caps(t) = text(size: 5.8pt, weight: "bold", tracking: 0.12em, fill: brand, upper(t))

// One person or horse: its name, then lines of details.
#let identity(title, name, ..lines) = block(width: 100%)[
  #label(title) \
  #text(size: 11.5pt, weight: "bold")[#name]
  #for line in lines.pos().filter(x => x != none and x != "") [ \ #line]
]

#let card(s) = box(width: 100%, height: 148.5mm, inset: (x: 9mm, top: 8mm, bottom: 6mm), clip: true, {
  // Swiss Equestrian's logo, the class, the number.
  grid(
    columns: (auto, 1fr, auto),
    column-gutter: 8pt,
    align(horizon, image("/assets/swiss-equestrian.png", height: 17mm)),
    [
      #text(size: 7pt, fill: muted)[#data.event.title]
      #v(1pt)
      #text(size: 14pt, weight: "bold", fill: brand)[#l.class #c.number] #h(6pt) #text(size: 10pt, weight: "bold")[#c.category]
      #v(0pt)
      #text(size: 8pt, fill: muted)[#(c.scoring, c.date, c.time).filter(x => x != "").join("  ·  ")]
    ],
    align(right + top, stack(
      dir: ltr,
      spacing: 6pt,
      ..if s.hc { (align(horizon, box(fill: accent, inset: 4pt, radius: 2pt, text(fill: white, weight: "bold")[HC])),) },
      ..if s.level != "" { (align(horizon, box(stroke: 0.6pt + ink, inset: 4pt, radius: 2pt, text(weight: "bold")[#l.level #s.level])),) },
      // The start number (room to write one when the list has none yet) and the rank, to write
      // in: one tag, as on the style sheets.
      box(width: 62mm, height: 21mm, fill: tint, stroke: 0.7pt + brand, radius: 3pt, inset: (x: 1mm, y: 2.2mm), grid(
        columns: (1fr, auto, 1fr),
        rows: (100%,),
        align: center,
        grid(rows: (auto, 1fr), caps(l.number), align(horizon, stack(
          spacing: 2pt,
          text(size: 22pt, weight: "bold")[#opt(s.number)],
          ..if s.time != none { (text(size: 8pt, weight: "bold")[#s.time],) },
        ))),
        align(horizon, line(angle: 90deg, length: 15mm, stroke: (paint: brand, thickness: 0.7pt, dash: (2pt, 1.5pt)))),
        caps(l.rank),
      )),
    )),
  )
  v(4pt)
  line(length: 100%, stroke: 0.4pt + rule)
  v(4pt)
  // Who rides what.
  grid(
    columns: (1fr, 1.15fr),
    column-gutter: 14pt,
    identity(
      l.rider,
      s.rider,
      [#label(l.licence) #s.licence #if s.club != "" [ · #s.club]],
      if s.at("born", default: "") != "" [#label(l.born) #s.born],
      s.address,
      s.phone,
    ),
    identity(
      l.horse,
      s.horse,
      [#label(l.passport) #s.passport #if s.signalement != "" [ · #s.signalement]],
      if s.pedigree != "" [#label(l.pedigree) #s.pedigree],
      if s.owner != "" [#label(l.owner) #s.owner],
      [#label(l.winning_points) #s.gwp.at(0) · #s.gwp.at(1) · #s.gwp.at(2) #text(size: 6.5pt, fill: muted)[(#l.rider_short · #l.horse_short · #l.pair_short)]],
    ),
  )
  v(7pt)
  // The rounds: the boxes as tall as fits.
  block(width: 100%, height: 1fr, layout(size => {
    let label-w = 22mm
    // The totals' column, at the right of every round.
    let w-tot = 19mm
    let totals-w = 2 * w-tot + 1.5mm + 4mm
    let work = size.width - label-w - totals-w
    let n = max-obstacles
    let w-obs = (work - 0.8mm * (n - 1)) / n
    let w-sum = sum-width(data.rounds, work)
    let rounds(bh) = stack(spacing: if data.rounds.len() <= 2 { 4mm } else { 2.5mm }, ..data.rounds.map(r => grid(
      columns: (label-w, work, totals-w),
      align(left + horizon, text(size: 9pt, weight: "bold", fill: brand, r.label)),
      stack(spacing: 1mm, obstacle-boxes(r.obstacles, w-obs, bh), part-boxes(r.groups, w-sum, bh)),
      align(right, total-boxes(r.groups, w-tot, 2 * bh + 1mm)),
    )))
    rounds(fitting((14mm, 13mm, 12mm, 11mm, 10mm, 9mm, 8mm, 7.5mm, 7mm, 6.5mm, 6mm, 5.5mm, 5mm), size.height, size.width, rounds))
  }))
})

#if a5 {
  for (i, s) in data.cards.enumerate() {
    if i > 0 { pagebreak() }
    card(s)
  }
} else {
  for (i, pair) in data.cards.chunks(2).enumerate() {
    if i > 0 { pagebreak() }
    // The line to cut along, in the middle of the page.
    place(top + left, dy: 148.5mm, line(length: 100%, stroke: (paint: rule, dash: "dashed")))
    grid(rows: (148.5mm, 148.5mm), card(pair.at(0)), if pair.len() > 1 { card(pair.at(1)) })
  }
}
