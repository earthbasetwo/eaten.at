/* Handle suggestions from the Bluesky AppView (plan 10), called without
   credentials. A picked suggestion sends the form, by Enter or by
   pointer (L6); Enter with none picked sends exactly what was typed. */
(function () {
  "use strict";
  var inputs = document.querySelectorAll("input[data-typeahead]");
  Array.prototype.forEach.call(inputs, function (input) {
    var origin = input.getAttribute("data-typeahead");
    window.eaCombobox(input, {
      minChars: 3,
      delay: 350,
      source: function (q, signal) {
        var url = origin + "/xrpc/app.bsky.actor.searchActorsTypeahead?limit=5&q=" + encodeURIComponent(q);
        return fetch(url, { credentials: "omit", signal: signal })
          .then(function (r) { return r.ok ? r.json() : { actors: [] }; })
          .then(function (body) { return body.actors || []; });
      },
      render: function (actor) {
        return { label: actor.displayName || actor.handle, detail: "@" + actor.handle };
      },
      pick: function (actor) { input.value = actor.handle; },
      send: true
    });
  });
})();
