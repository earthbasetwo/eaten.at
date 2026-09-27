/* Choosing the place (plan 12): suggestions as the name is typed, a
   last row for a place by hand (PL26), the author's recent places
   before typing (PL11). A pick is sent as its index into the cached
   search; a recent place as its fields. A corrected name keeps the
   pick (PL29); another name drops it; an edited address is by hand. */
(function () {
  "use strict";
  var name = document.querySelector("form.editor-choosing input[name=\"place_name\"]");
  if (!name || !name.form) return;
  var form = name.form;
  var address = form.elements.place_address;
  var query = form.elements.place_query;
  var start = form.querySelector("#start-writing");
  var picked = null, byHand = false;
  // The second line (D48): "near" the town, or "at" the address once a
  // place is picked or by hand. Home keeps "near".
  var near = form.elements.near, nearLine = form.querySelector(".near-line");
  var placeLine = form.querySelector(".place-line");
  function isHome() { return /^home$/i.test(name.value.trim()) && !matchesPick(); }
  function lines() {
    if (!nearLine) return;
    var at = (matchesPick() || byHand) && !isHome();
    nearLine.hidden = at;
    placeLine.hidden = !at;
    if (isHome() && address.value) { address.value = ""; }
  }

  // Measured against a mirror where field-sizing is not understood.
  (function () {
    if (window.CSS && CSS.supports && CSS.supports("field-sizing", "content")) return;
    var mirror = document.createElement("span");
    mirror.className = "inline-mirror";
    mirror.setAttribute("aria-hidden", "true");
    document.body.appendChild(mirror);
    function fit(el) {
      var style = getComputedStyle(el);
      mirror.style.font = style.font;
      mirror.style.letterSpacing = style.letterSpacing;
      mirror.textContent = el.value || el.placeholder || "";
      el.style.inlineSize = Math.ceil(mirror.getBoundingClientRect().width + 2) + "px";
    }
    form.querySelectorAll(".inline-field input").forEach(function (el) {
      fit(el);
      el.addEventListener("input", function () { fit(el); });
    });
  })();

  function fold(s) {
    return s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");
  }
  function distance(a, b) {
    var prev = [], cur, i, j;
    for (j = 0; j <= b.length; j++) prev[j] = j;
    for (i = 1; i <= a.length; i++) {
      cur = [i];
      for (j = 1; j <= b.length; j++) {
        cur[j] = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
      }
      prev = cur;
    }
    return prev[b.length];
  }
  // Still the picked place: the same folded, a start or tail of it, or
  // a typo or two away (PL29).
  function corrects(typed, pickedName) {
    var a = fold(typed), b = fold(pickedName);
    if (!a || /^home$/i.test(typed.trim())) return false;
    if (a === b) return true;
    if (a.length >= 3 && (b.indexOf(a) === 0 || a.indexOf(b) === 0)) return true;
    return distance(a, b) <= (Math.min(a.length, b.length) >= 6 ? 2 : 1);
  }
  function matchesPick() {
    return picked !== null && corrects(name.value, picked.name) && address.value === picked.address;
  }
  var ids = ["gers_id", "lat_e6", "lon_e6", "place_category"];
  function fill(values) {
    ids.forEach(function (id) {
      if (form.elements[id]) form.elements[id].value = values[id] === undefined || values[id] === null ? "" : values[id];
    });
  }
  function arm() {
    if (start) {
      start.hidden = !name.value.trim();
      start.value = matchesPick() ? (picked.take ? "take" : "pick:" + picked.i) : "manual";
    }
    if (!matchesPick()) fill({});
    if (query) query.value = matchesPick() ? picked.q : "";
    lines();
  }

  name.addEventListener("input", function () {
    // Another name, or none, drops the pick and its address (a
    // correction keeps both, PL29).
    if (!name.value.trim() || (picked && !corrects(name.value, picked.name))) {
      if (!name.value.trim()) name.value = "";
      picked = null;
      address.value = "";
      byHand = false;
      address.dispatchEvent(new Event("input", { bubbles: true }));
    }
    arm();
  });
  name.addEventListener("blur", function () {
    if (name.value.trim() && !matchesPick()) { byHand = true; arm(); }
  });
  address.addEventListener("input", arm);

  var url = name.getAttribute("data-suggest"), recentUrl = name.getAttribute("data-recent");
  if (url && window.eaCombobox) {
    var searched = "";
    window.eaCombobox(name, {
      minChars: 3,
      delay: 300,
      empty: recentUrl ? function (q, signal) {
        if (picked || byHand) return Promise.resolve([]);
        return fetch(recentUrl, { signal: signal, credentials: "same-origin", headers: { Accept: "application/json" } })
          .then(function (r) { return r.ok ? r.json() : []; })
          .then(function (places) {
            return places.map(function (p) { p.recent = true; return p; });
          });
      } : undefined,
      source: function (q, signal) {
        /* The list stays shut while the lines still read as the pick. */
        if (matchesPick()) return Promise.resolve([]);
        return fetch(url + "?q=" + encodeURIComponent(q) + (near ? "&near=" + near.value : ""), {
          signal: signal,
          credentials: "same-origin",
          headers: { Accept: "application/json" }
        })
          .then(function (r) { return r.ok ? r.json() : { hits: [] }; })
          .then(function (body) {
            searched = body.q || q;
            var hits = body.hits || [];
            hits.push({ byHand: true, name: q });
            return hits;
          });
      },
      render: function (hit) {
        if (hit.byHand) return { label: "Add \u201c" + hit.name + "\u201d by hand", kind: "combobox-action" };
        if (hit.recent) return { label: hit.name, detail: hit.address || "" };
        return { label: hit.name, detail: hit.detail };
      },
      pick: function (hit) {
        if (hit.recent) {
          // Id, position and category ride in the hidden fields.
          picked = { take: true, name: hit.name, address: hit.address || "", q: "" };
          byHand = false;
          fill({ gers_id: hit.gersId, lat_e6: hit.latE6, lon_e6: hit.lonE6, place_category: hit.category });
          name.value = hit.name;
          address.value = picked.address;
          address.dispatchEvent(new Event("input", { bubbles: true }));
          arm();
          return;
        }
        if (hit.byHand) {
          picked = null;
          byHand = true;
          name.value = hit.name;
          address.value = "";
          address.dispatchEvent(new Event("input", { bubbles: true }));
          arm();
          address.focus();
          return;
        }
        picked = { i: hit.i, name: hit.name, address: hit.address || "", q: searched };
        name.value = hit.name;
        address.value = picked.address;
        address.dispatchEvent(new Event("input", { bubbles: true }));
        arm();
      }
    });
  }
  // Arrival focused the name before this ran.
  if (recentUrl && document.activeElement === name && !name.value.trim()) name.dispatchEvent(new Event("focus"));
  var town = form.elements.near_query;
  if (town && near && window.eaCombobox) {
    var nearUrl = town.getAttribute("data-near");
    window.eaCombobox(town, {
      minChars: 2,
      delay: 150,
      source: function (q, signal) {
        return fetch(nearUrl + "?q=" + encodeURIComponent(q) + "&near=" + near.value, {
          signal: signal,
          credentials: "same-origin",
          headers: { Accept: "application/json" }
        }).then(function (r) { return r.ok ? r.json() : []; });
      },
      render: function (t) { return { label: t.label }; },
      pick: function (t) {
        near.value = t.id;
        townButton.textContent = t.label;
        closeTown();
        name.focus();
        name.dispatchEvent(new Event("input", { bubbles: true }));
      }
    });
    // The town is a button until pressed; then the field takes its place.
    var townButton = nearLine.querySelector(".near-town"), townField = town.closest(".inline-field");
    function closeTown() { townField.hidden = true; townButton.hidden = false; town.value = ""; }
    townButton.addEventListener("click", function () {
      townButton.hidden = true;
      townField.hidden = false;
      town.focus();
    });
    town.addEventListener("blur", function () {
      setTimeout(function () { if (document.activeElement !== town) closeTown(); }, 200);
    });
  }
  // Return with nothing highlighted leaves the field as typed.
  name.addEventListener("keydown", function (e) {
    if (e.isComposing || e.keyCode === 229) return;
    if (e.key === "Enter") { e.preventDefault(); name.blur(); }
  });
  arm();
  // Typing on arrival replaces the current name; a later click still
  // places the caret.
  if (name.value) {
    name.focus();
    name.select();
  }
})();
