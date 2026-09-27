/* The author's home. Live find (plan 11): after a pause, the form's own
   GET is fetched and its results swapped in, the address kept in step.
   The form still works as a form. The field carries the find's state
   (S19): its clear mark shows while it holds text and empties it in
   place, and the tag line is away while a find is on. And the tag
   line folds to its first line, the rest behind one "+n more" (S4);
   without script every tag shows. */
(function () {
  "use strict";
  var form = document.querySelector("form.find");
  var section = form && form.closest(".own-publication");
  if (!form || !section || !window.fetch || !window.DOMParser) return;
  var input = form.elements.q;
  var clear = form.querySelector(".find-clear");
  var tags = section.querySelector(".own-tags");
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
    /* The tag line is the feed's, not the find's: away while one is on (S19). */
    if (tags) tags.hidden = !!q;
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
    if (clear) clear.hidden = !input.value;
    clearTimeout(timer);
    timer = setTimeout(find, 500);
  });
  /* The clear mark empties the field in place and brings the list back. */
  if (clear) clear.addEventListener("click", function (e) {
    if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    input.value = "";
    clear.hidden = true;
    clearTimeout(timer);
    find();
    input.focus();
  });

  var list = section.querySelector(".tag-list");
  var unfolded = false;
  function fold() {
    if (!list || unfolded) return;
    var old = list.querySelector(".tag-more");
    if (old) old.remove();
    var items = Array.prototype.slice.call(list.children);
    items.forEach(function (li) { li.hidden = false; });
    if (items.length < 2) return;
    var top = items[0].offsetTop;
    var first = items.filter(function (li) { return li.offsetTop === top; }).length;
    if (first === items.length) return;
    var more = document.createElement("li");
    more.className = "tag-more";
    var button = document.createElement("button");
    button.type = "button";
    /* Set like the words it stands in for: a chip among chips, a word in a line. */
    var word = list.querySelector("a");
    button.className = word ? word.className : "tag";
    button.addEventListener("click", function () {
      unfolded = true;
      items.forEach(function (li) { li.hidden = false; });
      var next = items[keep] && items[keep].querySelector("a");
      more.remove();
      if (next) next.focus();
    });
    more.appendChild(button);
    list.appendChild(more);
    /* Hide from the end until the chip itself sits on the first line. */
    var keep = first;
    while (keep > 0) {
      items.forEach(function (li, i) { li.hidden = i >= keep; });
      button.textContent = "+" + (items.length - keep) + " more";
      if (more.offsetTop === top) break;
      keep--;
    }
  }
  var refold = null;
  window.addEventListener("resize", function () {
    clearTimeout(refold);
    refold = setTimeout(fold, 150);
  });
  fold();
})();
