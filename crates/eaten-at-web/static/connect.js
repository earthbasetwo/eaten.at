/* Connect (plan 09): the landing page's one primary action is a link
   to the sign-in page. With script, pressing it swaps the handle field
   in where the button stood and puts the caret in it, so signing in
   starts on the page the reader is already on. A plain click is the
   only one taken; a modified click or a middle click still opens the
   sign-in page as the link says.

   The swap is drawn by one rule. The 2px of vermilion under "Connect"
   is measured, a rule of its own is laid over it, and that rule slides
   to where the field's rule will be while the label fades out and the
   field fades in; the block eases to the form's height meanwhile, so
   nothing below jumps. When the rule lands it is removed and the
   field's own focus rule, the same 2px in the same place, takes over.
   Both ends are measured, so the stylesheet owns every size; the
   script only carries the numbers across. Positions are physical
   because the measurements are. */
(function () {
  "use strict";
  var root = document.querySelector("[data-connect]");
  if (!root) return;
  var idle = root.querySelector(".connect-idle");
  var button = root.querySelector(".connect-button");
  var form = root.querySelector(".connect-form");
  var input = form && form.querySelector("input[name=handle]");
  if (!idle || !button || !form || !input) return;
  var RULE = 2; /* the rule's thickness, as the stylesheet draws it */
  var SETTLE_AFTER = 400; /* past the longest transition, should its end never fire */
  var live = false;

  function place(rule, origin, box) {
    rule.style.left = box.left - origin.left + "px";
    rule.style.top = box.bottom - origin.top - RULE + "px";
    rule.style.width = box.width + "px";
  }

  button.addEventListener("click", function (e) {
    if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    if (live) return;
    live = true;

    /* Where the rule starts and how tall the block is, before anything
       changes. The block's corner is the same in both states, so one
       origin serves both measurements. */
    var origin = root.getBoundingClientRect();
    var from = button.getBoundingClientRect();
    var startHeight = root.offsetHeight;

    /* Lift the row out of the flow and bring the form in, then read
       where the rule ends and how tall the block becomes. Nothing has
       painted yet, so the reader sees none of this. */
    root.classList.add("connect-live");
    form.hidden = false;
    var to = input.getBoundingClientRect();
    var endHeight = root.offsetHeight;

    var rule = document.createElement("span");
    rule.className = "connect-rule";
    place(rule, origin, from);
    root.style.height = startHeight + "px";
    root.appendChild(rule);
    /* The block clips while it grows, so focus must not scroll the
       field into view inside it: that would shift the form mid-flight.
       The field stands where the button just was, so it is in view. */
    input.focus({ preventScroll: true });

    /* Commit the starting frame, then set the destination; the
       stylesheet's transitions carry everything between. */
    void rule.offsetWidth;
    root.classList.add("connect-arriving");
    place(rule, origin, to);
    root.style.height = endHeight + "px";

    var settled = false;
    function settle() {
      if (settled) return;
      settled = true;
      rule.remove();
      idle.hidden = true;
      root.style.height = "";
      /* The field's own rule must appear in the same frame the moving
         one leaves, not ease in over the field's usual transition. */
      input.style.transition = "none";
      root.classList.remove("connect-live", "connect-arriving");
      void input.offsetWidth;
      input.style.transition = "";
    }
    rule.addEventListener("transitionend", function (ev) {
      if (ev.target === rule) settle();
    });
    setTimeout(settle, SETTLE_AFTER);
  });
})();
