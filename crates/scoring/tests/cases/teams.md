# Team classes (S7)

Rows are riders, grouped by team, teams in expected ranking order. `rank` is the team's rank;
`team faults` and `team time` are the team's totals in its deciding block. Rules given
2026-11-13 (docs/deviations.md S7).

## Nations Cup formula: the worst result of each round is dropped

table: 26

Table 26, two rounds, faults and time of both rounds count. Teams of 4 drop their worst result
in each round; a team of 3 drops nothing.

- Gelb: round 1, Y4 is worst (4 faults) and dropped. Round 2, Y4 was eliminated: that is the
  dropped result. 0 faults, 213 + 210 = 423 s.
- Rot: round 1 drops R4 (8 faults), round 2 drops R1 (4 faults): 4 faults, 213 + 206 = 419 s.
- Blau (3 riders): everything counts: 4 faults, 213 + 207 = 420 s.

| team | horse | strafpunkte_initial_1 | zeit_initial_1 | strafpunkte_initial_2 | zeit_initial_2 | out | rank | team faults | team time |
|------|-------|----:|------:|----:|------:|----:|----:|----:|-------:|
| Gelb | Y1    | 0   | 70.00 | 0   | 69.00 |     | 1   | 0   | 423.00 |
| Gelb | Y2    | 0   | 71.00 | 0   | 70.00 |     | 1   |     |        |
| Gelb | Y3    | 0   | 72.00 | 0   | 71.00 |     | 1   |     |        |
| Gelb | Y4    | 4   | 73.00 |     |       | eliminated | 1   |     |        |
| Rot  | R1    | 0   | 70.00 | 4   | 68.00 |     | 2   | 4   | 419.00 |
| Rot  | R2    | 4   | 72.00 | 0   | 69.00 |     | 2   |     |        |
| Rot  | R3    | 0   | 71.00 | 0   | 70.00 |     | 2   |     |        |
| Rot  | R4    | 8   | 69.00 | 0   | 67.00 |     | 2   |     |        |
| Blau | B1    | 0   | 70.00 | 0   | 68.00 |     | 3   | 4   | 420.00 |
| Blau | B2    | 0   | 71.00 | 4   | 69.00 |     | 3   |     |        |
| Blau | B3    | 0   | 72.00 | 0   | 70.00 |     | 3   |     |        |

## Every result counts: nothing dropped

table: 26
options: every result counts

The same results, in a class whose Ausschreibung counts every rider (class option; Springreglement
4.6; P. Burkhard, 2026-11-17). Nothing is dropped: Rot has 0 + 4 + 0 + 8 = 12 faults in round 1
and 4 in round 2, 16 faults in 282 + 274 = 556 s, behind Blau. Gelb, with Y4 eliminated, has no
result left to drop: out.

| team | horse | strafpunkte_initial_1 | zeit_initial_1 | strafpunkte_initial_2 | zeit_initial_2 | out | rank | team faults | team time |
|------|-------|----:|------:|----:|------:|----:|----:|----:|-------:|
| Blau | B1    | 0   | 70.00 | 0   | 68.00 |     | 1   | 4   | 420.00 |
| Blau | B2    | 0   | 71.00 | 4   | 69.00 |     | 1   |     |        |
| Blau | B3    | 0   | 72.00 | 0   | 70.00 |     | 1   |     |        |
| Rot  | R1    | 0   | 70.00 | 4   | 68.00 |     | 2   | 16  | 556.00 |
| Rot  | R2    | 4   | 72.00 | 0   | 69.00 |     | 2   |     |        |
| Rot  | R3    | 0   | 71.00 | 0   | 70.00 |     | 2   |     |        |
| Rot  | R4    | 8   | 69.00 | 0   | 67.00 |     | 2   |     |        |
| Gelb | Y1    | 0   | 70.00 | 0   | 69.00 |     | out |     |        |
| Gelb | Y2    | 0   | 71.00 | 0   | 70.00 |     | out |     |        |
| Gelb | Y3    | 0   | 72.00 | 0   | 71.00 |     | out |     |        |
| Gelb | Y4    | 4   | 73.00 |     |       | eliminated | out |     |        |

## Jump-off with one rider per team

table: 17

Rot and Blau are equal first after dropping their worst round-1 result; one rider of each jumps
off and that result is the team's.

