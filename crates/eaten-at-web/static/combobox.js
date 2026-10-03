/* A combobox over a text input (plan 10), the WAI-ARIA pattern: arrows
   move, Enter picks, or a pick and Enter both send when the source asks
   (opts.send), Escape closes. */
window.eaCombobox = function (input, opts) {
  "use strict";
  var id = input.id + "-list";
  var list = document.createElement("ul");
  list.className = "combobox-list";
  list.id = id;
  list.setAttribute("role", "listbox");
  list.hidden = true;
  var anchor = opts.anchor || input.closest(".lookup-row") || input;
  anchor.parentNode.insertBefore(list, anchor.nextSibling);
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-controls", id);
  input.setAttribute("aria-expanded", "false");
  var items = [], active = -1, timer = null, pending = null, resting = false, slow = null, live = null, told = false;

  /* Said to screen readers when a search runs long enough to be shown. */
  function say(text) {
    if (!live) {
      live = document.createElement("div");
      live.className = "visually-hidden";
      live.setAttribute("aria-live", "polite");
      list.parentNode.insertBefore(live, list.nextSibling);
    }
    live.textContent = text;
  }
  function settle() {
    clearTimeout(slow);
    list.classList.remove("stale");
    list.removeAttribute("aria-busy");
    if (opts.busy) opts.busy(false);
  }

  function close() {
    settle();
    list.hidden = true;
    list.textContent = "";
    items = [];
    active = -1;
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
  }
  function highlight(i) {
    active = i;
    var rows = list.querySelectorAll(".combobox-option");
    for (var k = 0; k < rows.length; k++) rows[k].setAttribute("aria-selected", k === i ? "true" : "false");
    if (i >= 0) input.setAttribute("aria-activedescendant", rows[i].id);
    else input.removeAttribute("aria-activedescendant");
  }
  function pick(i) {
    var item = items[i];
    close();
    /* A pick may answer with rows to show instead (a "show more" row). */
    var again = item && opts.pick(item);
    if (again) show(again);
    /* A source that sends (opts.send) sends on a pick by pointer too,
       since a click that only filled the field read as nothing (L6). */
    else if (item && opts.send && input.form) submit(input.form);
  }
  function show(found, heading) {
    settle();
    resting = !!heading;
    items = found;
    list.textContent = "";
    if (told) { say(found.length ? found.length + " rows" : "Nothing found"); told = false; }
    if (heading && items.length) {
      var head = document.createElement("li");
      head.className = "combobox-heading kicker";
      head.setAttribute("role", "presentation");
      head.textContent = heading;
      list.appendChild(head);
    }
    items.forEach(function (item, i) {
      var row = document.createElement("li");
      row.className = "combobox-option";
      row.id = id + "-" + i;
      row.setAttribute("role", "option");
      row.setAttribute("aria-selected", "false");
      var view = opts.render(item);
      if (view.kind) row.classList.add(view.kind);
      var label = document.createElement("span");
      label.className = "combobox-label";
      label.textContent = view.label;
      row.appendChild(label);
      if (view.detail) {
        var detail = document.createElement("span");
        detail.className = "combobox-detail";
        detail.textContent = view.detail;
        row.appendChild(detail);
      }
      row.addEventListener("mousedown", function (e) { e.preventDefault(); pick(i); });
      row.addEventListener("click", function () { pick(i); });
      list.appendChild(row);
    });
    active = -1;
    list.hidden = items.length === 0;
    input.setAttribute("aria-expanded", items.length ? "true" : "false");
  }
  function search() {
    var q = input.value.trim();
    if (pending) pending.abort();
    /* opts.empty: rows for an empty field, before anything is typed. */
    var empty = !q && opts.empty, from = empty ? opts.empty : q.length < opts.minChars ? null : opts.source;
    if (!from) { close(); return; }
    var ctrl = new AbortController();
    pending = ctrl;
    /* A slow search: the rows on show step back at once, and after
       400 ms the page is told (opts.busy) and screen readers hear where
       it looks (opts.searching), so a cached answer never flickers
       either (PL31, Ken 2026-09-27). */
    if (opts.searching && !empty) {
      list.classList.add("stale");
      clearTimeout(slow);
      slow = setTimeout(function () {
        if (pending !== ctrl) return;
        list.setAttribute("aria-busy", "true");
        if (opts.busy) opts.busy(true);
        say(opts.searching());
        told = true;
      }, 400);
    }
    from(q, ctrl.signal).then(function (found) {
      if (pending === ctrl) show(found || [], empty && opts.emptyHeading);
    }, function () {
      if (pending === ctrl) close();
    });
  }

  function submit(form) {
    if (form.requestSubmit) form.requestSubmit();
    else form.submit();
  }

  input.addEventListener("input", function () {
    clearTimeout(timer);
    /* Typing closes the empty field's rows. */
    if (resting && input.value.trim()) close();
    timer = setTimeout(search, opts.delay);
  });
  if (opts.empty) {
    input.addEventListener("focus", function () { if (!input.value.trim()) search(); });
  }
  input.addEventListener("keydown", function (e) {
    // Mid-word in an input method, the keys are its own.
    if (e.isComposing || e.keyCode === 229) return;
    if (list.hidden) return;
    if (e.key === "ArrowDown") { e.preventDefault(); highlight((active + 1) % items.length); }
    else if (e.key === "ArrowUp") { e.preventDefault(); highlight((active - 1 + items.length) % items.length); }
    else if (e.key === "Enter") {
      var had = active >= 0;
      if (had) { e.preventDefault(); pick(active); return; }
      close();
      if (opts.send && input.form) { e.preventDefault(); submit(input.form); }
    }
    else if (e.key === "Escape") { close(); }
  });
  input.addEventListener("blur", function () { setTimeout(close, 150); });
};
