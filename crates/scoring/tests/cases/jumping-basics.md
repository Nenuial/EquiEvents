# Show jumping: the basics

## Fewer faults first, then the faster time

table: 14

Table 14, "Wertung A mit Zeitmessung": one round, faults then time. Eliminated and retired
riders are out and listed last.

| horse  | strafpunkte_initial_1 | zeit_initial_1 | out | rank | why |
|--------|----:|------:|----:|----:|---|
| Cassis | 0   | 61.08 |     | 1   | round 1: 0 faults, 61.08 s |
| Amigo  | 0   | 62.50 |     | 2   | round 1: 0 faults, 62.50 s |
| Bijou  | 4   | 58.10 |     | 3   | round 1: 4 faults, 58.10 s |
| Dandy  | 8   | 55.00 |     | 4   | round 1: 8 faults, 55.00 s |
| Elan   |     |       | eliminated | out | out: eliminated (reason 21) |

## Same faults and same time: same rank, listed by horse name

table: 14

Tied riders share the rank and are listed alphabetically by horse name (S4); the next rank
skips (2, 2, 4).

| horse  | strafpunkte_initial_1 | zeit_initial_1 | rank | why |
|--------|----:|------:|----:|---|
| Zorro  | 0   | 60.00 | 1   | round 1: 0 faults, 60.00 s |
| Amigo  | 4   | 61.00 | 2   | round 1: 4 faults, 61.00 s |
| Bijou  | 4   | 61.00 | 2   | round 1: 4 faults, 61.00 s |
| Cassis | 4   | 62.00 | 4   | round 1: 4 faults, 62.00 s |

## Not timed: equal faults are tied

table: 7

Table 7, "Wertung A ohne Zeitmessung": only faults count.

| horse  | strafpunkte_initial_1 | rank | why |
|--------|----:|----:|---|
| Amigo  | 0   | 1   | round 1: 0 faults |
| Bijou  | 0   | 1   | round 1: 0 faults |
| Cassis | 4   | 3   | round 1: 4 faults |

## Out riders are grouped by reason, then listed by horse name

table: 14

Riders who are out are listed after everyone ranked: by reason code (as Rosson did: eliminated 21,
retired 22, withdrawn 23), and within the same reason by horse name (S4).

| horse  | strafpunkte_initial_1 | zeit_initial_1 | out | rank | why |
|--------|----:|------:|----:|----:|---|
| Amigo  | 0   | 61.00 |     | 1   | round 1: 0 faults, 61.00 s |
| Dandy  |     |       | eliminated | out | out: eliminated (reason 21) |
| Elan   |     |       | eliminated | out | out: eliminated (reason 21) |
| Bijou  |     |       | retired | out | out: retired (reason 22) |

## Times are cut at the hundredth

table: 14

Times count to the hundredth (Springreglement 7.9): a timing file with thousandths is cut, not
rounded (P. Burkhard, 2026-11-17). 61.239 and 61.231 are both 61.23: Amigo and Bijou are equal.

| horse  | strafpunkte_initial_1 | zeit_initial_1 | rank | time | why |
|--------|----:|------:|----:|------:|---|
| Amigo  | 0   | 61.239 | 1  | 61.23 | round 1: 0 faults, 61.23 s |
| Bijou  | 0   | 61.231 | 1  | 61.23 | round 1: 0 faults, 61.23 s |
| Cassis | 0   | 61.25  | 3  | 61.25 | round 1: 0 faults, 61.25 s |
