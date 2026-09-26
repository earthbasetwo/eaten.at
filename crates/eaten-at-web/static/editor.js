/* The editor island (plan §6.3, C3.5), a convenience on a form that
   already works: the draft is kept in localStorage under the page's
   path, a banner offers to restore it, and the textareas grow with
   their text. The photos ride in the draft as their hidden fields;
   restoring hands them to the photos island, which redraws. */
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

  var placeFields = ["place_name", "place_address", "place_mode", "gers_id", "lat_e6", "lon_e6"];
  function isPhotoField(name) { return name.indexOf("photo_") === 0; }
  function fields() {
    var out = [];
    var els = form.elements;
    for (var i = 0; i < els.length; i++) {
      var el = els[i];
      if (!el.name || el.type === "file" || el.type === "submit") continue;
      if (el.type === "hidden" && placeFields.indexOf(el.name) < 0 && !isPhotoField(el.name)) continue;
      out.push(el);
    }
    return out;
  }
  /* A radio group or a checkbox is kept as the value that is checked
     ("" for none), once; restoring checks the control with that value. */
  function ticks(el) { return el.type === "radio" || el.type === "checkbox"; }
  function snapshot() {
    var data = {};
    fields().forEach(function (el) {
      if (!ticks(el)) data[el.name] = el.value;
      else if (el.checked || !(el.name in data)) data[el.name] = el.checked ? el.value : "";
    });
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
    var at = Date.now();
    // `photos` marks a draft that carries its photo list, so restoring
    // an older one leaves the photos on the page alone.
    // `v: 2` marks a draft that keeps radios by what is checked.
    try { store.setItem(key, JSON.stringify({ at: at, photos: true, v: 2, data: snapshot() })); return at; } catch (e) {}
  }
  function scheduleSave() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(save, 400);
  }
  function forget() {
    clearTimeout(timer);
    timer = null;
    if (!store) return;
    try { store.removeItem(key); } catch (e) {}
  }

  function banner(saved) {
    var box = document.createElement("div");
    box.className = "notice restore";
    box.setAttribute("role", "status");
    var completePlace = placeFields.every(function (name) { return typeof saved.data[name] === "string"; });
    var when = new Date(saved.at);
    var text = document.createElement("span");
    text.textContent = "An unsaved draft from " + when.toLocaleString() + " is on this device. ";
    if (!completePlace) text.textContent += "Check the restaurant after restoring this older draft. ";
    var restore = document.createElement("button");
    restore.type = "button";
    restore.className = "link-button";
    restore.textContent = "Restore it";
    var discard = document.createElement("button");
    discard.type = "button";
    discard.className = "link-button";
    discard.textContent = "Discard it";
    restore.addEventListener("click", function () {
      // Older drafts lack place identity fields. Never restore only a
      // name/address over a different restaurant's ID and coordinates.
      // Older drafts kept the last radio's value, not the checked one.
      var restored = fields().filter(function (el) {
        return Object.prototype.hasOwnProperty.call(saved.data, el.name) && !isPhotoField(el.name) &&
          (completePlace || placeFields.indexOf(el.name) < 0) && (saved.v || !ticks(el));
      });
      restored.forEach(function (el) {
        if (ticks(el)) el.checked = el.value === saved.data[el.name];
        else el.value = saved.data[el.name];
        grow(el);
      });
      // Notify islands only after all related fields are restored. The
      // photos are a list the photos island owns: it takes the draft's
      // whole and redraws.
      restored.forEach(function (el) {
        el.dispatchEvent(new Event("change", { bubbles: true }));
      });
      if (saved.photos) form.dispatchEvent(new CustomEvent("draft-restore", { detail: saved.data }));
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
    if (action === "change_place") forget(); else {
      clearTimeout(timer);
      form.elements.draft_id.value = save() || "";
    }
  });
  fields().forEach(grow);
})();
