/* Connect (plan 09): the landing page's one primary action is a link
   to the sign-in page. With script, pressing it swaps the handle field
   in where the button stood and puts the caret in it, so signing in
   starts on the page the reader is already on. A plain click is the
   only one taken; a modified click or a middle click still opens the
   sign-in page as the link says. */
(function () {
  "use strict";
  var root = document.querySelector("[data-connect]");
  if (!root) return;
  var idle = root.querySelector(".connect-idle");
  var button = root.querySelector(".connect-button");
  var form = root.querySelector(".connect-form");
  var input = form && form.querySelector("input[name=handle]");
  if (!idle || !button || !form || !input) return;
  button.addEventListener("click", function (e) {
    if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    idle.hidden = true;
    form.hidden = false;
    input.focus();
  });
})();
