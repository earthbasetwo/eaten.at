/* Photos in the editor (plan 07, D37 amended). The tiles the page came
   with become live: the empty box and the add tile open the file
   picker, a tile opens its detail with a caption, hovering one reveals
   its remove mark, and dragging one reorders them. A file is uploaded
   to the author's repository the moment it is picked and comes back as
   a blob reference; the list rides in the form as hidden fields, so the
   photos are written with the record on Publish or Save, before or
   after the write-up exists, and nothing else here touches the record.
   Without this the tiles link to the photos page, once there is one. */
(function () {
  "use strict";
  var form = document.querySelector("form.editor-write");
  var block = form && form.querySelector(".photos-block");
  var mount = block && block.querySelector("[data-upload]");
  var fields = block && block.querySelector(".photo-fields");
  if (!mount || !fields) return;
  var endpoint = mount.getAttribute("data-upload");
  var hint = block.querySelector(".photo-hint");

  var photos = [];
  Array.prototype.forEach.call(block.querySelectorAll(".photo-tile"), function (tile) {
    var img = tile.querySelector("img");
    photos.push({
      cid: tile.dataset.cid,
      mime: tile.dataset.mime || "",
      size: tile.dataset.size || "",
      width: tile.dataset.width || "",
      height: tile.dataset.height || "",
      alt: tile.dataset.alt || "",
      thumb: img ? img.getAttribute("src") : "",
      full: tile.dataset.full || ""
    });
  });

  var grid = document.createElement("div");
  grid.className = "photo-tiles";
  grid.setAttribute("role", "list");
  mount.replaceWith(grid);
  var picker = document.createElement("input");
  picker.type = "file";
  picker.accept = "image/*";
  picker.multiple = true;
  picker.hidden = true;
  picker.tabIndex = -1;
  block.appendChild(picker);
  var problems = document.createElement("p");
  problems.className = "field-error";
  problems.hidden = true;
  problems.setAttribute("role", "alert");
  grid.insertAdjacentElement("afterend", problems);
  if (!hint) {
    hint = document.createElement("p");
    hint.className = "hint photo-hint";
    problems.insertAdjacentElement("afterend", hint);
  }

  function el(tag, className) {
    var node = document.createElement(tag);
    if (className) node.className = className;
    return node;
  }
  function button(className, label) {
    var b = el("button", className);
    b.type = "button";
    if (label) b.setAttribute("aria-label", label);
    return b;
  }
  function cross(size) {
    var svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("width", size);
    svg.setAttribute("height", size);
    svg.setAttribute("viewBox", "0 0 10 10");
    svg.setAttribute("aria-hidden", "true");
    var path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", "M 1 1 L 9 9 M 9 1 L 1 9");
    path.setAttribute("stroke", "currentColor");
    path.setAttribute("stroke-width", "1.4");
    path.setAttribute("fill", "none");
    svg.appendChild(path);
    return svg;
  }
  function showProblems(list) {
    problems.textContent = list.join(" ");
    problems.hidden = list.length === 0;
  }

  /* The list as the form carries it: six hidden fields a photo, in
     the order shown, which is the order written. */
  function sync() {
    fields.textContent = "";
    photos.forEach(function (p, i) {
      [["cid", p.cid], ["mime", p.mime], ["size", p.size], ["alt", p.alt], ["width", p.width], ["height", p.height]]
        .forEach(function (pair) {
          var input = el("input");
          input.type = "hidden";
          input.name = "photo_" + pair[0] + "_" + i;
          input.value = pair[1] == null ? "" : String(pair[1]);
          fields.appendChild(input);
        });
    });
  }

  /* ---- uploading ---- */
  var busy = false;
  function upload(files) {
    if (busy) return;
    busy = true;
    grid.classList.add("busy");
    var data = new FormData();
    files.forEach(function (f) { data.append("photos", f, f.name); });
    fetch(endpoint, {
      method: "POST",
      body: data,
      credentials: "same-origin",
      headers: { Accept: "application/json" }
    })
      .then(function (r) {
        return r.json().then(function (body) { return { ok: r.ok, body: body }; }, function () { return { ok: false, body: {} }; });
      })
      .then(function (res) {
        var body = res.body || {};
        (body.photos || []).forEach(function (p) {
          photos.push({
            cid: p.cid, mime: p.mime || "", size: p.size == null ? "" : p.size,
            width: p.width == null ? "" : p.width, height: p.height == null ? "" : p.height,
            alt: p.alt || "", thumb: p.thumb, full: p.full
          });
        });
        var messages = (body.problems || []).slice();
        if (body.error) messages.push(body.error);
        if (!res.ok && messages.length === 0) messages.push("The photos could not be uploaded; try again in a moment.");
        showProblems(messages);
      }, function () {
        showProblems(["The photos could not be uploaded; try again in a moment."]);
      })
      .then(function () {
        busy = false;
        grid.classList.remove("busy");
        sync();
        render();
      });
  }
  function pick() {
    picker.value = "";
    picker.click();
  }
  picker.addEventListener("change", function () {
    var files = Array.prototype.filter.call(picker.files || [], function (f) {
      return !f.type || f.type.indexOf("image/") === 0;
    });
    if (files.length) upload(files);
  });

  /* ---- the grid ---- */
  var dragging = null;
  function indexOf(cid) {
    for (var i = 0; i < photos.length; i++) if (photos[i].cid === cid) return i;
    return -1;
  }
  function remove(cid) {
    var i = indexOf(cid);
    if (i < 0) return;
    photos.splice(i, 1);
    sync();
    render();
  }
  function render() {
    grid.textContent = "";
    if (photos.length === 0) {
      var empty = button("photo-empty", "Add a photo");
      var idle = el("span", "photo-empty-idle");
      idle.textContent = "Nothing to look at yet.";
      var hover = el("span", "photo-empty-hover");
      hover.setAttribute("aria-hidden", "true");
      hover.textContent = "add a photo";
      empty.appendChild(idle);
      empty.appendChild(hover);
      empty.addEventListener("click", pick);
      grid.appendChild(empty);
      grid.classList.add("empty");
      hint.hidden = true;
      return;
    }
    grid.classList.remove("empty");
    photos.forEach(function (photo, i) {
      var tile = el("figure", "photo-tile");
      tile.setAttribute("role", "listitem");
      tile.draggable = true;
      tile.dataset.cid = photo.cid;
      tile.tabIndex = 0;
      tile.setAttribute("aria-label", photo.alt ? photo.alt : "Photo " + (i + 1));
      var img = el("img");
      img.src = photo.thumb;
      img.alt = "";
      img.draggable = false;
      img.width = 400;
      img.height = 400;
      tile.appendChild(img);
      if (i === 0) {
        var badge = el("span", "cover-badge");
        badge.textContent = "Cover";
        tile.appendChild(badge);
      }
      var removeMark = button("tile-remove", "Remove this photo");
      removeMark.title = "Remove photo";
      removeMark.appendChild(cross(10));
      removeMark.addEventListener("click", function (e) {
        e.stopPropagation();
        remove(photo.cid);
      });
      tile.appendChild(removeMark);
      tile.addEventListener("click", function () {
        if (dragging !== null) return;
        openDetail(photo.cid);
      });
      tile.addEventListener("keydown", function (e) {
        if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openDetail(photo.cid); }
      });
      tile.addEventListener("dragstart", function (e) {
        e.dataTransfer.effectAllowed = "move";
        try { e.dataTransfer.setData("text/plain", photo.cid); } catch (err) { /* older engines */ }
        setTimeout(function () { dragging = photo.cid; tile.classList.add("dragging"); }, 0);
      });
      tile.addEventListener("dragenter", function () {
        if (dragging === null || dragging === photo.cid) return;
        var from = grid.querySelector("[data-cid=\"" + dragging + "\"]");
        if (!from) return;
        var tiles = Array.prototype.slice.call(grid.querySelectorAll(".photo-tile"));
        var a = tiles.indexOf(from), b = tiles.indexOf(tile);
        if (a < 0 || b < 0) return;
        grid.insertBefore(from, a < b ? tile.nextSibling : tile);
      });
      tile.addEventListener("dragover", function (e) { e.preventDefault(); });
      tile.addEventListener("drop", function (e) { e.preventDefault(); });
      tile.addEventListener("dragend", function () {
        tile.classList.remove("dragging");
        var order = Array.prototype.map.call(grid.querySelectorAll(".photo-tile"), function (t) {
          return photos[indexOf(t.dataset.cid)];
        }).filter(Boolean);
        setTimeout(function () { dragging = null; }, 150);
        if (order.length === photos.length) photos = order;
        sync();
        render();
      });
      grid.appendChild(tile);
    });
    var add = button("photo-add", "Add photos");
    add.title = "Add photos";
    add.textContent = "+";
    add.addEventListener("click", pick);
    add.addEventListener("dragover", function (e) { e.preventDefault(); });
    grid.appendChild(add);
    hint.hidden = false;
    hint.textContent = "Drag to reorder — the first photo is the cover.";
  }

  /* ---- one photo, large, with its caption ---- */
  var detail = null;
  function openDetail(cid) {
    var i = indexOf(cid);
    if (i < 0) return;
    var photo = photos[i];
    closeDetail();
    var scrim = el("div", "photo-scrim");
    var panel = el("div", "photo-detail paper");
    panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-modal", "true");
    panel.setAttribute("aria-label", "Photo");
    var img = el("img");
    img.src = photo.full;
    img.alt = photo.alt;
    var caption = el("input", "photo-caption");
    caption.type = "text";
    caption.placeholder = "add a caption…";
    caption.value = photo.alt;
    caption.maxLength = 1000;
    caption.setAttribute("aria-label", "Caption");
    var foot = el("p", "hint photo-detail-foot");
    var done = button("hint-action");
    done.textContent = "done";
    var removeIt = button("hint-action danger");
    removeIt.textContent = "remove this photo";
    foot.appendChild(done);
    foot.appendChild(removeIt);
    panel.appendChild(img);
    panel.appendChild(caption);
    panel.appendChild(foot);
    scrim.appendChild(panel);
    panel.addEventListener("click", function (e) { e.stopPropagation(); });
    scrim.addEventListener("click", closeDetail);
    done.addEventListener("click", closeDetail);
    removeIt.addEventListener("click", function () {
      closeDetail(true);
      remove(cid);
    });
    caption.addEventListener("keydown", function (e) {
      if (e.key === "Enter") { e.preventDefault(); closeDetail(); }
    });
    document.body.appendChild(scrim);
    document.body.classList.add("has-scrim");
    detail = { cid: cid, scrim: scrim, caption: caption, opener: document.activeElement };
    caption.focus();
  }
  /* Closing keeps the caption as typed. */
  function closeDetail(discard) {
    if (!detail) return;
    var d = detail;
    detail = null;
    d.scrim.remove();
    document.body.classList.remove("has-scrim");
    var i = indexOf(d.cid);
    if (!discard && i >= 0) {
      photos[i].alt = d.caption.value.trim();
      sync();
      render();
    }
    if (d.opener && d.opener.focus && form.contains(d.opener)) d.opener.focus();
  }
  document.addEventListener("keydown", function (e) {
    if (e.key === "Escape" && detail) { e.stopPropagation(); closeDetail(); }
  }, true);

  sync();
  render();
})();
