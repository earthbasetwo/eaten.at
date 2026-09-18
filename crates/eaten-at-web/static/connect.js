/* Connect (plan 09): each way in on the landing page is a link to the
   page that would ask for the handle. With script, pressing it swaps
   that handle field in where the button stood and puts the caret in it,
   so signing in, or looking someone up, starts on the page the reader
   is already on. A plain click is the only one taken; a modified click
   or a middle click still opens the linked page as the link says.

   The swap is drawn by one rule. The rule under the button is measured,
   a rule of its own is laid over it, and that rule slides to where the
   field's rule will be while the label fades out and the field fades
   in; the block eases to the form's height meanwhile, so nothing below
   jumps. It also grows and reddens on the way where the two ends
   differ: the primary starts on its own 2px of vermilion, a secondary
   on 1px of ink, and both land on the field's 2px of vermilion. When
   the rule lands it is removed and the field's own focus rule, the same
   2px in the same place, takes over. Both ends are measured and both
   thicknesses are named by the stylesheet, so the stylesheet owns every
   size; the script only carries the numbers across. Positions are
   physical because the measurements are. */
(function () {
  "use strict";
  var SETTLE_AFTER = 400; /* past the longest transition, should its end never fire */

  /* A length the stylesheet names on the block, in pixels. */
  function rule(root, name) {
    return parseFloat(getComputedStyle(root).getPropertyValue(name)) || 0;
  }

  function place(moving, origin, box, thickness) {
    moving.style.left = box.left - origin.left + "px";
    moving.style.top = box.bottom - origin.top - thickness + "px";
    moving.style.width = box.width + "px";
  }

  function wire(root) {
    var idle = root.querySelector(".connect-idle");
    var button = root.querySelector(".connect-button");
    var form = root.querySelector(".connect-form");
    var input = form && form.querySelector("input[name=handle]");
    if (!idle || !button || !form || !input) return;
    var live = false;

    button.addEventListener("click", function (e) {
      if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      e.preventDefault();
      if (live) return;
      live = true;

      /* Where the rule starts, how thick it is at either end, and how
         tall the block is, before anything changes. The block's corner
         is the same in both states, so one origin serves both
         measurements. */
      var origin = root.getBoundingClientRect();
      var from = button.getBoundingClientRect();
      var startHeight = root.offsetHeight;
      var thickFrom = rule(root, "--connect-rule-from");
      var thickTo = rule(root, "--connect-rule-to");

      /* Lift the row out of the flow and bring the form in, then read
         where the rule ends and how tall the block becomes. Nothing has
         painted yet, so the reader sees none of this. */
      root.classList.add("connect-live");
      form.hidden = false;
      var to = input.getBoundingClientRect();
      var endHeight = root.offsetHeight;

      var moving = document.createElement("span");
      moving.className = "connect-rule";
      place(moving, origin, from, thickFrom);
      moving.style.height = thickFrom + "px";
      root.style.height = startHeight + "px";
      root.appendChild(moving);
      /* The block clips while it grows, so focus must not scroll the
         field into view inside it: that would shift the form mid-flight.
         The field stands where the button just was, so it is in view. */
      input.focus({ preventScroll: true });

      /* Commit the starting frame, then set the destination; the
         stylesheet's transitions carry everything between. */
      void moving.offsetWidth;
      root.classList.add("connect-arriving");
      place(moving, origin, to, thickTo);
      moving.style.height = thickTo + "px";
      root.style.height = endHeight + "px";

      var settled = false;
      function settle() {
        if (settled) return;
        settled = true;
        moving.remove();
        idle.hidden = true;
        root.style.height = "";
        /* The field's own rule must appear in the same frame the moving
           one leaves, not ease in over the field's usual transition. */
        input.style.transition = "none";
        root.classList.remove("connect-live", "connect-arriving");
        void input.offsetWidth;
        input.style.transition = "";
      }
      moving.addEventListener("transitionend", function (ev) {
        if (ev.target === moving) settle();
      });
      setTimeout(settle, SETTLE_AFTER);
    });
  }

  Array.prototype.forEach.call(document.querySelectorAll("[data-connect]"), wire);
})();
