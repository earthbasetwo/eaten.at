/* The digest: a live markdown editor over the write-up's textarea. The
   text is shown formatted, one line of source per line on the page;
   the line the caret is on shows its raw markdown, the syntax in faint
   ink around the formatting it makes, and folds back when the caret
   leaves. Headings to three levels, bullet and numbered lists, quotes,
   bold, italic, code, and links are drawn; anything else stays as
   written and the server renders it. The textarea stays the carrier:
   every change is written back to it, so the draft island and the send
   see the same text, and a restored draft comes back through it.
   Without this the textarea is the editor. */
(function () {
  "use strict";
  var ta = document.querySelector("form.editor-write textarea[name=\"body\"]");
  if (!ta || !ta.form) return;

  var box = document.createElement("div");
  box.className = "digest-editor";
  box.setAttribute("role", "textbox");
  box.setAttribute("aria-multiline", "true");
  box.setAttribute("aria-labelledby", "body-label");
  box.tabIndex = 0;
  box.dataset.placeholder = ta.placeholder;
  box.setAttribute("aria-placeholder", ta.placeholder);
  ta.hidden = true;
  ta.parentNode.insertBefore(box, ta.nextSibling);

  var lines = split(ta.value);
  var active = null;
  var caret = 0;
  var composing = false;
  var undo = [], redo = [], lastEdit = 0, lastLine = null;

  function split(text) { return text.replace(/\r\n?/g, "\n").split("\n"); }

  /* ---- the markup of a line ---- */
  function block(text) {
    var m;
    if ((m = /^#{1,3} /.exec(text))) return { type: "h" + (m[0].length - 1), prefix: m[0], shown: 0 };
    if ((m = /^[-*] /.exec(text))) return { type: "li", prefix: m[0], shown: 2 };
    if ((m = /^\d+\. /.exec(text))) return { type: "ol", prefix: m[0], shown: m[0].length };
    if ((m = /^> /.exec(text))) return { type: "q", prefix: m[0], shown: 0 };
    return { type: "p", prefix: "", shown: 0 };
  }
  var INLINE = /\*\*([^*\n]+)\*\*|\*([^*\n]+)\*|_([^_\n]+)_|`([^`\n]+)`|\[([^\]\n]*)\]\(([^)\s]*)\)/g;
  function segments(text) {
    var out = [], last = 0, m;
    INLINE.lastIndex = 0;
    while ((m = INLINE.exec(text))) {
      if (m.index > last) out.push({ t: "plain", raw: text.slice(last, m.index), inner: text.slice(last, m.index), lead: 0 });
      if (m[1] != null) out.push({ t: "strong", raw: m[0], inner: m[1], lead: 2, mark: "**" });
      else if (m[2] != null) out.push({ t: "em", raw: m[0], inner: m[2], lead: 1, mark: "*" });
      else if (m[3] != null) out.push({ t: "em", raw: m[0], inner: m[3], lead: 1, mark: "_" });
      else if (m[4] != null) out.push({ t: "code", raw: m[0], inner: m[4], lead: 1, mark: "`" });
      else out.push({ t: "link", raw: m[0], inner: m[5], lead: 1, url: m[6] });
      last = m.index + m[0].length;
    }
    if (last < text.length) out.push({ t: "plain", raw: text.slice(last), inner: text.slice(last), lead: 0 });
    return out;
  }
  function esc(s) { return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;"); }
  function tok(s) { return "<span class=\"md-tok\">" + esc(s) + "</span>"; }
  function lineHtml(text, isActive) {
    var b = block(text), html = "";
    if (b.prefix) {
      if (isActive) html += tok(b.prefix);
      else if (b.type === "li") html += "<span class=\"md-bullet\">• </span>";
      else if (b.type === "ol") html += "<span class=\"md-bullet\">" + esc(b.prefix) + "</span>";
    }
    segments(text.slice(b.prefix.length)).forEach(function (g) {
      if (g.t === "plain") { html += esc(g.raw); return; }
      var open = isActive ? tok(g.t === "link" ? "[" : g.mark) : "";
      var close = isActive ? tok(g.t === "link" ? "](" + g.url + ")" : g.mark) : "";
      var tag = g.t === "strong" ? "strong" : g.t === "em" ? "em" : g.t === "code" ? "code class=\"md-code\"" : "span class=\"md-link\"";
      html += open + "<" + tag + ">" + esc(g.inner) + "</" + tag.split(" ")[0] + ">" + close;
    });
    return html || "<br>";
  }
  function lineClass(text, i) {
    var b = block(text);
    var c = "md-line md-" + b.type;
    if (i > 0 && (b.type === "h1" || b.type === "h2")) c += " md-after";
    return c;
  }
  function render() {
    box.textContent = "";
    lines.forEach(function (text, i) {
      var d = document.createElement("div");
      d.dataset.md = i;
      d.className = lineClass(text, i);
      d.innerHTML = lineHtml(text, i === active);
      if (i === active) {
        d.contentEditable = "true";
        d.tabIndex = -1;
        d.classList.add("md-active");
      }
      box.appendChild(d);
    });
    box.classList.toggle("digest-empty", lines.join("\n").trim() === "");
    box.classList.toggle("active", active !== null);
    if (active !== null) {
      var el = box.children[active];
      el.focus({ preventScroll: true });
      setCaret(el, caret);
    }
  }
  function sync() {
    box.classList.toggle("digest-empty", lines.join("\n").trim() === "");
    ta.value = lines.join("\n");
    ta.dispatchEvent(new Event("input", { bubbles: true }));
  }

  /* ---- the caret: as an offset into the line's text ---- */
  function caretOffset(el) {
    var sel = window.getSelection();
    if (!sel || !sel.rangeCount) return 0;
    var r = sel.getRangeAt(0);
    var pre = document.createRange();
    pre.selectNodeContents(el);
    pre.setEnd(r.endContainer, r.endOffset);
    return pre.toString().length;
  }
  function setCaret(el, offset) {
    var sel = window.getSelection();
    if (!sel) return;
    var r = document.createRange();
    var walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    var node, seen = 0, placed = false;
    while ((node = walker.nextNode())) {
      if (seen + node.length >= offset) {
        r.setStart(node, Math.max(0, offset - seen));
        placed = true;
        break;
      }
      seen += node.length;
    }
    if (placed) r.collapse(true);
    else { r.selectNodeContents(el); r.collapse(false); }
    sel.removeAllRanges();
    sel.addRange(r);
  }
  function offsetAtPoint(el, x, y) {
    var node = null, off = 0;
    if (document.caretPositionFromPoint) {
      var p = document.caretPositionFromPoint(x, y);
      if (p) { node = p.offsetNode; off = p.offset; }
    } else if (document.caretRangeFromPoint) {
      var q = document.caretRangeFromPoint(x, y);
      if (q) { node = q.startContainer; off = q.startOffset; }
    }
    if (!node || !el.contains(node)) return null;
    var pre = document.createRange();
    pre.selectNodeContents(el);
    pre.setEnd(node, off);
    return pre.toString().length;
  }
  /* A folded line shows less than its source; a click on it lands the
     caret at the matching place in the source. */
  function shownToRaw(text, shown) {
    var b = block(text);
    var raw = b.prefix.length, seen = 0;
    if (b.shown) {
      if (shown <= b.shown) return raw;
      seen = b.shown;
    }
    var segs = segments(text.slice(b.prefix.length));
    for (var i = 0; i < segs.length; i++) {
      var g = segs[i];
      if (shown <= seen + g.inner.length) return raw + g.lead + (shown - seen);
      seen += g.inner.length;
      raw += g.raw.length;
    }
    return text.length;
  }

  /* Selection spans the digest, even though only the active line shows
     source. Map folded DOM offsets back to markdown for the clipboard
     and replacement; never unfold the other lines just to select them. */
  function endpoint(node, offset) {
    if (node === box) {
      return offset >= lines.length ? { line: lines.length - 1, offset: lines[lines.length - 1].length }
        : { line: offset, offset: 0 };
    }
    var el = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
    var line = el && el.closest("[data-md]");
    if (!line || !box.contains(line)) return null;
    var i = Number(line.dataset.md);
    var r = document.createRange();
    r.selectNodeContents(line);
    r.setEnd(node, offset);
    var shown = r.toString().length;
    return { line: i, offset: i === active ? shown : shownToRaw(lines[i], shown) };
  }
  function selection() {
    var sel = window.getSelection();
    if (!sel || !sel.rangeCount || sel.isCollapsed) return null;
    var r = sel.getRangeAt(0);
    var start = endpoint(r.startContainer, r.startOffset);
    var end = endpoint(r.endContainer, r.endOffset);
    return start && end ? { start: start, end: end } : null;
  }
  function selectedText(range) {
    var part = lines.slice(range.start.line, range.end.line + 1);
    part[part.length - 1] = part[part.length - 1].slice(0, range.end.offset);
    part[0] = part[0].slice(range.start.offset);
    return part.join("\n");
  }
  function replaceSelection(text, range) {
    remember(true);
    var start = range.start, end = range.end;
    var left = lines[start.line].slice(0, start.offset);
    var right = lines[end.line].slice(end.offset);
    var pieces = split(text);
    var lastLength = pieces[pieces.length - 1].length;
    pieces[0] = left + pieces[0];
    pieces[pieces.length - 1] += right;
    lines.splice.apply(lines, [start.line, end.line - start.line + 1].concat(pieces));
    activate(start.line + pieces.length - 1, (pieces.length === 1 ? left.length : 0) + lastLength);
    lastLine = null;
    sync();
  }
  ["copy", "cut"].forEach(function (type) {
    box.addEventListener(type, function (e) {
      var range = selection();
      if (!range || !e.clipboardData) return;
      e.preventDefault();
      e.clipboardData.setData("text/plain", selectedText(range));
      if (type === "cut") replaceSelection("", range);
    });
  });
  box.addEventListener("beforeinput", function (e) {
    var range = selection();
    if (!range || !e.cancelable) return;
    if (e.inputType.indexOf("delete") === 0) {
      e.preventDefault(); replaceSelection("", range);
    } else if (e.inputType === "insertText" || e.inputType === "insertReplacementText") {
      e.preventDefault(); replaceSelection(e.data || "", range);
    }
  });

  /* ---- history ---- */
  function remember(force) {
    var now = Date.now();
    if (!force && active === lastLine && now - lastEdit < 600) { lastEdit = now; return; }
    undo.push({ lines: lines.slice(), active: active, caret: caret });
    if (undo.length > 200) undo.shift();
    redo = [];
    lastEdit = now;
    lastLine = active;
  }
  function restore(from, to) {
    var snap = from.pop();
    if (!snap) return;
    to.push({ lines: lines.slice(), active: active, caret: caret });
    lines = snap.lines.slice();
    active = snap.active === null ? null : Math.min(snap.active, lines.length - 1);
    caret = snap.caret;
    lastLine = null;
    render();
    sync();
  }

  /* ---- editing ---- */
  function activate(i, offset) {
    active = Math.max(0, Math.min(i, lines.length - 1));
    caret = Math.max(0, Math.min(offset, lines[active].length));
    render();
  }
  var leaving = false;
  function deactivate() {
    if (active === null) return;
    active = null;
    lastLine = null;
    render();
    var sel = window.getSelection();
    if (sel) sel.removeAllRanges();
  }

  box.addEventListener("mousedown", function (e) {
    if (e.button !== 0) return;
    var line = e.target.closest && e.target.closest("[data-md]");
    if (line && Number(line.dataset.md) === active) return;
    e.preventDefault();
    var i, offset;
    if (line) {
      i = Number(line.dataset.md);
      var shown = offsetAtPoint(line, e.clientX, e.clientY);
      offset = shown == null ? lines[i].length : shownToRaw(lines[i], shown);
    } else {
      i = lines.length - 1;
      offset = lines[i].length;
    }
    activate(i, offset);
  });
  /* A click that no mousedown announced (a script's, or assistive
     technology's) opens the line at its end. */
  box.addEventListener("click", function (e) {
    var line = e.target.closest && e.target.closest("[data-md]");
    if (line && Number(line.dataset.md) !== active) activate(Number(line.dataset.md), lines[Number(line.dataset.md)].length);
    else if (!line && active === null) activate(lines.length - 1, lines[lines.length - 1].length);
  });
  /* Reached by keyboard, the editor opens at its first line; leaving it
     folds the line that was open. */
  box.addEventListener("focus", function () {
    if (leaving) { leaving = false; return; }
    if (active === null) activate(0, 0);
  });
  box.addEventListener("focusout", function () {
    setTimeout(function () {
      var focus = document.activeElement;
      if (active !== null && !box.contains(focus)) deactivate();
    }, 0);
  });

  box.addEventListener("compositionstart", function () {
    var range = selection();
    if (range) replaceSelection("", range);
    composing = true;
  });
  box.addEventListener("compositionend", function () { composing = false; edited(); });
  box.addEventListener("input", function () { if (!composing) edited(); });
  function edited() {
    if (active === null) return;
    var el = box.children[active];
    remember(false);
    caret = caretOffset(el);
    var text = el.textContent.replace(/\n/g, " ");
    lines[active] = text;
    el.className = lineClass(text, active) + " md-active";
    el.innerHTML = lineHtml(text, true);
    setCaret(el, caret);
    sync();
  }

  box.addEventListener("paste", function (e) {
    if (active === null) return;
    e.preventDefault();
    var text = (e.clipboardData || window.clipboardData).getData("text/plain");
    if (!text) return;
    var range = selection();
    if (range) { replaceSelection(text, range); return; }
    remember(true);
    var el = box.children[active];
    var off = caretOffset(el);
    var current = lines[active];
    var pieces = split(text);
    var left = current.slice(0, off), right = current.slice(off);
    var inserted = pieces.map(function (piece, k) {
      var line = piece;
      if (k === 0) line = left + line;
      if (k === pieces.length - 1) line = line + right;
      return line;
    });
    lines.splice.apply(lines, [active, 1].concat(inserted));
    var lastPiece = pieces[pieces.length - 1];
    var end = (pieces.length === 1 ? left.length : 0) + lastPiece.length;
    activate(active + pieces.length - 1, end);
    sync();
  });

  box.addEventListener("keydown", function (e) {
    if (active === null || composing || e.isComposing || e.keyCode === 229) return;
    var i = active, el = box.children[i], text = lines[i];
    var meta = e.metaKey || e.ctrlKey;
    if (meta && e.key.toLowerCase() === "a") {
      e.preventDefault();
      var all = document.createRange();
      all.selectNodeContents(box);
      var whole = window.getSelection();
      whole.removeAllRanges();
      whole.addRange(all);
      return;
    }
    if (meta && (e.key === "z" || e.key === "Z")) {
      e.preventDefault();
      if (e.shiftKey) restore(redo, undo); else restore(undo, redo);
      return;
    }
    if (meta && e.key === "y") { e.preventDefault(); restore(redo, undo); return; }
    var selected = selection();
    // A range spanning folded lines crosses editing hosts. Collapse it
    // ourselves: native arrow navigation can leave the whole range selected.
    if (selected && !e.shiftKey && /^(ArrowLeft|ArrowRight|ArrowUp|ArrowDown|Home|End)$/.test(e.key)) {
      e.preventDefault();
      var start = /^(ArrowLeft|ArrowUp|Home)$/.test(e.key);
      var edge = start ? selected.start : selected.end;
      var offset = e.key === "Home" ? 0 : e.key === "End" ? lines[edge.line].length : edge.offset;
      activate(edge.line, offset);
      return;
    }
    if (selected && !meta && !e.altKey) {
      if (e.key === "Backspace" || e.key === "Delete" || e.key === "Enter" || e.key.length === 1) {
        e.preventDefault();
        replaceSelection(e.key === "Enter" ? "\n" : e.key.length === 1 ? e.key : "", selected);
        return;
      }
    }
    if (e.key === "Escape") {
      e.preventDefault();
      deactivate();
      leaving = true;
      box.focus();
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      remember(true);
      var off = caretOffset(el);
      var b = block(text);
      var left = text.slice(0, off), right = text.slice(off);
      var listy = b.type === "li" || b.type === "ol" || b.type === "q";
      /* Return on an empty list or quote line ends the list. */
      if (listy && left === b.prefix && !right) {
        lines[i] = "";
        activate(i, 0);
        sync();
        return;
      }
      var next = 0;
      if (listy && off >= b.prefix.length) {
        var prefix = b.type === "ol" ? (Number(b.prefix) + 1) + ". " : b.prefix;
        right = prefix + right;
        next = prefix.length;
      }
      lines.splice(i, 1, left, right);
      activate(i + 1, next);
      sync();
      return;
    }
    var sel = window.getSelection();
    var collapsed = sel && sel.isCollapsed;
    if (e.key === "Backspace" && collapsed && caretOffset(el) === 0 && i > 0) {
      e.preventDefault();
      remember(true);
      var joinAt = lines[i - 1].length;
      lines.splice(i - 1, 2, lines[i - 1] + text);
      activate(i - 1, joinAt);
      sync();
      return;
    }
    if (e.key === "Delete" && collapsed && caretOffset(el) === text.length && i + 1 < lines.length) {
      e.preventDefault();
      remember(true);
      lines.splice(i, 2, text + lines[i + 1]);
      activate(i, text.length);
      sync();
      return;
    }
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      var up = e.key === "ArrowUp";
      if (!sel || !sel.rangeCount) return;
      var r = sel.getRangeAt(0).cloneRange();
      r.collapse(false);
      var rect = r.getClientRects()[0];
      var elRect = el.getBoundingClientRect();
      if (!rect) rect = elRect;
      var lh = parseFloat(getComputedStyle(el).lineHeight) || 26;
      var onEdge = up ? rect.top - elRect.top < lh * 0.6 : elRect.bottom - rect.bottom < lh * 0.6;
      if (!onEdge) return;
      var ni = up ? i - 1 : i + 1;
      if (ni < 0 || ni >= lines.length) return;
      e.preventDefault();
      activate(ni, up ? lines[ni].length : 0);
      var nd = box.children[ni], nr = nd.getBoundingClientRect();
      var y = up ? nr.bottom - lh / 2 : nr.top + lh / 2;
      var shown = offsetAtPoint(nd, rect.left, y);
      if (shown != null) {
        caret = Math.min(shown, lines[ni].length);
        setCaret(nd, caret);
      }
    }
  });

  /* A restored draft, or anything else that sets the textarea, is read
     back into the lines. */
  ta.addEventListener("change", function () {
    lines = split(ta.value);
    active = null;
    undo = [];
    redo = [];
    render();
  });

  render();
})();
