/* The editor's controls, as the Write Pages handoff draws them, each
   over a form control that works on its own: the date, the meal and
   price, the teaser's fold, the link cards, Delete's confirmation. One
   popover at a time; an outside click or Escape closes it. */
(function () {
  "use strict";
  var form = document.querySelector("form.editor-write");
  if (!form) return;
  form.classList.add("js-live");

  function el(tag, className) {
    var node = document.createElement(tag);
    if (className) node.className = className;
    return node;
  }
  function button(className, text) {
    var b = el("button", className);
    b.type = "button";
    if (text !== undefined) b.textContent = text;
    return b;
  }
  function announce(input) {
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
  }

  /* ---- inline fields, measured without field-sizing ---- */
  var fit = (function () {
    if (window.CSS && CSS.supports && CSS.supports("field-sizing", "content")) return function () {};
    var mirror = el("span", "inline-mirror");
    mirror.setAttribute("aria-hidden", "true");
    document.body.appendChild(mirror);
    function fit(input) {
      var style = getComputedStyle(input);
      mirror.style.font = style.font;
      mirror.style.letterSpacing = style.letterSpacing;
      mirror.textContent = input.value || input.placeholder || "";
      input.style.inlineSize = Math.ceil(mirror.getBoundingClientRect().width + 2) + "px";
    }
    form.querySelectorAll(".inline-field input").forEach(function (input) {
      fit(input);
      input.addEventListener("input", function () { fit(input); });
      input.addEventListener("change", function () { fit(input); });
    });
    return fit;
  })();

  // The head follows the place and the title: the name stands in.
  /* The address as shown: without a postcode, as view.rs draws it. */
  function shownAddress(address) {
    var parts = address.split(", "), n = parts.length - 1, last = parts[n];
    var code = function (s) { return /\d/.test(s) && /^[A-Za-z0-9 -]+$/.test(s) && s.replace(/[^A-Za-z]/g, "").length <= 4; };
    var m = /^([A-Z]{2}) (\S+)$/.exec(last);
    if (/^(#|(unit|shop|suite|ste|apt|floor|fl|level|room|building|bldg)\b)/i.test(last));
    else if (m && code(m[2])) parts[n] = m[1];
    else if (n && code(last)) parts.pop();
    else if (n) parts[n] = last.replace(/^\d{3,6} (?=\D)/, "").replace(/(.) ([A-Z]{1,2}\d[A-Z\d]? \d[A-Z]{2}|\d{3,6}(-\d{4})?)$/, "$1");
    return parts.join(", ");
  }
  function showPlace() {
    var name = form.elements.place_name.value, address = form.elements.place_address.value;
    var titled = form.elements.title.value.trim() !== "";
    form.elements.title.placeholder = name;
    form.querySelector(".change-place").setAttribute("aria-label", "Change restaurant: " + name);
    form.querySelector(".place-name-text").textContent = name;
    form.querySelector(".place-name-text").hidden = !titled;
    form.querySelector(".place-comma").hidden = !titled || !address;
    form.querySelector(".place-address-text").textContent = shownAddress(address);
    form.querySelector(".place-where").hidden = !titled && !address;
  }
  form.elements.place_name.addEventListener("change", showPlace);
  form.elements.place_address.addEventListener("change", showPlace);
  // Return in the title goes on to the digest; it is one line.
  form.elements.title.addEventListener("keydown", function (e) {
    if (e.key === "Enter" && !e.isComposing) { e.preventDefault(); (form.querySelector(".digest-editor") || form.elements.body).focus(); }
  });
  form.elements.title.addEventListener("input", showPlace);
  form.elements.title.addEventListener("change", showPlace);

  /* ---- popovers: one at a time; outside click and Escape close ---- */
  var open = null;
  function closePopover() {
    if (open) { open.close(); open = null; }
  }
  function showPopover(popover) {
    if (open && open !== popover) open.close();
    open = popover;
  }
  document.addEventListener("mousedown", function (e) {
    if (open && !open.root.contains(e.target)) closePopover();
  });

  /* ---- the date: a word on a hairline, a calendar under it ---- */
  (function () {
    var input = form.elements.visited_on;
    var field = form.querySelector(".date-field");
    if (!input || !field) return;
    var MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    var trigger = button("date-button");
    trigger.setAttribute("aria-haspopup", "dialog");
    trigger.setAttribute("aria-expanded", "false");
    var pop = el("div", "popover date-popover paper");
    pop.hidden = true;
    pop.setAttribute("role", "dialog");
    pop.setAttribute("aria-label", "Choose the date");
    input.hidden = true;
    field.appendChild(trigger);
    field.appendChild(pop);
    var anchor = null;
    function parts(value) {
      var m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
      return m ? { y: Number(m[1]), m: Number(m[2]), d: Number(m[3]) } : null;
    }
    function pad(n) { return String(n).padStart(2, "0"); }
    function label() {
      var p = parts(input.value);
      trigger.classList.toggle("unset", !p);
      if (!p) { trigger.textContent = "a date"; return; }
      var text = MONTHS[p.m - 1] + " " + p.d;
      if (p.y !== new Date().getFullYear()) text += ", " + p.y;
      trigger.textContent = text;
    }
    function shift(by) {
      var y = anchor.y, m = anchor.m + by;
      if (m < 1) { m = 12; y -= 1; }
      if (m > 12) { m = 1; y += 1; }
      anchor = { y: y, m: m };
      render();
    }
    function render() {
      var y = anchor.y, m = anchor.m;
      pop.textContent = "";
      var head = el("div", "date-head");
      var prev = button("date-nav", "←");
      prev.setAttribute("aria-label", "Earlier month");
      prev.addEventListener("click", function () { shift(-1); });
      var month = el("span", "date-month");
      month.textContent = MONTHS[m - 1] + " " + y;
      var next = button("date-nav", "→");
      next.setAttribute("aria-label", "Later month");
      next.addEventListener("click", function () { shift(1); });
      head.appendChild(prev);
      head.appendChild(month);
      head.appendChild(next);
      var grid = el("div", "date-grid");
      ["S", "M", "T", "W", "T", "F", "S"].forEach(function (d) {
        var dow = el("span", "date-dow");
        dow.textContent = d;
        grid.appendChild(dow);
      });
      var first = new Date(y, m - 1, 1).getDay();
      var days = new Date(y, m, 0).getDate();
      for (var i = 0; i < first; i++) grid.appendChild(el("span", "date-blank"));
      var today = new Date();
      for (var d = 1; d <= days; d++) {
        var iso = y + "-" + pad(m) + "-" + pad(d);
        var day = button("date-day", String(d));
        day.setAttribute("aria-label", MONTHS[m - 1] + " " + d + ", " + y);
        if (iso === input.value) {
          day.classList.add("selected");
          day.setAttribute("aria-pressed", "true");
        }
        if (today.getFullYear() === y && today.getMonth() === m - 1 && today.getDate() === d) day.classList.add("today");
        day.addEventListener("click", function (chosen) {
          return function () {
            input.value = chosen;
            announce(input);
            closePopover();
            trigger.focus();
          };
        }(iso));
        grid.appendChild(day);
      }
      pop.appendChild(head);
      pop.appendChild(grid);
    }
    var popover = {
      root: field,
      trigger: trigger,
      close: function () { pop.hidden = true; trigger.setAttribute("aria-expanded", "false"); }
    };
    trigger.addEventListener("click", function () {
      if (!pop.hidden) { closePopover(); return; }
      var p = parts(input.value);
      var now = new Date();
      anchor = p ? { y: p.y, m: p.m } : { y: now.getFullYear(), m: now.getMonth() + 1 };
      render();
      pop.hidden = false;
      trigger.setAttribute("aria-expanded", "true");
      showPopover(popover);
      (pop.querySelector(".selected") || pop.querySelector(".today") || pop.querySelector(".date-day")).focus();
    });
    input.addEventListener("change", label);
    label();
  })();

  /* ---- the meal and the price: a word over a paper menu ---- */
  form.querySelectorAll(".note-field").forEach(function (field) {
    var select = field.querySelector("select");
    var rule = field.querySelector(".select-rule");
    if (!select || !rule) return;
    var unset = field.getAttribute("data-unset") || "";
    // A word that opens a short list of buttons, the chosen one marked
    // current; named with its field, as "Meal: lunch".
    var trigger = button("note-button");
    trigger.setAttribute("aria-expanded", "false");
    var pop = el("div", "popover note-popover paper");
    pop.hidden = true;
    rule.hidden = true;
    // Where the select was: a problem the server found stays under it.
    rule.after(trigger, pop);
    function label() {
      var chosen = select.options[select.selectedIndex];
      trigger.classList.toggle("unset", !select.value);
      trigger.textContent = select.value && chosen ? chosen.textContent : unset;
      trigger.setAttribute("aria-label", select.getAttribute("aria-label") + ": " + trigger.textContent);
    }
    function choose(value) {
      select.value = value;
      announce(select);
      closePopover();
      trigger.focus();
    }
    function render() {
      pop.textContent = "";
      Array.prototype.forEach.call(select.options, function (option) {
        if (!option.value) return;
        var row = button("note-option", option.textContent);
        if (option.selected) { row.classList.add("selected"); row.setAttribute("aria-current", "true"); }
        row.addEventListener("click", function () { choose(option.value); });
        pop.appendChild(row);
      });
      var clear = button("note-option note-clear", field.dataset.note === "meal" ? "food" : "no note");
      clear.addEventListener("click", function () { choose(""); });
      pop.appendChild(clear);
    }
    var popover = {
      root: field,
      trigger: trigger,
      close: function () { pop.hidden = true; trigger.setAttribute("aria-expanded", "false"); }
    };
    trigger.addEventListener("click", function () {
      if (!pop.hidden) { closePopover(); return; }
      render();
      pop.hidden = false;
      trigger.setAttribute("aria-expanded", "true");
      showPopover(popover);
      (pop.querySelector(".selected") || pop.firstChild).focus();
    });
    select.addEventListener("change", label);
    label();
  });

  /* ---- the teaser ---- */
  (function () {
    var fold = form.querySelector("details.teaser");
    var text = form.elements.description;
    var body = form.elements.body;
    if (!fold || !text) return;
    var whenDefault = fold.querySelector(".teaser-default");
    var whenCustom = fold.querySelector(".teaser-custom");
    // Opened by the author, the fold hands them the field; opened for a
    // restored draft's teaser, it leaves focus alone. Folded, focus
    // goes back to its line.
    var quiet = false;
    function sync() {
      var custom = text.value.trim() !== "";
      whenDefault.hidden = custom;
      whenCustom.hidden = !custom;
      if (custom && !fold.open) { quiet = true; fold.open = true; }
    }
    text.addEventListener("input", sync);
    text.addEventListener("change", sync);
    fold.addEventListener("toggle", function () {
      if (fold.open && !quiet) text.focus();
      quiet = false;
    });
    function shut() { fold.open = false; fold.querySelector("summary").focus(); }
    fold.querySelector("[data-teaser=\"fold\"]").addEventListener("click", shut);
    fold.querySelector("[data-teaser=\"reset\"]").addEventListener("click", function () {
      text.value = "";
      announce(text);
      shut();
    });
    // The first lines, roughly as the listings draw them.
    function firstLines(markdown) {
      var lines = markdown.replace(/\r\n?/g, "\n").split("\n");
      var line = "";
      for (var i = 0; i < lines.length; i++) {
        var t = lines[i].trim();
        if (!t || /^#{1,6}\s/.test(t)) continue;
        line = t;
        break;
      }
      line = line
        .replace(/^([-*]|\d+\.|>)\s+/, "")
        .replace(/\*\*([^*]+)\*\*/g, "$1")
        .replace(/\*([^*]+)\*/g, "$1")
        .replace(/_([^_]+)_/g, "$1")
        .replace(/`([^`]+)`/g, "$1")
        .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
        .replace(/\s+/g, " ");
      if (line.length > 160) {
        var cut = line.lastIndexOf(" ", 160);
        line = line.slice(0, cut > 100 ? cut : 160).replace(/[\s,;:.]+$/, "") + "…";
      }
      return line;
    }
    function auto() {
      if (!body) return;
      var drawn = firstLines(body.value);
      text.placeholder = drawn ? "“" + drawn + "”" : "the first lines, once there are some";
    }
    if (body) {
      body.addEventListener("input", auto);
      body.addEventListener("change", auto);
    }
    sync();
  })();

  /* ---- links: a sentence of labels, one card at a time ---- */
  (function () {
    var block = form.querySelector(".elsewhere");
    if (!block) return;
    var words = block.querySelector(".link-words");
    var addSlot = block.querySelector(".add-link-slot");
    var addButton = block.querySelector(".add-link");
    var cards = Array.prototype.slice.call(block.querySelectorAll(".link-card"));
    var editing = null;
    var before = null;
    function host(url) {
      try { return new URL(url).host.replace(/^www\./, ""); } catch (e) { return url; }
    }
    function fields(card) {
      return {
        url: card.querySelector("input[type=\"url\"]"),
        label: card.querySelector("input[type=\"text\"]"),
        open: card.querySelector(".open-link"),
        remove: card.querySelector("button[value^=\"remove_link\"]"),
        keep: card.querySelector(".link-card-keep")
      };
    }
    function value(card) {
      var f = fields(card);
      return { url: f.url.value.trim(), label: f.label.value.trim() };
    }
    function render() {
      words.textContent = "";
      var filled = cards.filter(function (card) { return value(card).url !== ""; });
      filled.forEach(function (card, k) {
        var v = value(card);
        var word = button("link-word", v.label || host(v.url));
        word.title = v.url;
        word.addEventListener("click", function () { openCard(card); });
        words.appendChild(word);
        if (k < filled.length - 1) {
          var comma = el("span", "soft");
          comma.textContent = ",";
          words.appendChild(comma);
        }
        words.appendChild(document.createTextNode(" "));
      });
      block.classList.toggle("has-links", filled.length > 0);
      addSlot.hidden = editing !== null;
    }
    function closeCards() {
      cards.forEach(function (card) { card.hidden = true; });
      editing = null;
      render();
    }
    function openCard(card, still) {
      cards.forEach(function (c) { c.hidden = c !== card; });
      editing = card;
      before = value(card);
      var f = fields(card);
      f.keep.hidden = false;
      if (f.remove) f.remove.hidden = before.url === "";
      render();
      fit(f.url);
      fit(f.label);
      if (!still) f.label.focus();
    }
    cards.forEach(function (card) {
      var f = fields(card);
      card.hidden = true;
      f.url.addEventListener("input", function () {
        f.open.href = f.url.value.trim() || "#";
      });
      function save() {
        if (editing !== card) return;
        if (value(card).url === "") { cancel(); return; }
        announce(f.url);
        closeCards();
        addButton.focus();
      }
      function cancel() {
        if (editing !== card) return;
        f.url.value = before.url;
        f.label.value = before.label;
        announce(f.url);
        closeCards();
        addButton.focus();
      }
      card.querySelector("[data-link-save]").addEventListener("click", save);
      card.querySelector("[data-link-cancel]").addEventListener("click", cancel);
      if (f.remove) {
        /* Without script this asks the server to drop the row; here the
           row is emptied, which the server skips. */
        f.remove.addEventListener("click", function (e) {
          e.preventDefault();
          f.url.value = "";
          f.label.value = "";
          announce(f.url);
          closeCards();
          addButton.focus();
        });
      }
      card.addEventListener("keydown", function (e) {
        if (e.isComposing || e.keyCode === 229) return;
        if (e.key === "Enter") { e.preventDefault(); save(); }
        else if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); cancel(); }
      });
    });
    // A restored draft sets the rows; a link the server refused opens.
    block.addEventListener("change", function () { if (!editing) render(); });
    var refused = cards.filter(function (card) { return card.querySelector(".field-error"); })[0];
    if (addButton) {
      // A blank row opens in place; else the server adds one.
      addButton.addEventListener("click", function (e) {
        var blank = cards.filter(function (card) { return value(card).url === ""; })[0];
        if (!blank) return;
        e.preventDefault();
        openCard(blank);
      });
    }
    if (refused) openCard(refused, true); else render();
  })();

  /* ---- delete, confirmed in its own slot ---- */
  var confirm = form.querySelector("details.delete-confirm");
  if (confirm) {
    confirm.querySelector(".confirm-no").addEventListener("click", function () {
      confirm.open = false;
      confirm.querySelector("summary").focus();
    });
    confirm.addEventListener("toggle", function () {
      if (confirm.open) confirm.querySelector(".confirm-no").focus();
    });
  }

  /* Escape closes the topmost transient. */
  document.addEventListener("keydown", function (e) {
    if (e.key !== "Escape") return;
    if (open) { var back = open.trigger; closePopover(); back.focus(); return; }
    if (confirm && confirm.open) { confirm.open = false; confirm.querySelector("summary").focus(); }
  });
})();
