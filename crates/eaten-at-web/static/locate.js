/* The location island (plan 06). Place search needs a point and only the
   browser has one: fill the hidden near_lat/near_lon fields from
   navigator.geolocation on the choosing state. Refused or unavailable,
   the form still submits and the server searches near the last visit. */
(function () {
  "use strict";
  var form = document.querySelector("form.editor[data-locate]");
  if (!form || !navigator.geolocation) return;
  var lat = form.elements.near_lat, lon = form.elements.near_lon;
  var status = form.querySelector(".locate-status");
  if (!lat || !lon || lat.value) return;
  var say = function (text) { if (status) status.textContent = text; };
  say("Finding where you are…");
  navigator.geolocation.getCurrentPosition(function (position) {
    lat.value = position.coords.latitude.toFixed(5);
    lon.value = position.coords.longitude.toFixed(5);
    say("Searching near you.");
  }, function () {
    say("Location unavailable; searching near your last visit.");
  }, { timeout: 8000, maximumAge: 600000 });
})();
