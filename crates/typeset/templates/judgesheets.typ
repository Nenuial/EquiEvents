// Dressage judges' sheets: Swiss Equestrian's official form, as Rosson prints it (its pdfmake
// layout, in points): the federation's head and logo; the test's title, arena, time, bridle and
// edition; start number, rider, horse, class, judge and position, date and place; the test's
// tables — movements (letters, exercises, directive ideas), criteria, penalty points — each with
// its subtotal; the result (total points, percent of the maximum) and the judge's signature. A
// sheet over two pages carries its subtotals over ("Report à la page 2", "Report de la page
// 1"). Judge by judge, a sheet per rider; optionally a memo sheet first in each judge's set,
// marked across, for the judge's own notes.

#let data = json("/data.json")
#let l = data.labels
#let navy = rgb("#00283c")
// The form's fields to fill in.
#let fill-out = rgb("#ffaa88")
// Swiss Equestrian's typeface when the computer has it, else a close one.
#let fonts = ("Gilroy", "Helvetica Neue", "Helvetica", "Arial", "Liberation Sans", "DejaVu Sans")
#let rules = (x: 0.3pt + black, y: 0.15pt + black)

// Rosson's column widths: pdfmake's width plus its 4 pt padding on each side.
#let w(x) = x + 8pt
// Driving's judges' sheet has no correction or coefficient columns (Rosson's).
#let driving = data.at("driving", default: false)
#let cc(a, b) = if driving { () } else { (a, b) }
#let standard-columns = (w(10pt), w(260pt), w(18pt), ..cc(w(18pt), w(18pt)), 1fr)

#set document(title: data.title)
#set text(font: fonts, size: 8pt, lang: data.language, top-edge: "ascender", bottom-edge: "descender")
// Figures of equal width, for columns (Inter's are proportional); only figures: its tabular
// hyphen would open gaps in names (Chalet - à - Gobet).
#show regex("[0-9]+"): set text(number-width: "tabular")
#set par(leading: 0pt, spacing: 0pt)
#set block(spacing: 0pt)
#set table(inset: (x: 4pt, y: 2pt), stroke: rules)
#set table.cell(breakable: false)

// `n` lines of the current text, empty: Rosson's "\n\n" in a remarks cell (two lines, as its
// printed sheets show: a test on two pages).
#let lines(n) = box(height: n * 1.17em)
#let head(it) = text(size: 7.5pt, weight: "bold", it)
#let to-fill = table.cell(fill: fill-out)[]

// The sheet a page belongs to: (its page in the sheet, the sheet's pages, the sheet).
#let sheet-page() = {
  let p = here().page()
  let starts = query(<sheet-start>).filter(m => m.location().page() <= p)
  if starts.len() == 0 { return (1, 1, none) }
  let start = starts.last()
  let end = query(<sheet-end>).find(m => m.value.i == start.value.i)
  let first = start.location().page()
  let last = if end == none { first } else { end.location().page() }
  (p - first + 1, last - first + 1, start.value)
}

