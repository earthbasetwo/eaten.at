/* Connect (plan 09): each way in on the landing page links to the page
   that asks for the handle; with script a plain click swaps that field
   in where the button stood, caret in it. One rule draws the swap: a
   rule laid over the button's slides, grows and reddens to where the
   field's focus rule will be, then hands over to it, while the block
   eases to the form's height. The stylesheet names every size. */
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

      // Measured before anything changes; one origin serves both ends.
      var origin = root.getBoundingClientRect();
      var from = button.getBoundingClientRect();
      var startHeight = root.offsetHeight;
      var thickFrom = rule(root, "--connect-rule-from");
      var thickTo = rule(root, "--connect-rule-to");

      // The form in, measured before anything paints.
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
      // No scroll: the block clips while it grows, and the field is in view.
      input.focus({ preventScroll: true });

      // Commit the start; the transitions carry the rest.
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
        // The field's rule appears the frame the moving one leaves.
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
