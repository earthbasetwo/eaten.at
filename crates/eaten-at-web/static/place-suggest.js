/* Place suggestions in the editor (plan 12): the search box becomes a
   combobox whose source is this site's own suggest endpoint, which
   searches Open Places near where the request is from. Picking a
   suggestion submits the form as the numbered pick of the same search,
   so the server reads the place from its cache. Without this, the
   Search button and the result cards do the same job. */
(function () {
  "use strict";
  var input = document.querySelector("input[data-suggest]");
  if (!input || !input.form || !window.eaCombobox) return;
  var form = input.form;
  var status = form.querySelector(".locate-status");
  var url = input.getAttribute("data-suggest");
  var searched = "";
  form.classList.add("js-suggesting");
  window.eaCombobox(input, {
    minChars: 3,
    delay: 300,
    source: function (q, signal) {
      return fetch(url + "?q=" + encodeURIComponent(q), {
        signal: signal,
        credentials: "same-origin",
        headers: { Accept: "application/json" }
      })
        .then(function (r) { return r.ok ? r.json() : { hits: [], error: "unavailable" }; })
        .then(function (body) {
          if (body.error === "unavailable" && status) {
            status.textContent = "Search isn't answering; enter the place below.";
          }
          searched = body.q || q;
          return body.hits || [];
        });
    },
    render: function (hit) { return { label: hit.name, detail: hit.detail }; },
    pick: function (hit) {
      input.value = searched;
      var action = document.createElement("input");
      action.type = "hidden";
      action.name = "action";
      action.value = "pick:" + hit.i;
      form.appendChild(action);
      form.submit();
    }
  });
})();