// A sheet's subtotals carried over to the next page.
#let carry(label, page) = table(
  columns: standard-columns,
  table.cell(colspan: 2, text(size: 7pt, weight: "bold")[#label #page]), to-fill, ..cc(to-fill, []), [],
)

#set page(
  paper: "a4",
  margin: (left: 20pt, right: 20pt, top: 40pt, bottom: 40pt),
  header-ascent: 13pt,
  header: context {
    let (n, _, _) = sheet-page()
    if n > 1 { carry(l.carried_from, n - 1) }
  },
  footer-descent: 0pt,
  footer: context {
    let (n, pages, sheet) = sheet-page()
    if n < pages { carry(l.carried_to, n + 1); v(10pt) }
    set text(size: 7.5pt)
    grid(columns: (1fr, auto), if sheet != none { sheet.title }, [© #data.year Swiss Equestrian])
  },
  // The memo sheet: marked across for the judge.
  background: context {
    let (_, _, sheet) = sheet-page()
    if sheet != none and sheet.memo {
      rotate(-55deg, text(size: 150pt, weight: "bold", fill: luma(88%), tracking: 6pt, l.memo))
    }
  },
)

// The federation's head, the test, who and what is judged.
#let sheet-head(t, s) = {
  block(width: 100%, height: 74pt, {
    set text(size: 9.5pt, fill: navy)
    text(weight: "bold")[SWISS EQUESTRIAN]
    v(3pt)
    [Postfach 726, Papiermühlestrasse 40 H, CH-3000 Bern 22]
    v(3pt)
    text("+41 (0)31 335 43 43, info@swiss-equestrian.ch, swiss-equestrian.ch")
    // Both sides given: with the width alone, the 74pt block's height cut the logo's top and bottom.
    place(top + right, dy: -21pt, image("/assets/swiss-equestrian.png", width: 85pt, height: 85pt))
  })
  if t.fei {
    align(right, image("/assets/fei.png", height: 20pt))
    v(-18pt)
  }
  v(10pt)
  text(size: 9.5pt, weight: "bold", t.title)
  v(2pt)
  grid(columns: (1fr, auto), column-gutter: 8pt, text(size: 7.5pt, t.subtitle), text(size: 7.5pt, t.version))
  v(10pt)
  // Its value's line kept when empty (the memo sheet).
  let field(title, value) = [#text(size: 5pt, title) \ #text(size: 10pt, weight: "bold")[#box(height: 1.17em)#value]]
  table(
    columns: (w(40pt), w(230pt), 1fr),
    stroke: none,
    inset: (left: 0pt, right: 4pt, y: 2pt),
    table.hline(stroke: 0.5pt),
    field(l.start_number, s.number), field(l.rider, s.rider), field(l.horse, s.horse),
    field(l.class_number, s.class), field(l.judge, s.judge), field(l.date_place, s.date_place),
    table.hline(stroke: 0.5pt),
  )
  v(15pt)
}

// One of the test's tables: its title kept with it, the texts before and after it.
#let test-table(t) = {
  block(sticky: true, text(weight: "bold", t.title))
  if t.before != "" {
    v(10pt)
    block(sticky: true, t.before)
  }
  v(12pt)
  let subtotal = text(size: 7.5pt, weight: "bold")[#l.subtotal #t.title]
  if t.layout == "lektionen" or t.layout == "lektionen_ohne_noten" {
    // Each movement: its number, then a line per part (letter, exercise, directive idea), and
    // its mark, correction, coefficient and remarks over all its parts.
    let marks = t.layout == "lektionen"
    let rows = ()
    for r in t.rows {
      let k = calc.max(1, r.items.len())
      let items = if r.items.len() > 0 { r.items } else { ((position: "", text: "", ideas: ""),) }
      rows.push(table.cell(rowspan: k, r.number))
      for (i, it) in items.enumerate() {
        let part(body) = table.cell(inset: (x: 4pt, top: 1pt, bottom: 1.5pt), body)
        rows += (part(it.position), part(it.text), part(it.ideas))
        if i == 0 and marks {
          rows += (
            table.cell(rowspan: k, r.mark), ..cc(table.cell(rowspan: k, r.correction), table.cell(rowspan: k, r.coefficient)),
            table.cell(rowspan: k, if r.remarks == "" { lines(2) } else { r.remarks }),
          )
        } else if i == 0 {
          rows.push(table.cell(rowspan: k, lines(2)))
        }
      }
      rows.push(table.hline(stroke: 0.15pt + black))
    }
    table(
      columns: if marks { (w(10pt), 32pt, 136pt, 100pt, w(18pt), ..cc(w(18pt), w(18pt)), 1fr) } else { (w(10pt), 32pt, 136pt, 151.3pt, 0pt) },
      stroke: (x, y) => (x: 0.3pt + black, top: if y == 0 { 0.15pt + black }),
      table.header(
        head(l.number), table.cell(colspan: 2, head(l.movements)), head(l.ideas),
        ..if marks { (head(l.mark), ..cc(head(l.correction), head(l.coefficient)), head(l.remarks)) } else { ([],) },
      ),
      table.hline(stroke: 0.15pt + black),
      ..rows,
      ..if marks { (table.cell(colspan: 3, subtotal), [], to-fill, ..cc(to-fill, []), [], table.hline(stroke: 0.15pt + black)) },
    )
  } else if t.layout == "strafpunkte" {
    set text(size: 7.5pt)
    table(
      columns: (w(10pt), w(130pt), w(121pt), w(18pt), ..cc(w(18pt), w(18pt)), 1fr),
      ..t.rows.map(r => ([], r.text, r.penalty, r.received, ..cc([], []), r.remarks)).flatten(),
      table.cell(colspan: 3, subtotal), to-fill, ..cc(to-fill, []), [],
    )
  } else {
    table(
      columns: standard-columns,
      table.header(head(l.number), head(l.criteria), head(l.mark), ..cc(head(l.correction), head(l.coefficient)), head(l.remarks)),
      ..t.rows.map(r => (r.number, r.text, r.mark, ..cc(r.correction, r.coefficient), if r.remarks == "" { lines(2) } else { r.remarks })).flatten(),
      table.cell(colspan: 2, subtotal), to-fill, ..cc(to-fill, []), [],
    )
  }
  v(22pt)
  if t.after != "" {
    v(-12pt)
    text(size: 7.5pt, t.after)
    v(12pt)
  }
}

// The result: (kür, children: the A and B marks,) total points, percent; the signature — kept
// together, the signature never alone on a page.
#let result(t) = block(breakable: false, {
  v(-5pt)
  block(sticky: true, text(size: 9.5pt, weight: "bold", l.result))
  v(7pt)
  set text(size: 7.5pt)
  let line(label, value: [], filled: false, remark: []) = (
    table.cell(colspan: 2, text(weight: "bold", label)),
    table.cell(colspan: if driving { 1 } else { 3 }, align: right, fill: if filled { fill-out }, text(weight: "bold", value)),
    remark,
  )
  table(
    columns: standard-columns,
    ..if t.kuer { line(l.total_a) + line(l.total_b) } else if t.children { line(l.total_a_children) + line(l.total_b_children) },
    ..line(l.total_points, filled: true),
    ..line(l.percent, value: [ %], filled: true, remark: [#l.max_points #t.max_points = 100%]),
  )
  v(22pt)
  if t.result_after != "" {
    v(-12pt)
    text(size: 7.5pt, t.result_after)
    v(12pt)
  }
  v(-7pt)
  grid(columns: (40%, 1fr), text(size: 7.5pt, l.signature), [......................................................])
})

#for (i, s) in data.sheets.enumerate() {
  let t = data.tests.at(s.test)
  if i > 0 { pagebreak() }
  [#metadata((i: i, memo: s.memo, title: t.title)) <sheet-start>]
  sheet-head(t, s)
  for x in t.tables { test-table(x) }
  result(t)
  [#metadata((i: i)) <sheet-end>]
}
