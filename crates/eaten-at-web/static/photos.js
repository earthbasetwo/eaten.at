/* Photos in the editor (plan 07, D37 amended). The tiles the page came
   with become live: the empty box and the add tile open the file
   picker, a tile opens its detail with a caption, hovering one reveals
   its remove mark, and dragging one reorders them. A file is uploaded
   to the author's repository the moment it is picked and comes back as
   a blob reference; the list rides in the form as hidden fields, so the
   photos are written with the record on Publish or Save, before or
   after the write-up exists, and nothing else here touches the record.
   Every change is announced to the form, so the draft kept on the
   device follows the photos, and restoring that draft hands the list
   back here. Without this the tiles link to the photos page, once
   there is one. */
(function () {
  "use strict";
  var form = document.querySelector("form.editor-write");
  var block = form && form.querySelector(".photos-block");
  var mount = block && block.querySelector("[data-upload]");
  var fields = block && block.querySelector(".photo-fields");
  if (!mount || !fields) return;
  var endpoint = mount.getAttribute("data-upload");
  var hint = block.querySelector(".photo-hint");

  /* Each photo on the page has a key of its own, so two tiles of the
     same blob (the same file picked twice, or two files that encode
     alike) are two photos: captioned, moved, and removed apart. */
  var photos = [];
  var keys = 0;
  function keyed(photo) {
    photo.key = "p" + (++keys);
    return photo;
  }
  Array.prototype.forEach.call(block.querySelectorAll(".photo-tile"), function (tile) {
    var img = tile.querySelector("img");
    photos.push(keyed({
      cid: tile.dataset.cid,
      mime: tile.dataset.mime || "",
      size: tile.dataset.size || "",
      width: tile.dataset.width || "",
      height: tile.dataset.height || "",
      alt: tile.dataset.alt || "",
      thumb: img ? img.getAttribute("src") : "",
      full: tile.dataset.full || ""
    }));
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
  function ownPhoto(cid, size) { return "/write/photo/" + encodeURIComponent(cid) + "?size=" + size; }
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
  /* A change the author made: rewrite the fields, redraw, and tell the
     form, so the draft on the device is saved with the photos in it. */
  function changed() {
    sync();
    render();
    waiting();
    form.dispatchEvent(new Event("change", { bubbles: true }));
  }

  /* ---- uploading ----
     One file a request, in the order picked: each is judged on its own
     and a big batch never meets the request's size cap. A file over
     the upload limit is named at once and never sent. */
  var maxBytes = Number(mount.getAttribute("data-max-bytes")) || 0;
  var maxPhotos = Number(mount.getAttribute("data-max-photos")) || 24;
  var queue = [];
  var busy = false;
  var batchProblems = [];
  function megabytes(bytes) { return Math.round(bytes / (1024 * 1024)); }
  function upload(files) {
    if (!busy && queue.length === 0) batchProblems = [];
    // As many as fit on the digest are sent; the rest are named by count.
    var room = maxPhotos - photos.length - queue.length - (busy ? 1 : 0);
    var full = room <= 0, left = 0;
    files.forEach(function (f) {
      if (maxBytes && f.size > maxBytes) batchProblems.push(f.name + " is over " + megabytes(maxBytes) + " MB.");
      else if (room <= 0) left++;
      else { queue.push(f); room--; }
    });
    if (left) {
      batchProblems.push("At most " + maxPhotos + " photos on a digest; " +
        (full ? "remove some first." : "the last " + left + (left === 1 ? " was" : " were") + " not added."));
    }
    showProblems(batchProblems);
    next();
  }
  function next() {
    if (!busy && queue.length) {
      busy = true;
      send(queue.shift()).then(function (messages) {
        busy = false;
        batchProblems = batchProblems.concat(messages);
        showProblems(batchProblems);
        changed();
        next();
      });
    }
    waiting();
  }

  /* Until the last upload answers, the list the form carries is not
     whole, so Publish (or Save changes) waits: the button is held, the
     line beside it and the one under the tiles say how many are on
     their way, and every submit but Delete's is held back. It lets go
     when the last one answers, taken or refused. More can be picked
     meanwhile; they join the queue. */
  var submit = form.querySelector(".editor-actions button[value=publish]");
  var wait = el("span", "hint upload-wait");
  wait.setAttribute("role", "status");
  if (submit) submit.insertAdjacentElement("afterend", wait);
  function pending() { return queue.length + (busy ? 1 : 0); }
  function waiting() {
    var n = pending();
    var text = n === 0 ? "" : n === 1 ? "Uploading a photo…" : "Uploading " + n + " photos…";
    wait.textContent = text;
    if (submit) submit.disabled = n > 0;
    grid.classList.toggle("busy", n > 0);
    if (n) {
      hint.hidden = false;
      hint.textContent = text;
    }
  }
  form.addEventListener("submit", function (e) {
    if (!pending() || (e.submitter && e.submitter.name === "delete_post")) return;
    e.preventDefault();
    e.stopImmediatePropagation();
  }, true);
  function send(file) {
    var data = new FormData();
    data.append("existing", String(photos.length));
    data.append("photos", file, file.name);
    var failed = file.name + " could not be uploaded; try again in a moment.";
    return fetch(endpoint, {
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
          photos.push(keyed({
            cid: p.cid, mime: p.mime || "", size: p.size == null ? "" : p.size,
            width: p.width == null ? "" : p.width, height: p.height == null ? "" : p.height,
            alt: p.alt || "", thumb: p.thumb, full: p.full
          }));
        });
        var messages = (body.problems || []).slice();
        if (body.error) messages.push(body.error);
        if (!res.ok && messages.length === 0) messages.push(failed);
        return messages;
      }, function () {
        return [failed];
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
  function indexOf(key) {
    for (var i = 0; i < photos.length; i++) if (photos[i].key === key) return i;
    return -1;
  }
  function remove(key) {
    var i = indexOf(key);
    if (i < 0) return;
    photos.splice(i, 1);
    changed();
  }
  function render() {
    grid.textContent = "";
    if (photos.length === 0) {
      var empty = button("photo-empty", "Add a photo");
      var idle = el("span", "photo-empty-idle");
      idle.textContent = "Add photos";
      var hover = el("span", "photo-empty-hover");
      hover.setAttribute("aria-hidden", "true");
      hover.textContent = "add a photo";
      empty.appendChild(idle);
      empty.appendChild(hover);
      empty.addEventListener("click", pick);
      grid.appendChild(empty);
      grid.classList.add("photos-empty");
      hint.hidden = true;
      return;
    }
    grid.classList.remove("photos-empty");
    photos.forEach(function (photo, i) {
      var tile = el("figure", "photo-tile");
      tile.setAttribute("role", "listitem");
      tile.draggable = true;
      tile.dataset.cid = photo.cid;
      tile.dataset.key = photo.key;
      tile.tabIndex = 0;
      tile.setAttribute("aria-label", photo.alt ? photo.alt : "Photo " + (i + 1));
      var img = el("img");
      img.src = photo.thumb;
      img.alt = "";
      img.draggable = false;
      img.width = 400;
      img.height = 400;
      tile.appendChild(img);
      var removeMark = button("tile-remove", "Remove this photo");
      removeMark.title = "Remove photo";
      removeMark.appendChild(cross(10));
      removeMark.addEventListener("click", function (e) {
        e.stopPropagation();
        remove(photo.key);
      });
      tile.appendChild(removeMark);
      tile.addEventListener("click", function () {
        if (dragging !== null) return;
        openDetail(photo.key);
      });
      tile.addEventListener("keydown", function (e) {
        if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openDetail(photo.key); }
      });
      tile.addEventListener("dragstart", function (e) {
        e.dataTransfer.effectAllowed = "move";
        try { e.dataTransfer.setData("text/plain", photo.cid); } catch (err) { /* older engines */ }
        setTimeout(function () { dragging = photo.key; tile.classList.add("dragging"); }, 0);
      });
      tile.addEventListener("dragenter", function () {
        if (dragging === null || dragging === photo.key) return;
        var from = grid.querySelector("[data-key=\"" + dragging + "\"]");
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
          return photos[indexOf(t.dataset.key)];
        }).filter(Boolean);
        setTimeout(function () { dragging = null; }, 150);
        if (order.length === photos.length) photos = order;
        changed();
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
  function openDetail(key) {
    var i = indexOf(key);
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
    caption.placeholder = "Describe this photo";
    caption.value = photo.alt;
    caption.maxLength = 1000;
    caption.setAttribute("aria-label", "Describe this photo");
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
      remove(key);
      var next = grid.querySelector(".photo-tile, .photo-add, .photo-empty");
      if (next) next.focus();
    });
    caption.addEventListener("keydown", function (e) {
      if (e.key === "Enter") { e.preventDefault(); closeDetail(); }
    });
    document.body.appendChild(scrim);
    document.body.classList.add("has-scrim");
    detail = { key: key, scrim: scrim, caption: caption };
    caption.focus();
  }
  /* Closing keeps the caption as typed. */
  function closeDetail(discard) {
    if (!detail) return;
    var d = detail;
    detail = null;
    d.scrim.remove();
    document.body.classList.remove("has-scrim");
    var i = indexOf(d.key);
    if (discard !== true && i >= 0) {
      photos[i].alt = d.caption.value.trim();
      changed();
    }
    // render() replaces the opener, so focus its replacement by key.
    var opener = grid.querySelector('[data-key="' + d.key + '"]') || grid.querySelector(".photo-add, .photo-empty");
    if (opener) opener.focus();
  }
  document.addEventListener("keydown", function (e) {
    if (!detail) return;
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); closeDetail(); }
    else if (e.key === "Tab") {
      var controls = detail.scrim.querySelectorAll("input, button");
      var first = controls[0], last = controls[controls.length - 1];
      var focused = document.activeElement;
      if (!detail.scrim.contains(focused) || (e.shiftKey && focused === first) || (!e.shiftKey && focused === last)) {
        e.preventDefault();
        (e.shiftKey ? last : first).focus();
      }
    }
  }, true);

  /* The draft kept on the device, restored: its photos replace the
     ones on the page, drawn from the author's own blobs. */
  form.addEventListener("draft-restore", function (e) {
    var data = e.detail || {};
    var list = [];
    for (var i = 0; Object.prototype.hasOwnProperty.call(data, "photo_cid_" + i); i++) {
      var cid = data["photo_cid_" + i];
      if (!cid) continue;
      list.push(keyed({
        cid: cid, mime: data["photo_mime_" + i] || "", size: data["photo_size_" + i] || "",
        width: data["photo_width_" + i] || "", height: data["photo_height_" + i] || "",
        alt: data["photo_alt_" + i] || "", thumb: ownPhoto(cid, "thumb"), full: ownPhoto(cid, "full")
      }));
    }
    closeDetail(true);
    photos = list;
    showProblems([]);
    changed();
  });

  sync();
  render();
})();
