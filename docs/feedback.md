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
