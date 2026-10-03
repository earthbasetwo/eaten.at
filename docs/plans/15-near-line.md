# 15 · The near line

Working branch: `places`. From Ken's first walkthrough of the chooser
against real places on 2026-09-27 (Backlog: Places, PL3, PL4, PL6). The
API searches near a point and cannot geocode, and 50 miles is its cap,
so reviewing a Brooklyn restaurant from Acton needs a second point of
our own. Ken's shape: the line under the search field is always
"where". Before a pick it says where the search is looking, "Near
Acton, MA · change"; after a pick it is the place's address, as a fact.

## Cities

A bundled GeoNames list (`cities5000.txt`, towns above 5,000 people,
about 70,000 rows, CC BY 4.0) with `admin1CodesASCII.txt` beside it for
region names, held in memory like the DB-IP file: `EATEN_AT_CITIES`
names the cities file, `just cities-refresh` fetches both, the about
page credits GeoNames. A city is labelled "Acton, MA" in the US, Canada
and Australia (the region code), "London, England" elsewhere (the
region's name), no country (Ken). Nearest city to a point; prefix search
on the name ranked by distance from the current near point, else by
population, six rows.

## The near state

`near` is a GeoNames id, a hidden field on the chooser. It is set on
arrival to the city nearest the located point (IP, else the last
visit); when nothing locates, it is empty and the line reads "Near
where? · choose" and the search waits. The suggest endpoint and a pick
both resolve the same `near` to the same point, so a pick reads the
row the browser showed. A place by hand takes the near city's centroid
as its position (PL6: a coarse pin, never the IP point or the address).

## The line

`p.near-line`: "Near " + the label + " · change". Change opens a city
field (a `<details>`, so it works without script; with script the field
is a combobox over `/write/near?q=`). Picking a city sets `near` and the
label and closes the field; without script the typed `near_query` is
resolved on the next submit. With script, the address line is hidden
while the near line shows and shown once a place is picked or typed by
hand; without script both lines show. The address field in by-hand mode
says under it that the address is public.

## Out of scope

The town typed in the search (PL27); a geocoder (PL20); a place record
(PL15).
