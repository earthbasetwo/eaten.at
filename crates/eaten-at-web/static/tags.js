/* Filed under: the comma list becomes chips. The server's field stays,
   hidden, as the carrier; a slot takes one tag at a time. Return or a
   comma files it, a chip takes itself out when pressed, Backspace in
   the empty slot takes the last back, and Return there sends the form.
   Changes are announced on the carrier, and a restored draft's too.
   Tags match as the server's do: case and runs of space aside. */
(function () {
  "use strict";
  var input = document.querySelector("form.editor input[name=\"tags\"]");
  if (!input || !input.form) return;
  var field = input.parentNode;
  var carrier = document.createElement("input");
  carrier.type = "text";
  carrier.name = "tags";
  carrier.id = "tags-value";
  carrier.hidden = true;
  carrier.tabIndex = -1;
  carrier.value = input.value;
  input.removeAttribute("name");
  input.value = "";
  var chips = document.createElement("span");
  chips.className = "tag-chips";
  field.insertBefore(carrier, input);
  field.insertBefore(chips, input);
  var tags = [];

  function clean(t) { return t.trim().replace(/^#+/, "").trim(); }
  function key(t) { return t.split(/\s+/).join(" ").toLowerCase(); }
  function has(t) { var k = key(t); return tags.some(function (x) { return key(x) === k; }); }
  function render() {
    chips.textContent = "";
    tags.forEach(function (t) {
      var chip = document.createElement("button");
      chip.type = "button";
      chip.className = "chip";
      chip.textContent = t;
      chip.title = "Remove " + t;
      chip.addEventListener("click", function () { remove(t); input.focus(); });
      chips.appendChild(chip);
      chips.appendChild(document.createTextNode(", "));
    });
    carrier.value = tags.join(", ");
    input.placeholder = tags.length ? "another tag" : "a tag";
  }
  function announce() { carrier.dispatchEvent(new Event("input", { bubbles: true })); }
  // A chip filed from the keyboard strikes through only once the pointer moves.
  function still() {
    chips.classList.add("still");
    document.addEventListener("pointermove", function () { chips.classList.remove("still"); }, { once: true });
  }
  function add(t) {
    t = clean(t);
    if (!t || has(t)) return;
    tags.push(t);
    render();
    still();
    announce();
  }
  function remove(t) {
    tags = tags.filter(function (x) { return x !== t; });
    render();
    announce();
  }
  /* Whatever is in the slot becomes chips. */
  function file() {
    var parts = input.value.split(",");
    input.value = "";
    parts.forEach(add);
  }
  /* The carrier is the truth: read it into chips. */
  function read() {
    tags = [];
    carrier.value.split(",").map(clean).forEach(function (t) { if (t && !has(t)) tags.push(t); });
    render();
  }

  read();
  input.addEventListener("input", function () {
    if (input.value.indexOf(",") < 0) return;
    var parts = input.value.split(",");
    input.value = parts.pop();
    parts.forEach(add);
  });
  input.addEventListener("keydown", function (e) {
    /* An input method is mid-word: Return belongs to it. */
    if (e.isComposing || e.keyCode === 229) return;
    if (e.key === "Enter" && input.value.trim()) { e.preventDefault(); file(); }
    else if (e.key === "Backspace" && !input.value && tags.length) { e.preventDefault(); remove(tags[tags.length - 1]); }
  });
  input.addEventListener("blur", file);
  input.form.addEventListener("submit", file);
  carrier.addEventListener("change", read);
})();
