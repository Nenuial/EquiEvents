# Ranks set by hand

The show office can set a rider's rank by hand, for a decision the scoring rules do not cover
(a jury decision, or the rank the timekeeper's software sends when the office chooses to take it
over). The `manual` column holds the rank set by hand; `rank` is the result.

The rule (docs/deviations.md S18): the rider gets the rank set by hand; everyone else keeps their
order and their ties, and their ranks skip the numbers set by hand. Riders who are out stay out.
Rosson did the same, but in three slightly different ways depending on the discipline.

## The others skip the rank set by hand

table: 14

Dandy is 4th on the result; the office ranks Dandy 2nd. Bijou and Cassis stay equal, now 3rd.

| horse  | strafpunkte_initial_1 | zeit_initial_1 | manual | rank |
|--------|----:|------:|---:|----:|
| Amigo  | 0   | 61.08 |    | 1   |
| Dandy  | 8   | 58.00 | 2  | 2   |
| Bijou  | 4   | 59.90 |    | 3   |
| Cassis | 4   | 59.90 |    | 3   |
| Elan   | 12  | 70.00 |    | 5   |

## A winner ranked lower by hand

table: 14

Amigo has the best result but is ranked 3rd by the jury. Bijou and Cassis move up.

| horse  | strafpunkte_initial_1 | zeit_initial_1 | manual | rank |
|--------|----:|------:|---:|----:|
| Bijou  | 4   | 59.90 |    | 1   |
| Cassis | 4   | 60.50 |    | 2   |
| Amigo  | 0   | 61.08 | 3  | 3   |
| Dandy  | 8   | 58.00 |    | 4   |

## A rank set by hand for a rider who is out

table: 14

Elan was eliminated; the jury reinstates Elan at rank 2. The riders who are still out stay out.

| horse  | strafpunkte_initial_1 | zeit_initial_1 | out        | manual | rank |
|--------|----:|------:|------------|---:|----:|
| Amigo  | 0   | 61.08 |            |    | 1   |
| Elan   |     |       | eliminated | 2  | 2   |
| Bijou  | 4   | 59.90 |            |    | 3   |
| Fanny  |     |       | retired    |    | out |

## Two riders set to the same rank

table: 14

The office may set a tie by hand; it takes two places, as any tie: the next rank is 3.

| horse  | strafpunkte_initial_1 | zeit_initial_1 | manual | rank |
|--------|----:|------:|---:|----:|
| Bijou  | 4   | 59.90 | 1  | 1   |
| Cassis | 4   | 60.50 | 1  | 1   |
| Amigo  | 0   | 61.08 |    | 3   |
| Dandy  | 8   | 58.00 |    | 4   |

## Dressage

programme: max 30; 1x1, 2x2

Same rule in every discipline. Cassis has the lowest percentage and is ranked 1st by hand.

| horse  | C.1 | C.2 | manual | percentage | rank |
|--------|----:|----:|---:|------:|----:|
| Cassis | 5   | 5   | 1  | 50.00 | 1   |
| Amigo  | 8   | 8   |    | 80.00 | 2   |
| Bijou  | 7   | 7   |    | 70.00 | 3   |
