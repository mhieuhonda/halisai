// HalisAI - client chat voi streaming SSE
(function () {
  "use strict";

  var composer, input, sendBtn, stopBtn, messagesEl, titleEl, regenBtn, renameBtn, delBtn, scrollBtn;
  var controller = null;
  var sending = false;

  var T = window.I18N || {
    stopped: "Đã dừng tạo câu trả lời.",
    errGeneric: "Halis tạm thời không phản hồi. Vui lòng thử lại sau ít phút.",
    truncated: "Câu trả lời này đã bị ngắt vì đạt giới hạn đầu ra.",
    cannotCreate: "Không tạo được cuộc trò chuyện. Thử lại nhé.",
    cannotRename: "Không đổi được tên."
  };
  var MORE = (document.documentElement.lang === "en"
    ? { thinking: "Thinking", thought: "Thought for", running: "running…", stopped: "Stopped generating.", you: "You", justNow: "just now" }
    : { thinking: "Đang suy nghĩ", thought: "Đã suy nghĩ", running: "đang chạy…", stopped: "Đã dừng tạo câu trả lời.", you: "Bạn", justNow: "vừa xong" });

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
    delBtn = document.getElementById("del-btn");
    scrollBtn = document.getElementById("scroll-bottom");

    if (!composer) return;

    scrollToBottom(true);
    updateSendState();

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
    input.addEventListener("input", function () {
      autoresize();
      updateSendState();
    });
    stopBtn.addEventListener("click", stop);
    if (regenBtn) regenBtn.addEventListener("click", regenerate);
    if (renameBtn) renameBtn.addEventListener("click", rename);
    if (delBtn) delBtn.addEventListener("click", function () {
      // Sau khi xóa, về trang chủ
      setTimeout(function () { window.location.href = "/"; }, 350);
    });

    Array.prototype.forEach.call(document.querySelectorAll(".chip"), function (chip) {
      chip.addEventListener("click", function () {
        input.value = chip.getAttribute("data-fill") || "";
        input.focus();
        autoresize();
        updateSendState();
      });
    });

    input.addEventListener("focus", removeEmptyOnce);
    if (scrollBtn) {
      scrollBtn.addEventListener("click", function () { scrollToBottom(true); });
      messagesEl.addEventListener("scroll", function () {
        scrollBtn.classList.toggle("hidden", nearBottom());
      });
    }
    autoresize();
  }

  function updateSendState() {
    if (!sendBtn) return;
    sendBtn.classList.toggle("idle", !input.value.trim());
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
    return messagesEl.scrollHeight - messagesEl.scrollTop - messagesEl.clientHeight < 160;
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
      '<div class="msg-meta"><span class="msg-author author-user">' + MORE.you + "</span>" +
      '<span class="msg-time">' + MORE.justNow + "</span></div>" +
      '<div class="msg-block"><div class="msg-body msg-user-body"></div></div>';
    el.querySelector(".msg-body").textContent = content;
    messagesEl.appendChild(el);
    scrollToBottom(true);
    return el;
  }

  function appendAssistantMsg() {
    removeEmptyOnce();
    var el = document.createElement("article");
    el.className = "msg msg-assistant";
    el.innerHTML =
      '<div class="msg-meta"><span class="msg-avatar" aria-hidden="true">' +
      '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4h9l7 7-7 7H4z"/><path d="M9 9l6 6"/></svg>' +
      "</span>" +
      '<span class="msg-author author-ai">Halis</span><span class="msg-time">' + MORE.justNow + "</span></div>" +
      '<div class="ai-status"><span class="cursor"></span><span class="dots">' + MORE.thinking + "</span></div>" +
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
      '<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6"/></svg>' +
      "</span><span class=\"think-label\">" + MORE.thinking + "</span></summary>" +
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
        '<span class="tool-name"></span><span class="tool-summary">' + MORE.running + "</span><span class=\"tool-ms\"></span>";
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
    updateSendState();
    var userEl = appendUserMsg(content);

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
        userEl.remove();
        input.value = content;
        autoresize();
        updateSendState();
        setStreaming(false);
        window.toast && toast(T.cannotCreate || "Không tạo được cuộc trò chuyện. Thử lại nhé.", "error");
        return;
      }
    }

    await stream("/api/chat", { conversation_id: id, content: content }, { userEl: userEl, content: content });
  }

  async function regenerate() {
    if (sending) return;
    var id = convId();
    if (!id) return;
    // Xoa phan tu assistant cuoi tren man hinh
    var msgs = messagesEl.querySelectorAll(".msg-assistant");
    for (var i = msgs.length - 1; i >= 0; i--) {
      msgs[i].remove();
      break; // chi xoa phan tu cuoi cung
    }
    setStreaming(true);
    await stream("/api/chat/regenerate", { conversation_id: id }, {});
  }

  function stop() {
    if (controller) controller.abort();
  }

  function triggerConvRefresh() {
    if (window.htmx) {
      window.htmx.trigger(document.body, "refresh-conversations");
    }
  }

  async function stream(url, body, ctxInfo) {
    controller = new AbortController();
    var ai = appendAssistantMsg();
    var think = null;
    var toolsBox = null;
    var bodyEl = ai.querySelector(".msg-body");
    var textBuf = "";
    var thinkStart = 0;
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
        hideStatus(ai);
        if (think && think.root.open) think.root.open = false;
        textBuf += data.text || "";
        renderBody();
      } else if (evName === "thinking") {
        if (!thinkStart) thinkStart = performance.now();
        if (!think) think = buildThinking(ai);
        think.body.textContent += data.text || "";
        think.body.scrollTop = think.body.scrollHeight;
        maybeScroll();
      } else if (evName === "tool") {
        hideStatus(ai);
        toolsBox = ensureToolsBox(ai);
        handleToolLine(toolsBox, data);
        maybeScroll();
      } else if (evName === "usage") {
        // Trạng thái mức dùng mới nhất (kể cả bị ép 100% vì lỗi 429)
        window.updateUsage && window.updateUsage(data);
      } else if (evName === "done") {
        if (data.truncated) {
          appendNotice(ai, T.truncated, "warn");
        }
        if (think && think.label) {
          var dur = data.duration_ms ? data.duration_ms / 1000 : (performance.now() - (thinkStart || performance.now())) / 1000;
          think.label.textContent = MORE.thought + " · " + dur.toFixed(1) + "s";
        }
      } else if (evName === "error") {
        appendNotice(ai, data.message || T.errGeneric, "error");
      }
    }

    try {
      var resp = await fetch(url, {
        method: "POST",
        signal: controller.signal,
        headers: jsonHeaders(),
        body: JSON.stringify(body),
      });
      if (!resp.ok) {
        // 429 mức dùng: cập nhật thanh usage, hoàn tác tin nhắn
        if (resp.status === 429) {
          var j = null;
          try { j = await resp.json(); } catch (e) {}
          if (j && j.usage && window.updateUsage) {
            window.updateUsage(j.usage);
          }
          if (ctxInfo.userEl && ctxInfo.content) {
            ctxInfo.userEl.remove();
            input.value = ctxInfo.content;
            autoresize();
            updateSendState();
          }
          window.toast && toast((j && j.message) || T.errGeneric, "error");
          ai.remove();
          setStreaming(false);
          controller = null;
          return;
        }
        throw new Error("http " + resp.status);
      }
      if (!resp.body) throw new Error("no body");
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
        appendNotice(ai, MORE.stopped, "info");
      } else {
        appendNotice(ai, T.errGeneric, "error");
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
      window.toast && toast(T.cannotRename || "Không đổi được tên.", "error");
    }
  }
})();
