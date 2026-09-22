(function () {
  "use strict";
  var panel = document.querySelector(".publish-confirmation");
  if (!panel) return;
  try {
    var key = "ea:draft:" + panel.dataset.draftPath;
    var draft = JSON.parse(localStorage.getItem(key));
    if (draft && String(draft.at) === panel.dataset.draftId) localStorage.removeItem(key);
  } catch (_) {}
  var url = new URL(location.href);
  url.searchParams.delete("after");
  url.searchParams.delete("draft");
  history.replaceState(history.state, "", url);
  if (!navigator.clipboard) return;
  var button = panel.querySelector(".copy-permalink");
  var link = panel.querySelector(".permalink");
  button.hidden = false;
  link.hidden = true;
  button.addEventListener("click", async function () {
    var status = panel.querySelector(".copy-status");
    try {
      await navigator.clipboard.writeText(link.href);
      status.textContent = "Link copied.";
    } catch (_) {
      link.hidden = false;
      status.textContent = "Copy the permalink from the link.";
    }
  });
}());
