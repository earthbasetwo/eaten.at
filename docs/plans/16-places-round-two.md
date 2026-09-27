# 16 · Places, round two

Working branch: `places`. Ken's sort of the open Places rows on
2026-09-27 (Backlog: Places), after the near line (plan 15): the rows
that could be built without a design conversation, with his answers on
the four that needed one. PL8 (a place page), PL9 (fix the place once),
PL14 (the feed by place) and the rest stay on the backlog.

## The record (before the lexicons publish, DP5)

`at.eaten.place` gains two optional strings (D49, D50):

- `slug`: the place's short name for the author's own URLs, Ken's
  `{feed host}/{slug}/on/{date}`. Settled at publish from the name
  (lowercase, accents folded, apostrophes closed up, else hyphens) and
  the author's other visits, in one scan of the repo with the document
  paths: the same place (same GERS id, or the same name by hand) keeps
  the slug it has; a name another of the author's places holds takes
  the nearest town ("katzs-delicatessen-brooklyn"), then a number. An
  edit keeps the record's slug unless the place changed. Nothing is
  stored on the site: the repo is the record. Routing the URL is DP3.
- `category`: Overture's own value at pick time (`coffee_shop`), a
  hidden field a pick fills and the record carries. Shown nowhere yet;
  a place by hand has none.

## The search

- Results are cached for a day (PL10; an hour before). Overture's data
  changes monthly and the data-rights terms allow it; the free tier is
  10,000 calls a month. The place table PL10 also named has no reader
  yet and is not built: picks re-read the day-long cache.
- The quota header is logged at info on every answer, not only on a
  refusal (PL12).
- Names and address parts come with HTML entities as Overture stores
  them ("Katz&amp Nelson"); the five named entities, with or without the
  semicolon, and numeric ones are decoded (PL23).

## The chooser

- Recent places (PL11): an empty name field, on focus, lists up to five
  of the author's own places, newest visit first, one row per place,
  from `/write/recent`, which reads their own visits and costs no
  quota. A row carries everything a pick would fill; taking it fills
  the hidden id, position and category fields and submits the new
  `take` action: a pick when it has an id, a place by hand when it has
  none (Home). The combobox grew an `empty` hook for it.
- A corrected name keeps the pick (PL29, Ken): the same name once
  folded, a start or a tail of it, or a typo or two away, keeps the id
  and the address, and the corrected name rides with the pick; the
  server puts the posted name over the listing's. Another name drops
  the pick and its address as before; "Home" always does.

## Verification

`just check`; new route tests for the slug, the category, the recent
list, the take action and the corrected pick; unit tests for the slug
rule and the entity decoding; the flows walked in a headless browser
against the local network. The script tripwire (84 KiB) is six bytes
from the line after trimming comments: T4 is due.
