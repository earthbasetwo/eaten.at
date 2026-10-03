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

## After Ken's look (2026-09-27)

- The recent rows are headed "Recently" and close on the first
  keystroke; the list sits under the near line, in the flow, so "near
  Acton, MA" stays in view above it.
- Loading (PL31): of three shapes shown as an artifact (rows step back
  with a status row; the near line as the indicator; a sweep along the
  name's rule), Ken chose the first alone, then, since the list now sits
  right under the near line, had the words go into that line instead of
  a row: the combobox marks the list stale when a search starts and,
  400 ms later, the near line reads "looking near Acton, MA…" and a
  live region says the same; the answer clears both.
- The town control, confirmed by Ken: pressing the town closes the
  results and opens the town field empty (a town is retyped, not
  edited); the name stays, and a picked town re-runs the same search
  there, with "looking near…" on the near line while it does. Return in
  the town field never sends the form (it had, and came back as "Name
  the place."); Escape puts the town back.
- Five rows, not ten (PL32, Ken: a lot, and maybe not for a phone),
  then "Show n more", which reveals the rest of the same cached answer
  with no request, then the by-hand row. The server sends the whole
  answer, twenty at most.
- The script tripwire went to 96 KiB (Ken) rather than trim comments a
  third time; T4 settles it.
- "eve" never finds "Eve & Murray's Farm to Home" (Acton, 2 mi), even
  at 3 miles, while "eve & murray" does (PL33). Not the matcher: the
  shop is a `cheese_shop` under Overture's `shopping >
  food_and_beverage_store`, outside `food_and_drink`, which covers
  places to eat and drink (bakeries, breweries and ice cream shops
  included) but not shops that sell food. The API's `mode` (`all`,
  `name`, `address`) changes nothing for a name query, and it takes one
  category per call. A close search without a category, filtered by
  hierarchy, would find it only when the wide answer is full; two
  categories at every step would halve the quota. Ken, 2026-09-27: the
  no-category fallback on an empty answer and the by-hand row are fine
  for v1; typing more of the name gets there.
- A clicked handle suggestion sends its form, as Return does (L6), and
  an over-long address is refused on the choosing page, where the field
  is, with the composer's refusal pointing at "somewhere else" (PC4).

## Verification

`just check`; new route tests for the slug, the category, the recent
list, the take action and the corrected pick; unit tests for the slug
rule and the entity decoding; the flows walked in a headless browser
against the local network. The script tripwire (84 KiB) is six bytes
from the line after trimming comments: T4 is due.
