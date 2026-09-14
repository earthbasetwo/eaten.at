/* Live find on the author's home (plan 11): after a pause in typing,
   the same GET the form would make is fetched and its results section
   swapped in, with the address bar kept in step so the find stays
   shareable. The form still submits as a form; this only saves the
   reload. */
(function () {
  "use strict";
  var form = document.querySelector("form.find");
  var section = form && form.closest(".own-publication");
  if (!form || !section || !window.fetch || !window.DOMParser) return;
  var input = form.elements.q;
  var timer = null, pending = null, shown = input.value.trim();

  function url(q) {
    return q ? "/?q=" + encodeURIComponent(q) : "/";
  }
  function swap(html, q) {
    var fresh = new DOMParser().parseFromString(html, "text/html").querySelector(".find-results");
    var mine = section.querySelector(".find-results");
    if (!fresh || !mine) return;
    mine.replaceWith(fresh);
    shown = q;
    history.replaceState(null, "", url(q));
  }
  function find() {
    var q = input.value.trim();
    if (q === shown || (q.length === 1)) return;
    if (pending) pending.abort();
    var ctrl = new AbortController();
    pending = ctrl;
    fetch(url(q), { signal: ctrl.signal, headers: { Accept: "text/html" } })
      .then(function (r) { return r.ok ? r.text() : Promise.reject(r.status); })
      .then(function (html) { if (pending === ctrl) swap(html, q); })
      .catch(function () {});
  }

  input.addEventListener("input", function () {
    clearTimeout(timer);
    timer = setTimeout(find, 500);
  });
})();
