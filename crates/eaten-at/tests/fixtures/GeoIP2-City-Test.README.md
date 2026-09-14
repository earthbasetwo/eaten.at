# GeoIP2-City-Test.mmdb

MaxMind's own small test database, from
https://github.com/maxmind/MaxMind-DB/tree/main/test-data (MIT or
Apache-2.0, at your option). The same file format and GeoIP2 City
layout production reads (DB-IP's IP-to-City Lite), so the IP locator
can be tested without the production file. Known addresses:
`81.2.69.160` is London (51.5142, -0.0931); `2.125.160.216` is Boxford
(51.75, -1.25). See `source-data/GeoIP2-City-Test.json` there for the
rest.
