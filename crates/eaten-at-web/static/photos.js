/* Photos in the editor (plan 07, D37 amended). A picked file is
   uploaded at once and comes back as a blob reference; the list rides
   in the form as hidden fields, written with the record on Publish or
   Save. Every change is told to the form, so the draft follows it.
   Without this the tiles link to the photos page. */
(function () {
  "use strict";
  var form = document.querySelector("form.editor-write");
  var block = form && form.querySelector(".photos-block");
  var mount = block && block.querySelector("[data-upload]");
  var fields = block && block.querySelector(".photo-fields");
  if (!mount || !fields) return;
  var endpoint = mount.getAttribute("data-upload");
  var hint = block.querySelector(".photo-hint");

  /* Each photo has a key of its own, so two tiles of one blob are two
     photos: captioned, moved, and removed apart. */
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
  // Read aloud: a tile's keys, where a moved photo landed.
  var keyHelp = el("span", "visually-hidden");
  keyHelp.id = "photo-keys";
  keyHelp.textContent = "Enter opens it; Alt and an arrow key move it.";
  var moved = el("span", "visually-hidden");
  moved.setAttribute("role", "status");
  block.appendChild(keyHelp);
  block.appendChild(moved);
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
  var CROSS = '<svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1 9 9M9 1 1 9" stroke="currentColor" stroke-width="1.4" fill="none"/></svg>';
  function showProblems(list) {
    problems.textContent = list.join("\n");
    problems.hidden = list.length === 0;
  }

  // The list as the form carries it, six fields a photo, in order.
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
  // A change: the fields, the tiles, and the form (so the draft).
  function tell() { form.dispatchEvent(new Event("change", { bubbles: true })); }
  function changed() {
    sync();
    render();
    waiting();
    tell();
  }

  /* ---- uploading: one file a request, in the order picked ---- */
  var maxBytes = Number(mount.getAttribute("data-max-bytes")) || 0;
  var maxPhotos = Number(mount.getAttribute("data-max-photos")) || 24;
  var queue = [];
  var busy = false;
  var batchProblems = [];
  function megabytes(bytes) { return Math.round(bytes / (1024 * 1024)); }
  // Sent in turn while there is room: a refused file takes none.
  var left = 0;
  function upload(files) {
    if (!busy && queue.length === 0) batchProblems = [];
    var full = photos.length >= maxPhotos;
    files.forEach(function (f) {
      if (f.size === 0) batchProblems.push(f.name + " is empty.");
      else if (f.type && f.type.indexOf("image/") !== 0) batchProblems.push(f.name + " isn't an image we can use. JPEG, PNG, GIF, or WebP, please.");
      else if (maxBytes && f.size > maxBytes) batchProblems.push(f.name + " is over " + megabytes(maxBytes) + " MB.");
      else if (full) left = -1;
      else queue.push(f);
    });
    if (left < 0) {
      batchProblems.push("At most " + maxPhotos + " photos on a digest; remove some first.");
      left = 0;
    }
    showProblems(batchProblems);
    next();
  }
  // A restored draft replaces the list: an answer for an earlier
  // `round` is dropped, and room is counted again as each lands.
  var round = 0;
  function next() {
    if (!busy && photos.length >= maxPhotos) { left += queue.length; queue = []; }
    if (!busy && left && !queue.length) {
      batchProblems.push("At most " + maxPhotos + " photos on a digest; the last " + left + (left === 1 ? " was" : " were") + " not added.");
      left = 0;
      showProblems(batchProblems);
    }
    if (!busy && queue.length) {
      busy = true;
      var mine = round, file = queue.shift();
      send(file).then(function (got) {
        if (mine !== round) return;
        busy = false;
        // Signed out, the rest would fare no better: they are named.
        if (got[2] === 401) {
          got[1].push("Not added: " + [file].concat(queue).map(function (f) { return f.name; }).join(", ") + ".");
          queue = [];
        }
        got[0].forEach(function (p) {
          if (photos.length < maxPhotos) photos.push(keyed(p));
          else got[1].push("At most " + maxPhotos + " photos on a digest; " + file.name + " was not added.");
        });
        batchProblems = batchProblems.concat(got[1]);
        showProblems(batchProblems);
        changed();
        next();
      });
    }
    waiting();
  }

  // Publish, and every submit but Delete's, waits for the uploads.
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
    // The first photo's progress is written in the add box itself; with
    // tiles up it goes on the line under them.
    var empty = grid.querySelector(".photo-empty");
    if (empty) {
      empty.querySelector(".photo-empty-idle").textContent = n ? text : "Add photos";
      empty.querySelector(".photo-empty-hover").textContent = n ? text : "add a photo";
    } else if (n) {
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
        return r.json().then(function (body) { return { ok: r.ok, status: r.status, body: body }; }, function () { return { ok: false, body: {} }; });
      })
      .then(function (res) {
        var body = res.body || {};
        var messages = (body.problems || []).slice();
        if (body.error) messages.push(body.error);
        if (!res.ok && messages.length === 0) messages.push(failed);
        return [(body.photos || []).map(function (p) {
          return {
            cid: p.cid, mime: p.mime || "", size: p.size == null ? "" : p.size,
            width: p.width == null ? "" : p.width, height: p.height == null ? "" : p.height,
            alt: p.alt || "", thumb: p.thumb, full: p.full
          };
        }), messages, res.status];
      }, function () {
        return [[], [failed]];
      });
  }
  function pick() {
    picker.value = "";
    picker.click();
  }
  picker.addEventListener("change", function () {
    var files = Array.prototype.slice.call(picker.files || []);
    if (files.length) upload(files);
  });

  /* ---- the grid ---- */
  var dragging = null;
  function indexOf(key) {
    for (var i = 0; i < photos.length; i++) if (photos[i].key === key) return i;
    return -1;
  }
  // Alt+arrow: one place along, focus kept.
  function move(key, by) {
    var i = indexOf(key), j = i + by;
    if (i < 0 || j < 0 || j >= photos.length) return;
    photos.splice(j, 0, photos.splice(i, 1)[0]);
    changed();
    grid.querySelector("[data-key=\"" + key + "\"]").focus();
    moved.textContent = "Photo " + (j + 1) + " of " + photos.length + (j === 0 ? ", the cover." : ".");
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
      removeMark.innerHTML = CROSS;
      removeMark.addEventListener("click", function (e) {
        e.stopPropagation();
        // Focus goes to the photo in its place, else to adding one.
        remove(photo.key);
        var next = grid.querySelectorAll(".photo-tile")[i] || grid.querySelector(".photo-add, .photo-empty");
        if (next) next.focus();
      });
      tile.appendChild(removeMark);
      tile.addEventListener("click", function () {
        if (dragging !== null) return;
        openDetail(photo.key);
      });
      tile.setAttribute("aria-describedby", keyHelp.id);
      tile.addEventListener("keydown", function (e) {
        if (e.target !== tile) return;
        if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openDetail(photo.key); }
        else if (e.altKey && /^Arrow(Left|Right|Up|Down)$/.test(e.key)) {
          e.preventDefault();
          move(photo.key, e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 1);
        }
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
    // The lightbox is outside the form: the caption reaches the fields,
    // and the draft, as it is typed.
    caption.addEventListener("input", function () {
      var i = indexOf(key);
      if (i < 0) return;
      photos[i].alt = caption.value.trim();
      sync();
      tell();
    });
    document.body.appendChild(scrim);
    document.body.classList.add("has-scrim");
    detail = { key: key, scrim: scrim, caption: caption };
    caption.focus();
  }
  // Closing keeps the caption.
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
    // The opener was redrawn: focus its new self.
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

  // A restored draft's photos replace the page's.
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
    round++;
    queue = [];
    left = 0;
    busy = false;
    showProblems([]);
    changed();
  });

  sync();
  render();
})();
