/* The editor island (plan §6.3, C3.5). Everything here is a convenience
   on top of a form that already works: the draft is kept in localStorage
   under the page's path, a banner offers to restore it, and the textareas
   grow with their text. No network, no dependencies, nothing the page
   needs. */
(function () {
  "use strict";
  var form = document.querySelector("form.editor");
  if (!form) return;
  /* The choosing state (plan 06) is the server's: every field rides
     along hidden, so there is nothing to keep and a draft from the
     writing state would only be offered against the wrong fields. */
  var mode = form.elements.place_mode;
  if (mode && mode.value === "choosing") return;
  var key = "ea:draft:" + location.pathname;
  var store = null;
  try { store = window.localStorage; } catch (e) { store = null; }

  function fields() {
    var out = [];
    var els = form.elements;
    for (var i = 0; i < els.length; i++) {
      var el = els[i];
      if (!el.name || el.type === "file" || el.type === "submit" || el.type === "hidden") continue;
      out.push(el);
    }
    return out;
  }
  function snapshot() {
    var data = {};
    fields().forEach(function (el) { data[el.name] = el.value; });
    return data;
  }
  function same(a, b) {
    var ka = Object.keys(a), kb = Object.keys(b);
    if (ka.length !== kb.length) return false;
    return ka.every(function (k) { return a[k] === b[k]; });
  }
  function grow(el) {
    if (el.tagName !== "TEXTAREA") return;
    el.style.height = "auto";
    el.style.height = el.scrollHeight + 2 + "px";
  }

  var timer = null;
  function save() {
    if (!store) return;
    try { store.setItem(key, JSON.stringify({ at: Date.now(), data: snapshot() })); } catch (e) {}
  }
  function scheduleSave() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(save, 400);
  }
  function forget() {
    if (!store) return;
    try { store.removeItem(key); } catch (e) {}
  }

  function banner(saved) {
    var box = document.createElement("div");
    box.className = "notice restore";
    box.setAttribute("role", "status");
    var when = new Date(saved.at);
    var text = document.createElement("span");
    text.textContent = "An unsaved draft from " + when.toLocaleString() + " is on this device. ";
    var restore = document.createElement("button");
    restore.type = "button";
    restore.className = "link-button";
    restore.textContent = "Restore it";
    var discard = document.createElement("button");
    discard.type = "button";
    discard.className = "link-button";
    discard.textContent = "Discard it";
    restore.addEventListener("click", function () {
      fields().forEach(function (el) {
        if (Object.prototype.hasOwnProperty.call(saved.data, el.name)) {
          el.value = saved.data[el.name];
          grow(el);
        }
      });
      box.remove();
    });
    discard.addEventListener("click", function () { forget(); box.remove(); });
    box.appendChild(text);
    box.appendChild(restore);
    box.appendChild(document.createTextNode(" "));
    box.appendChild(discard);
    form.insertBefore(box, form.firstChild);
  }

  if (store) {
    var saved = null;
    try { saved = JSON.parse(store.getItem(key)); } catch (e) { saved = null; }
    if (saved && saved.data && !same(saved.data, snapshot())) banner(saved);
  }

  form.addEventListener("input", function (e) { grow(e.target); scheduleSave(); });
  form.addEventListener("change", scheduleSave);
  form.addEventListener("submit", function (e) {
    var action = e.submitter && e.submitter.value;
    if (action === "publish" || action === "change_place") forget(); else save();
  });
  fields().forEach(grow);
})();
