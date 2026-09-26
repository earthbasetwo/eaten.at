/* A combobox over a text input (plan 10), the WAI-ARIA pattern: arrows
   move, Enter picks, Escape closes. Enter never swallows the send: it
   takes a highlighted row or leaves the typing, then submits when the
   source asks (opts.send). The form works without it. */
window.eaCombobox = function (input, opts) {
  "use strict";
  var id = input.id + "-list";
  var list = document.createElement("ul");
  list.className = "combobox-list";
  list.id = id;
  list.setAttribute("role", "listbox");
  list.hidden = true;
  var anchor = input.closest(".lookup-row") || input;
  anchor.parentNode.insertBefore(list, anchor.nextSibling);
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-controls", id);
  input.setAttribute("aria-expanded", "false");
  var items = [], active = -1, timer = null, pending = null;

  function close() {
    list.hidden = true;
    list.textContent = "";
    items = [];
    active = -1;
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
  }
  function highlight(i) {
    active = i;
    var rows = list.children;
    for (var k = 0; k < rows.length; k++) rows[k].setAttribute("aria-selected", k === i ? "true" : "false");
    if (i >= 0) input.setAttribute("aria-activedescendant", rows[i].id);
    else input.removeAttribute("aria-activedescendant");
  }
  function pick(i) {
    var item = items[i];
    close();
    if (item) opts.pick(item);
  }
  function show(found) {
    items = found;
    list.textContent = "";
    items.forEach(function (item, i) {
      var row = document.createElement("li");
      row.className = "combobox-option";
      row.id = id + "-" + i;
      row.setAttribute("role", "option");
      row.setAttribute("aria-selected", "false");
      var view = opts.render(item);
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
    if (q.length < opts.minChars) { close(); return; }
    var ctrl = new AbortController();
    pending = ctrl;
    opts.source(q, ctrl.signal).then(function (found) {
      if (pending === ctrl) show(found || []);
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
    timer = setTimeout(search, opts.delay);
  });
  input.addEventListener("keydown", function (e) {
    /* An input method is mid-word: Enter and the arrows belong to it,
       not to the menu. */
    if (e.isComposing || e.keyCode === 229) return;
    if (list.hidden) return;
    if (e.key === "ArrowDown") { e.preventDefault(); highlight((active + 1) % items.length); }
    else if (e.key === "ArrowUp") { e.preventDefault(); highlight((active - 1 + items.length) % items.length); }
    else if (e.key === "Enter") {
      if (active >= 0) pick(active); else close();
      if (opts.send && input.form) { e.preventDefault(); submit(input.form); }
      else if (active >= 0) e.preventDefault();
    }
    else if (e.key === "Escape") { close(); }
  });
  input.addEventListener("blur", function () { setTimeout(close, 150); });
};
