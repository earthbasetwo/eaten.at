/* Choosing the place (plan 12). Suggestions open under the name as it
   is typed, from this site's suggest endpoint, with a last row that
   takes what was typed as a place by hand (PL26). A pick fills name and
   address and is sent as the pick, read back from the same cached
   search; typing over either line makes it a place by hand again, and
   clearing the name clears the address. Without this, both are typed. */
(function () {
  "use strict";
  var name = document.querySelector("form.editor-choosing input[name=\"place_name\"]");
  if (!name || !name.form) return;
  var form = name.form;
  var address = form.elements.place_address;
  var query = form.elements.place_query;
  var start = form.querySelector("#start-writing");
  var picked = null, byHand = false;
  // The near line (plan 15) shows until a place is picked; the address
  // line takes its place, or joins it for a place by hand.
  var near = form.elements.near, nearLine = form.querySelector(".near-line");
  var placeLine = form.querySelector(".place-line"), hint = form.querySelector(".address-hint");
  function lines() {
    if (!nearLine) return;
    var isPick = matchesPick();
    nearLine.hidden = isPick;
    placeLine.hidden = !isPick && !byHand;
    if (hint) hint.hidden = isPick || !byHand;
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

  function matchesPick() {
    return picked !== null && name.value === picked.name && address.value === picked.address;
  }
  // Start writing shows once there is a name, as the pick or by hand.
  function arm() {
    if (start) {
      start.hidden = !name.value.trim();
      start.value = matchesPick() ? "pick:" + picked.i : "manual";
    }
    if (query) query.value = matchesPick() ? picked.q : "";
    lines();
  }

  name.addEventListener("input", function () {
    if (!name.value.trim()) {
      name.value = "";
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

  var url = name.getAttribute("data-suggest");
  if (url && window.eaCombobox) {
    var searched = "";
    window.eaCombobox(name, {
      minChars: 3,
      delay: 300,
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
        return { label: hit.name, detail: hit.detail };
      },
      pick: function (hit) {
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
        nearLine.querySelector(".near-town").textContent = t.label;
        nearLine.querySelector("summary").textContent = "change";
        town.value = "";
        town.closest("details").open = false;
        name.focus();
        name.dispatchEvent(new Event("input", { bubbles: true }));
      }
    });
  }
  // Return with nothing highlighted leaves the field as typed.
  name.addEventListener("keydown", function (e) {
    if (e.isComposing || e.keyCode === 229) return;
    if (e.key === "Enter") { e.preventDefault(); name.blur(); }
  });
  arm();
  // On arrival, typing replaces the current restaurant. Do this once,
  // so later clicks can still position the caret for a small correction.
  if (name.value) {
    name.focus();
    name.select();
  }
})();
