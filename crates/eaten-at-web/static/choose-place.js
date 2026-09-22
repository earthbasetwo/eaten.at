/* Choosing the place (plan 12, the Write Pages handoff). Suggestions
   open under the place's name as it is typed, from this site's own
   suggest endpoint through the combobox. Picking one fills the name and
   the address and remembers which suggestion it was, so Start writing
   sends the pick and the server reads the place from the same cached
   search it suggested from; type over either line and it is a place by
   hand again. Clearing the name clears the address with it, so an
   address from an earlier pick is never left behind. Without this, the
   two lines are typed and Start writing takes them as written. */
(function () {
  "use strict";
  var name = document.querySelector("form.editor-choosing input[name=\"place_name\"]");
  if (!name || !name.form) return;
  var form = name.form;
  var address = form.elements.place_address;
  var query = form.elements.place_query;
  var start = form.querySelector("#start-writing");
  var picked = null;

  /* Where field-sizing is not understood, an inline field is measured
     against a mirror of its text. */
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
  /* Start writing appears once there is a name, and says whether it is
     the pick or what was typed. */
  function arm() {
    if (start) {
      start.hidden = !name.value.trim();
      start.value = matchesPick() ? "pick:" + picked.i : "manual";
    }
    if (query) query.value = matchesPick() ? picked.q : "";
  }

  name.addEventListener("input", function () {
    if (!name.value.trim()) {
      name.value = "";
      address.value = "";
      address.dispatchEvent(new Event("input", { bubbles: true }));
    }
    arm();
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
        return fetch(url + "?q=" + encodeURIComponent(q), {
          signal: signal,
          credentials: "same-origin",
          headers: { Accept: "application/json" }
        })
          .then(function (r) { return r.ok ? r.json() : { hits: [] }; })
          .then(function (body) {
            searched = body.q || q;
            return body.hits || [];
          });
      },
      render: function (hit) { return { label: hit.name, detail: hit.detail }; },
      pick: function (hit) {
        picked = { i: hit.i, name: hit.name, address: hit.address || "", q: searched };
        name.value = hit.name;
        address.value = picked.address;
        address.dispatchEvent(new Event("input", { bubbles: true }));
        arm();
      }
    });
  }
  /* Return with nothing highlighted keeps what was typed and leaves the
     field; the combobox has already taken a highlighted row. Start
     writing is a press away either way. */
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
