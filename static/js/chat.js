// HalisAI - client chat voi streaming SSE
(function () {
  "use strict";

  var composer, input, sendBtn, stopBtn, messagesEl, titleEl, regenBtn, renameBtn;
  var controller = null;
  var sending = false;

  document.addEventListener("DOMContentLoaded", init);

  function init() {
    composer = document.getElementById("composer");
    input = document.getElementById("input");
    sendBtn = document.getElementById("send-btn");
    stopBtn = document.getElementById("stop-btn");
    messagesEl = document.getElementById("messages");
    titleEl = document.getElementById("conv-title");
    regenBtn = document.getElementById("regen-btn");
    renameBtn = document.getElementById("rename-btn");

    if (!composer) return;

    scrollToBottom(true);

    composer.addEventListener("submit", function (e) {
      e.preventDefault();
      send();
    });
    input.addEventListener("keydown", function (e) {
      var coarse = window.matchMedia("(pointer: coarse)").matches;
      if (e.key === "Enter" && !e.shiftKey && !coarse && !e.isComposing) {
        e.preventDefault();
        send();
      }
    });
    input.addEventListener("input", autoresize);
    stopBtn.addEventListener("click", stop);
    if (regenBtn) regenBtn.addEventListener("click", regenerate);
    if (renameBtn) renameBtn.addEventListener("click", rename);

    Array.prototype.forEach.call(document.querySelectorAll(".chip"), function (chip) {
      chip.addEventListener("click", function () {
        input.value = chip.getAttribute("data-fill") || "";
        input.focus();
        autoresize();
      });
    });

    // Xoa empty-state khi nguoi dung bat dau go
    input.addEventListener("focus", removeEmptyOnce);
    autoresize();
  }

  function removeEmptyOnce() {
    var empty = messagesEl && messagesEl.querySelector(".empty-state");
    if (empty) empty.remove();
    input.removeEventListener("focus", removeEmptyOnce);
  }

  function csrf() {
    var m = document.querySelector('meta[name="csrf-token"]');
    return m ? m.getAttribute("content") : "";
  }

  function jsonHeaders() {
    return { "Content-Type": "application/json", "X-CSRF-Token": csrf() };
  }

  function convId() {
    return (input && input.dataset.conv || "").trim() || null;
  }

  function autoresize() {
    input.style.height = "auto";
    input.style.height = Math.min(input.scrollHeight, 180) + "px";
  }

  function nearBottom() {
    if (!messagesEl) return true;
    return messagesEl.scrollHeight - messagesEl.scrollTop - messagesEl.clientHeight < 140;
  }

  function maybeScroll() {
    if (nearBottom()) messagesEl.scrollTop = messagesEl.scrollHeight;
  }

  function scrollToBottom(force) {
    if (messagesEl && (force || nearBottom())) {
      messagesEl.scrollTop = messagesEl.scrollHeight;
    }
  }

  function escapeText(s) {
    var d = document.createElement("div");
    d.textContent = s;
    return d.innerHTML;
  }

  function truncate(s, n) {
    return s.length > n ? s.slice(0, n).trimEnd() + "…" : s;
  }

  // ============== Tao phan tu tin nhan ==============

  function appendUserMsg(content) {
    removeEmptyOnce();
    var el = document.createElement("article");
    el.className = "msg msg-user";
    el.innerHTML =
      '<div class="msg-label"><span class="msg-author author-user">Bạn</span><span class="msg-time">vừa xong</span></div>' +
      '<div class="msg-body"></div>';
    el.querySelector(".msg-body").textContent = content;
    messagesEl.appendChild(el);
    scrollToBottom(true);
  }

  function appendAssistantMsg() {
    removeEmptyOnce();
    var el = document.createElement("article");
    el.className = "msg msg-assistant";
    el.innerHTML =
      '<div class="msg-label"><span class="msg-author author-ai">Halis</span><span class="msg-time">vừa xong</span></div>' +
      '<div class="ai-status"><span class="cursor">▊</span><span class="dots">Halis đang suy nghĩ</span></div>' +
      '<div class="msg-body"></div>';
    messagesEl.appendChild(el);
    scrollToBottom(true);
    return el;
  }

  function buildThinking(ai) {
    var details = document.createElement("details");
    details.className = "thinking";
    details.open = true;
    details.innerHTML =
      '<summary><span class="think-toggle" aria-hidden="true">' +
      '<svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6"/></svg>' +
      '</span><span class="think-label">Đang suy nghĩ</span></summary>' +
      '<div class="think-body"></div>';
    var bodyEl = ai.querySelector(".msg-body");
    bodyEl.parentNode.insertBefore(details, bodyEl);
    return {
      root: details,
      body: details.querySelector(".think-body"),
      label: details.querySelector(".think-label"),
    };
  }

  function ensureToolsBox(ai) {
    var box = ai.querySelector(".tools-box");
    if (!box) {
      box = document.createElement("div");
      box.className = "tools-box";
      var bodyEl = ai.querySelector(".msg-body");
      bodyEl.parentNode.insertBefore(box, bodyEl);
    }
    return box;
  }

  function handleToolLine(box, data) {
    if (data.status === "start") {
      var line = document.createElement("div");
      line.className = "tool-line running";
      line.dataset.tool = data.name;
      line.innerHTML =
        '<span class="tool-icon" aria-hidden="true">' +
        '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/></svg>' +
        "</span>" +
        '<span class="tool-name"></span><span class="tool-summary">đang chạy…</span><span class="tool-ms"></span>';
      line.querySelector(".tool-name").textContent = data.name;
      box.appendChild(line);
    } else {
      var run = box.querySelector('.tool-line[data-tool="' + cssEscape(data.name) + '"]:last-of-type.running') ||
        box.querySelector(".tool-line.running:last-of-type");
      if (!run) return;
      run.classList.remove("running");
      if (data.summary) run.querySelector(".tool-summary").textContent = data.summary;
      if (data.ms) run.querySelector(".tool-ms").textContent = data.ms + "ms";
    }
  }

  function cssEscape(s) {
    return (window.CSS && CSS.escape) ? CSS.escape(s) : s.replace(/[^a-zA-Z0-9_-]/g, "\\$&");
  }

  function appendNotice(ai, text, type) {
    var n = document.createElement("div");
    n.className = "notice " + (type === "error" ? "notice-error" : type === "info" ? "notice-info" : "notice-warn");
    n.textContent = text;
    ai.appendChild(n);
  }

  function hideStatus(ai) {
    var st = ai.querySelector(".ai-status");
    if (st) st.remove();
  }

  // ============== Gui / nhan stream ==============

  function setStreaming(on) {
    sending = on;
    sendBtn.classList.toggle("hidden", on);
    stopBtn.classList.toggle("hidden", !on);
    input.disabled = true;
    if (!on) {
      input.disabled = false;
      input.focus();
    }
  }

  async function send() {
    if (sending) return;
    var content = input.value.trim();
    if (!content) return;
    var id = convId();

    setStreaming(true);
    input.value = "";
    autoresize();
    appendUserMsg(content);

    if (!id) {
      try {
        var r = await fetch("/api/conversations", {
          method: "POST",
          headers: jsonHeaders(),
          body: JSON.stringify({}),
        });
        if (!r.ok) throw new Error("http " + r.status);
        var data = await r.json();
        id = data.id;
        input.dataset.conv = id;
        if (titleEl) {
          titleEl.dataset.conv = id;
          titleEl.textContent = truncate(content, 48);
        }
        triggerConvRefresh();
      } catch (e) {
        setStreaming(false);
        window.toast && toast("Không tạo được cuộc trò chuyện. Thử lại nhé.", "error");
        return;
      }
    }

    await stream("/api/chat", { conversation_id: id, content: content });
  }

  async function regenerate() {
    if (sending) return;
    var id = convId();
    if (!id) return;
    // Xoa cac phan tu assistant cuoi tren man hinh
    var msgs = messagesEl.querySelectorAll(".msg-assistant");
    for (var i = msgs.length - 1; i >= 0; i--) {
      msgs[i].remove();
      break; // chi xoa phan tu cuoi cung
    }
    setStreaming(true);
    await stream("/api/chat/regenerate", { conversation_id: id });
  }

  function stop() {
    if (controller) controller.abort();
  }

  function triggerConvRefresh() {
    if (window.htmx) {
      window.htmx.trigger(document.body, "refresh-conversations");
    }
  }

  async function stream(url, body) {
    controller = new AbortController();
    var ai = appendAssistantMsg();
    var think = null;
    var toolsBox = null;
    var bodyEl = ai.querySelector(".msg-body");
    var textBuf = "";
    var thinkBuf = "";
    var thinkStart = 0;
    var textStart = 0;
    var rafPending = false;

    function renderBody() {
      if (rafPending) return;
      rafPending = true;
      requestAnimationFrame(function () {
        rafPending = false;
        bodyEl.innerHTML = window.renderMarkdown ? window.renderMarkdown(textBuf) : escapeText(textBuf);
        maybeScroll();
      });
    }

    function handleFrame(frame) {
      var evName = "message";
      var evData = "";
      frame.split("\n").forEach(function (line) {
        if (line.indexOf("event:") === 0) evName = line.slice(6).trim();
        else if (line.indexOf("data:") === 0) evData += line.slice(5).trim();
      });
      if (!evData) return;
      var data;
      try { data = JSON.parse(evData); } catch (e) { return; }

      if (evName === "delta") {
        if (!textStart) textStart = performance.now();
        hideStatus(ai);
        if (think && think.root.open) think.root.open = false;
        textBuf += data.text || "";
        renderBody();
      } else if (evName === "thinking") {
        if (!thinkStart) thinkStart = performance.now();
        if (!think) think = buildThinking(ai);
        thinkBuf += data.text || "";
        think.body.textContent = thinkBuf;
        think.body.scrollTop = think.body.scrollHeight;
        maybeScroll();
      } else if (evName === "tool") {
        hideStatus(ai);
        toolsBox = ensureToolsBox(ai);
        handleToolLine(toolsBox, data);
        maybeScroll();
      } else if (evName === "done") {
        if (data.truncated) {
          appendNotice(ai, "Câu trả lời này đã bị ngắt vì đạt giới hạn đầu ra.", "warn");
        }
        if (think && think.label) {
          var dur = data.duration_ms ? data.duration_ms / 1000 : (performance.now() - (thinkStart || performance.now())) / 1000;
          think.label.textContent = "Đã suy nghĩ · " + dur.toFixed(1) + "s";
        }
      } else if (evName === "error") {
        appendNotice(ai, data.message || "Có lỗi xảy ra, vui lòng thử lại.", "error");
      }
    }

    try {
      var resp = await fetch(url, {
        method: "POST",
        signal: controller.signal,
        headers: jsonHeaders(),
        body: JSON.stringify(body),
      });
      if (!resp.ok || !resp.body) throw new Error("http " + resp.status);
      var reader = resp.body.getReader();
      var decoder = new TextDecoder();
      var buf = "";
      while (true) {
        var chunk = await reader.read();
        if (chunk.done) break;
        buf += decoder.decode(chunk.value, { stream: true });
        var parts = buf.split("\n\n");
        buf = parts.pop();
        parts.forEach(handleFrame);
      }
      if (buf.trim()) handleFrame(buf);
    } catch (err) {
      if (err && err.name === "AbortError") {
        appendNotice(ai, "Đã dừng tạo câu trả lời.", "info");
      } else {
        appendNotice(ai, "Halis tạm thời không phản hồi. Vui lòng thử lại sau ít phút.", "error");
      }
    }

    // Don dep va chot
    hideStatus(ai);
    bodyEl.innerHTML = window.renderMarkdown ? window.renderMarkdown(textBuf) : escapeText(textBuf);
    if (think && think.root.open) think.root.open = false;
    window.decorateCode && window.decorateCode(ai);
    triggerConvRefresh();
    setStreaming(false);
    controller = null;
    scrollToBottom(false);
  }

  // ============== Doi ten cuoc tro chuyen ==============

  async function rename() {
    var id = convId();
    if (!id) return;
    var current = titleEl ? titleEl.textContent : "";
    var title = window.prompt("Tên mới cho cuộc trò chuyện:", current);
    if (!title) return;
    title = title.trim().slice(0, 120);
    if (!title) return;
    try {
      var r = await fetch("/api/conversations/" + id + "/rename", {
        method: "POST",
        headers: jsonHeaders(),
        body: JSON.stringify({ title: title }),
      });
      if (!r.ok) throw new Error("http " + r.status);
      if (titleEl) titleEl.textContent = title;
      triggerConvRefresh();
    } catch (e) {
      window.toast && toast("Không đổi được tên.", "error");
    }
  }
})();
