# Ornaments

The vector ornament library (rule, fade, filigree, segment) and an author's own path data in the same slots.

- Level 1: divider motif, heading flanks, dividers in `ornament`.
- Level 2: the set's surface decoration: filigree corner pieces (`.m-ornamented`, with `.m-pad` or `--orn-pad` so they never touch text), or the segment mark on the top edge of framed surfaces (`.m-marked`).
- The drop cap is a separate switch.

Ornaments are single-ink masks in `ornament`, never inside controls or text runs, `aria-hidden`, and removed by "decoration off". Custom artwork must pass `Motet.validateOrnament()`.
