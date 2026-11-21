// Team start cards: one page per team, in the rotation. The class and the team on top, its
// riders (number, horse with signalement and passport, rider with licence, level, winning
// points); then a block for each round — the scoring table's, always two rounds and a jump-off
// — per rider a numbered box per obstacle, below them what the jury adds up and at the right
// the totals side by side, as on the single start cards; in teams of four whether the result
// is the one dropped; the team's penalties and time under the riders' totals. Swiss
// Equestrian's logo at the top left; at the top right, one tag: the team, and its rank to write
// in. On an A4 page each, or an A5 page (`paper`: "a5": the same card, at 1/√2).

#import "/common.typ": *

#show: setup

#let c = data.class
#let label(it) = text(size: 6.5pt, fill: muted, upper(it))
#let caps(t) = text(size: 5.8pt, weight: "bold", tracking: 0.12em, fill: brand, upper(t))

#let card(team) = {
  grid(
    columns: (auto, 1fr, auto),
    column-gutter: 8pt,
    align(horizon, image("/assets/swiss-equestrian.png", height: 16mm)),
    [
      #text(size: 13pt, weight: "bold", fill: brand)[#l.class #c.number] #h(6pt) #text(size: 10pt, weight: "bold")[#c.category]
      #v(0pt)
      #text(size: 8pt, fill: muted)[#(c.scoring, c.date, c.time).filter(x => x != "").join("  ·  ")]
    ],
    // The team and its rank, to write in: one tag, as on the start cards.
    box(height: 18mm, fill: tint, stroke: 0.7pt + brand, radius: 3pt, inset: (x: 3mm, y: 2.2mm), grid(
      columns: (auto, auto, 26mm),
      rows: (100%,),
      column-gutter: 3mm,
      align: center,
      grid(rows: (auto, 1fr), caps[#l.team #team.order], align(horizon, text(size: 14pt, weight: "bold")[#team.name])),
      align(horizon, line(angle: 90deg, length: 12mm, stroke: (paint: brand, thickness: 0.7pt, dash: (2pt, 1.5pt)))),
      caps(l.rank),
    )),
  )
  v(6pt)
  table(
    columns: (2.6em, 1.5fr, 1.6fr, 2.6em, 3.6em, 4em, 3.6em),
    stroke: (x, y) => if y > 0 { (bottom: 0.3pt + rule) },
    head(l.number, l.horse, l.rider, l.level, l.points_short + " " + l.rider_short, l.points_short + " " + l.horse_short, l.points_short + " " + l.pair_short),
    ..team.riders.map(r => {
      let st = if r.not_starting { it => text(fill: muted, strike(it)) } else { it => it }
      (
        st(text(size: 11pt, weight: "bold")[#r.number]),
        st[*#r.horse* \ #text(size: 7pt, fill: muted)[#r.signalement · #r.passport]],
        st[#r.rider \ #text(size: 7pt, fill: muted)[#l.licence #r.licence]],
        st[#r.level],
        st(align(right)[#r.gwp_rider]), st(align(right)[#r.gwp_horse]), st(align(right)[#r.gwp_pair]),
      )
    }).flatten(),
  )
  // A block per round: per rider the obstacles and, below, their sums; the team's totals
  // under the riders' totals. The boxes as tall as the page allows.
  v(4pt)
  block(width: 100%, height: 1fr, layout(size => {
    let label-w = 19mm
    // The totals' column, at the right of every rider and of the team.
    let w-tot = 17mm
    let totals-w = 2 * w-tot + 1.5mm + 4mm
    let work = size.width - label-w - totals-w
    let n = calc.max(..data.rounds.map(r => r.obstacles))
    let w-obs = (work - 0.8mm * (n - 1)) / n
    let w-sum = sum-width(data.rounds, work)
    let rounds(bh) = stack(spacing: 3.5mm, ..data.rounds.map(r => stack(
      spacing: 1.6mm,
      text(size: 9.5pt, weight: "bold", fill: brand)[#r.label],
      ..team.riders.map(rd => grid(
        columns: (label-w, work, totals-w),
        align(left + horizon, {
          text(size: 8pt)[*#rd.number* #rd.short]
          if data.dropped [ \ #box(width: 2.6mm, height: 2.6mm, stroke: 0.6pt + ink) #text(size: 6.5pt, fill: muted)[#l.dropped]]
        }),
        stack(spacing: 0.8mm, obstacle-boxes(r.obstacles, w-obs, bh), part-boxes(r.groups, w-sum, bh)),
        align(right, total-boxes(r.groups, w-tot, 2 * bh + 0.8mm)),
      )),
      grid(
        columns: (label-w + work, totals-w),
        align(right + horizon, pad(right: 4pt, text(size: 8pt, weight: "bold")[#l.team_total])),
        align(right, total-boxes(r.groups, w-tot, bh + 2mm, team: true)),
      ),
    )))
    rounds(fitting((10mm, 9.5mm, 9mm, 8.5mm, 8mm, 7.5mm, 7mm, 6.5mm, 6mm, 5.5mm, 5mm, 4.5mm), size.height, size.width, rounds))
  }))
}

// A4: the page's own size. A5: the A4 card at 1/√2 (page, margins, header and footer alike).
#let a5 = data.at("paper", default: "a4") == "a5"
#let k = 70.71%
#let small(body) = scale(k, reflow: true, origin: top + left, block(width: 180mm, body))
#set page(paper: "a5", margin: (top: 16.3mm, bottom: 11.3mm, x: 10.6mm), header: small(page-header), footer: small(page-footer)) if a5

#for (i, team) in data.teams.enumerate() {
  if i > 0 { pagebreak() }
  if a5 { small(block(height: 258mm, card(team))) } else { card(team) }
}
