# Feedback

Collected feedback, to be turned into a plan in a later session. Items are recorded as given.

## New visit creation flow

1. The "Write" / "A new write-up" text at the top of the page is useless. Remove it.
2. Titles should be optional. The name of the place visited is the most important thing and should be the default title, unless the user chooses to give the visit its own title.
3. Start the flow by having the user search for a place and pick it from a list of results, then continue with the write-up. This replaces the current "add ID" flow (see "Place identity" below).

## Place identity

4. Remove the concept of multiple external IDs. For now, use only Overture Places, identified by GERS ID.
5. Store the place name and address that Overture returns directly in the app's own model.
6. Use [Open Places API](https://openplacesapi.com/), which serves Overture data, for place search and lookup.

## Visit and place fields

7. Remove the "Other" meal option.
8. Limit link types to "Official site" and "Other". Today the `at.eaten.place` link `service` known values are `officialSite`, `menu`, and `reservations`.
9. Remove "Cover image". It doesn't fit a restaurant visit.

## Photos

10. Build an "Add photos" flow that lets a user attach as many photos as they want to a visit. This replaces the cover image.

## Publications

11. Make "New publication" its own separate flow. It should not be built into the "New write-up" flow.

## Second pass, 2026-09-13 (after walking through the UI)

Planned in `docs/plans/08-*` through `13-*`.

### Publications

12. Remove the ability to have multiple publications for v1. Every user
    using eaten.at should have a single designated eaten.at publication.

### Landing page

13. Signed out, the landing page should prioritize the "sign in" call to
    action, and allow lookup by AT Protocol id as a secondary action.
14. Signed in, the landing page should prioritize writing a new visit as
    the main action.
15. Signed in, the landing page should offer the ability to view and
    query your own publication as secondary actions: show the recent n
    reviews in a list.
16. The settings page should be tertiary priority.

### Sign in

17. The sign-in flow should autocomplete the AT Protocol handle with
    `app.bsky.actor.searchActorsTypeahead` on
    `https://public.api.bsky.app/xrpc/…?limit=5&q=…`: no auth,
    `credentials: 'omit'`.
18. The UI should indicate "start typing" or similar, so the user knows
    they won't have to enter their entire handle by hand.

### New visit

19. The new-visit UI should begin with selecting a place. Use
    [Open Places API](https://openplacesapi.com/) for autocomplete of
    place names, with a reverse-geo lookup of the IP for the location
    sent to the API.
20. Selecting from the API's list is not required: the user may enter a
    name (required) and an address (optional) manually.
21. After selecting a place, the user should be taken to the main
    write/edit UI with that place's info pre-filled, but still editable.

### Publication view

22. The publication view's cards (the list of documents in a
    publication) should highlight the images included in a visit, if
    any, and look elegant whether or not there are images.