| team | horse | strafpunkte_initial_1 | zeit_initial_1 | strafpunkte_stechen_1 | zeit_stechen_1 | rank | team faults | team time |
|------|-------|----:|------:|----:|------:|----:|----:|-------:|
| Rot  | R1    | 0   | 60.00 | 0   | 35.00 | 1   | 0   | 35.00  |
| Rot  | R2    | 0   | 61.00 |     |       | 1   |     |        |
| Rot  | R3    | 0   | 62.00 |     |       | 1   |     |        |
| Rot  | R4    | 4   | 59.00 |     |       | 1   |     |        |
| Blau | B1    | 0   | 61.00 |     |       | 2   | 4   | 33.00  |
| Blau | B2    | 0   | 60.00 | 4   | 33.00 | 2   |     |        |
| Blau | B3    | 4   | 58.00 |     |       | 2   |     |        |
| Blau | B4    | 0   | 63.00 |     |       | 2   |     |        |
| Gelb | G1    | 4   | 60.00 |     |       | 3   | 8   | 183.00 |
| Gelb | G2    | 4   | 61.00 |     |       | 3   |     |        |
| Gelb | G3    | 0   | 62.00 |     |       | 3   |     |        |
| Gelb | G4    | 8   | 59.00 |     |       | 3   |     |        |

## All four riders jump off: the worst is dropped too

table: 17

| team | horse | strafpunkte_initial_1 | zeit_initial_1 | strafpunkte_stechen_1 | zeit_stechen_1 | rank | team faults | team time |
|------|-------|----:|------:|----:|------:|----:|----:|-------:|
| Blau | B1    | 0   | 60.00 | 0   | 34.00 | 1   | 0   | 105.00 |
| Blau | B2    | 0   | 61.00 | 0   | 35.00 | 1   |     |        |
| Blau | B3    | 0   | 62.00 | 0   | 36.00 | 1   |     |        |
| Blau | B4    | 0   | 64.00 | 8   | 30.00 | 1   |     |        |
| Rot  | R1    | 0   | 60.00 | 0   | 35.00 | 2   | 0   | 108.00 |
| Rot  | R2    | 0   | 61.00 | 0   | 36.00 | 2   |     |        |
| Rot  | R3    | 0   | 62.00 | 4   | 34.00 | 2   |     |        |
| Rot  | R4    | 0   | 63.00 | 0   | 37.00 | 2   |     |        |

## Pairs: both results are added

table: 14

| team   | horse | strafpunkte_initial_1 | zeit_initial_1 | out | rank | team faults | team time |
|--------|-------|----:|------:|----:|----:|----:|-------:|
| Pair B | B1    | 0   | 61.00 |     | 1   | 0   | 123.00 |
| Pair B | B2    | 0   | 62.00 |     | 1   |     |        |
| Pair A | A1    | 0   | 60.00 |     | 2   | 4   | 118.00 |
| Pair A | A2    | 4   | 58.00 |     | 2   |     |        |
| Pair C | C1    |     |       | eliminated | out |     |        |
| Pair C | C2    |     |       | eliminated | out |     |        |

## A team is out with 2 riders out of 4, or 1 out of 3

table: 14

Gelb (4 riders) loses one rider: that is its dropped result. Rot (4 riders) loses two: out.
Blau (3 riders) loses one: out (S7, decided 2026-11-13). Teams that are out are listed by
reason: eliminated (Rot) before retired (Blau).

| team | horse | strafpunkte_initial_1 | zeit_initial_1 | out | rank | team faults | team time |
|------|-------|----:|------:|----:|----:|----:|-------:|
| Gelb | Y1    | 0   | 60.00 |     | 1   | 4   | 181.00 |
| Gelb | Y2    | 0   | 61.00 |     | 1   |     |        |
| Gelb | Y3    | 4   | 60.00 |     | 1   |     |        |
| Gelb | Y4    |     |       | eliminated | 1 |  |        |
| Rot  | R1    | 0   | 59.00 |     | out |     |        |
| Rot  | R2    | 0   | 60.00 |     | out |     |        |
| Rot  | R3    |     |       | eliminated | out |  |        |
| Rot  | R4    |     |       | eliminated | out |  |        |
| Blau | B1    | 0   | 60.00 |     | out |     |        |
| Blau | B2    | 0   | 61.00 |     | out |     |        |
| Blau | B3    |     |       | retired | out |  |        |
