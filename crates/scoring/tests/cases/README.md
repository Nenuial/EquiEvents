# Test cases

Hand-written cases for the scoring rules, meant to be read and extended by people who know the
regulations. `cargo test -p scoring --test cases` runs every case in this folder.

## How a case is written

```markdown
## A short name saying what the case shows

table: 14

| horse  | strafpunkte_initial_1 | zeit_initial_1 | out | rank |
|--------|----:|------:|----:|----:|
| Amigo  | 0   | 61.08 |     | 1   |
| Bijou  | 4   | 59.90 |     | 2   |
| Cassis |     |       | 21  | out |
```

- One `##` heading per case. Text around the table is free: explain the rule.
- **The rows are the expected ranking, in the order it will be printed.** The test checks the
  order as well as the ranks.
- `rank` is the expected rank, or `out` for eliminated / retired.
- `why` (optional): why the rider stands there, in the engine's words — the values the ranking
  compared, in order (`scoring::jumping::Why::text`; dressage: `scoring::ranking::reasons_text`).
  Checked word for word: it shows a reader what each rank rests on.

### Jumping

- `table:` the Swiss Equestrian scoring table number (`tests/fixtures/scoring-tables.json`).
- One column per field of the table, named as in the table definition
  (`strafpunkte_initial_1`, `zeit_stechen_1`, ...). Empty cell = not ridden / not entered.
- `out`: `eliminated`, `retired` or `withdrawn` (Swiss Equestrian codes 21, 22, 23), or another reason code.
- Optional expectations: `faults` (or points), `time` of the deciding block; `placeable`
  (`yes` / `no`): whether the rider can be placed at all (a reduced second round, S34).
- `ideal:` the class's ideal time in seconds (ideal-time table 35).
- `options:` the class's options, as words: `medal jump-offs` (S14), `placed 4` (the placed
  count the organiser set), `every result counts` (teams, S36).

### Dressage

- `programme:` the maximum points per judge and the coefficient of each movement, e.g.
  `programme: max 30; 1x1, 2x2` (movement 1 with coefficient 1, movement 2 with coefficient 2).
- `judges:` how many judges the class has (default: as many as appear in the columns).
- One column per judge and movement: `C.1`, `C.2`, `M.1`, ... A judge's point penalties go in
  `C.penalty`.
- `out`: anything in this column means eliminated / retired.
- Optional expectation: `percentage`.
