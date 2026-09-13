# 01 · Editor page head

Feedback 1: *The "Write" / "A new write-up" text at the top of the page is
useless. Remove it.*

## Change

`crates/eaten-at/src/editor/view.rs`, `page()`: the `div.page-head` opens
with a `p.kicker` ("Write" or "Edit") and an `h1` ("A new write-up" or
"Edit this write-up"). Remove the kicker and the `h1` for a new write-up.
Keep the `div.page-head` itself: it also carries the form-error summary
("One thing to fix below.") and the publish error, which stay.

For an existing write-up, replace the boilerplate `h1` with the
write-up's title, which is at least information. Plan 02 makes that the
place's name when the author gave no title, and plan 06 gives the new
page a real heading ("Where did you eat?" while choosing, then the
chosen place's name), so the page is without an `h1` only until then.

## Touchpoints

- `editor/view.rs`: `page()`.
- `routes/write.rs`: `render()` still sets the browser title from
  `editing.is_some()`; unchanged.
- `docs/design.md`: the "Editor `/write`" row of the page table says
  "page head: 'Write' or 'Edit' kicker, h1". Update it, and add a line
  under **Page head** noting the editor is the one page without a
  heading until a place is chosen.
- Tests: grep `crates/eaten-at/tests/routes.rs` for "A new write-up" and
  "Edit this write-up" and adjust any assertion.

## Definition of done

- `/write` renders no kicker and no `h1`; error summaries still render.
- `/write/{rkey}` renders the document's title as `h1`.
- `just check` and `just visual-check` pass.

## Decisions

1. **Keep an `h1` on the edit page?** Recommended: yes, the write-up's
   title. Every other page opens with a heading (design principle 5) and
   it costs nothing.
