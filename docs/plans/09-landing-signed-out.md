# 09 · Landing page, signed out

Feedback 13: *Signed out, the landing page should prioritize the "sign
in" call to action, and allow lookup by AT Protocol id as a secondary
action.*

Small. Independent of the others, but plan 10 changes the lookup field
this page carries, so this goes first.

## The page today

A pitch (`h1` and lede), the lookup form as the only control, a note in
the metadata voice, and an account line at the bottom: "Have an account?
Sign in to write here." Sign-in is the last thing on the page and the
only link in a sentence.

## The page after

Signed out, in order:

1. **Page head.** Kicker none; `h1` the pitch, reworded toward the
   author: "AT where you ate." Lede: "Your reviews of places
   to eat, kept in your own AT Protocol repository and published as a
   small journal of your own."
2. **The one primary action.** An `.actions` row with a primary
   `a.button` "Sign in" leading to `/login`, and beside it, in the sans
   at small size in `ink-muted`, "with your Bluesky or AT Protocol
   account". The page's only accent-filled control (design rule: at
   most one primary per page).
3. **The secondary action.** A hairline, then the lookup form with its
   label reworded as "Or read someone's reviews" and the button as a
   secondary pill ("Read"). Plan 10 gives its input the same handle
   autocomplete as the sign-in page.
4. **The note.** The existing metadata-voice line, trimmed: "Every
   review stays in its author's repository; this site only reads."

The account line goes: signed out, the page has said "Sign in" already.
Signed in, the page is plan 11's.

## Design

- `docs/design.md`, page recipes: Landing `/` becomes "page head: h1
  pitch, lede | primary Sign in with its aside; hairline; lookup form
  (secondary button); note". The paragraph about the account line under
  the lookup form is rewritten by plan 11.
- `app.css`: `.landing-actions` (the row: pill and aside on one
  baseline, wrapping under 26rem), `.landing-divider` (the hairline with
  section spacing), and the lookup form's button as `.button-secondary`
  when the form says so. The lookup form component takes a `primary:
  bool` so the lookup-error page can keep its primary button (it is the
  only control there).

## Touchpoints

- `routes/landing.rs`: the signed-out branch. The signed-in branch is
  left as it is for plan 11 to replace.
- `eaten-at-web/src/components.rs`: `lookup_form` gains the label and
  button-style parameters (a small `LookupForm` struct rather than more
  positional arguments).
- `app.css`, `docs/design.md`.
- `scripts/visual-check.mjs`: `landing` now expects `a.button[href="/login"]`.
- Tests: `routes.rs` landing signed out has exactly one primary button
  and it leads to `/login`; the lookup form is still present and still
  posts to `/lookup`.

## Definition of done

- Signed out, the first control on the page is "Sign in"; the lookup
  form is present, secondary, and works with JavaScript off.
- `just check` and `just visual-check` pass.

## As built (2026-09-13)

As planned. The signed-in branch of the landing page is untouched and
keeps its old copy, lookup form, and account line until plan 11
replaces it. The lookup-error page keeps its primary "Go" button and
its old label through `LookupForm::default()`.

## Decisions

Settled 2026-09-13:

1. **Pitch copy** is as written above. One open question sits behind
   it: "write-up" is not settled as the canonical word for a document,
   and the copy here says "review" in places the rest of the site says
   "write-up". The replacement is not chosen yet. Until it is, each
   plan uses the word it found; when the word is chosen, the copy across
   the site changes in one pass, not plan by plan.
2. **No sample write-ups signed out.** There is no index (D9), so it
   would mean a configured showcase DID; worth its own small plan once
   there is content worth showing.
