// Labels on a sheet of sticky labels (Zweckform 3475: 3 × 8 of 70 × 36 mm; 3484: 2 × 8 of
// 105 × 37 mm; or a sheet the office describes: margins, gaps, alignment): each label a few
// lines, the first `skip` places left empty (a sheet already partly used). No header, no
// footer: the page is the sheet.

#import "/common.typ": data, fonts, ink, muted

#let s = data.sheet
#set page(paper: "a4", margin: 0pt)
#set text(font: fonts, size: s.font_pt * 1pt, lang: data.language, fill: ink)
// Figures of equal width, for columns (Inter's are proportional); only figures: its tabular
// hyphen would open gaps in names (Chalet - à - Gobet).
#show regex("[0-9]+"): set text(number-width: "tabular")
#set par(leading: 0.35em)

#let per-page = s.cols * s.rows
#let places = range(data.skip).map(_ => none) + data.labels
#for (p, chunk) in places.chunks(per-page).enumerate() {
  if p > 0 { pagebreak() }
  for (i, label) in chunk.enumerate() {
    if label == none { continue }
    let col = calc.rem(i, s.cols)
    let row = calc.quo(i, s.cols)
    let gap-x = s.at("gap_x_mm", default: 0)
    let gap-y = s.at("gap_y_mm", default: 0)
    let align-to = (left: left, center: center, right: right).at(s.at("align", default: "left"))
    place(
      top + left,
      dx: (s.left_mm + col * (s.width_mm + gap-x)) * 1mm,
      dy: (s.top_mm + row * (s.height_mm + gap-y)) * 1mm,
      box(width: s.width_mm * 1mm, height: s.height_mm * 1mm, inset: s.padding_mm * 1mm, clip: true, align(align-to, {
        for (j, line) in label.lines.enumerate() {
          if j > 0 { linebreak() }
          text(
            weight: if line.at("bold", default: false) { "bold" } else { "regular" },
            size: line.at("size", default: s.font_pt) * 1pt,
            fill: if line.at("muted", default: false) { muted } else { ink },
            line.text,
          )
        }
      })),
    )
  }
}
