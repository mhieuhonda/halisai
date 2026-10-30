// HalisAI - JS chung: theme, i18n client, markdown, usage, tiện ích
(function () {
  "use strict";

  // ===== Ngôn ngữ phía client =====
  var LANG = (document.documentElement.lang === "en" ? "en" : "vi");

  var I18N = {
    vi: {
      messagesLeft: function (n) { return "Còn " + n + " tin nhắn"; },
      limitReached: "Đã đạt giới hạn",
      resetsAt: "đặt lại lúc",
      reopensAt: "mở lại lúc",
      reopensIn: "mở lại sau",
      refreshesIn: "làm mới sau",
      used: function (n) { return n + " đã dùng"; },
      blockedNote: "Hạn mức sẽ mở lại sau",
      normalNote: "Hạn mức làm mới sau",
      copied: "Đã copy",
      errorGeneric: "Có lỗi xảy ra khi thực hiện thao tác.",
      tooFast: "Bạn thao tác quá nhanh, hãy thử lại sau.",
      saved: "Đã lưu thay đổi"
    },
    en: {
      messagesLeft: function (n) { return n + (n === 1 ? " message left" : " messages left"); },
      limitReached: "Limit reached",
      resetsAt: "resets at",
      reopensAt: "reopens",
      reopensIn: "reopens in",
      refreshesIn: "refreshes in",
      used: function (n) { return n + " used"; },
      blockedNote: "Your allowance reopens in",
      normalNote: "Your allowance refreshes in",
      copied: "Copied",
      errorGeneric: "Something went wrong.",
      tooFast: "You're going too fast, try again shortly.",
      saved: "Changes saved"
    }
  };
  var t = I18N[LANG];
  window.I18N = t;

  // ===== HTMX config =====
  document.addEventListener("DOMContentLoaded", function () {
    if (window.htmx) {
      window.htmx.config.includeIndicatorStyles = false;
      var meta = document.querySelector('meta[name="csrf-token"]');
      var csrf = meta ? meta.getAttribute("content") : "";
      document.body.addEventListener("htmx:configRequest", function (e) {
        e.detail.headers["X-CSRF-Token"] = csrf;
      });
      document.body.addEventListener("htmx:responseError", function (e) {
        var msg = t.errorGeneric;
        try {
          var text = e.detail.xhr.responseText || "";
          if (text.indexOf("Quá nhanh") !== -1 || text.indexOf("too fast") !== -1) msg = t.tooFast;
        } catch (err) {}
        toast(msg, "error");
      });
    }

    if (window.marked && window.marked.setOptions) {
      window.marked.setOptions({ gfm: true, breaks: true });
    }
  });

  // ===== Toast =====
  var toastWrap = null;
  window.toast = function (message, type) {
    if (!toastWrap) {
      toastWrap = document.createElement("div");
      toastWrap.className = "toast-wrap";
      document.body.appendChild(toastWrap);
    }
    var el = document.createElement("div");
    el.className = "toast" + (type === "error" ? " toast-error" : "");
    el.textContent = message;
    toastWrap.appendChild(el);
    setTimeout(function () {
      el.style.opacity = "0";
      el.style.transition = "opacity 0.3s";
      setTimeout(function () { el.remove(); }, 320);
    }, 3400);
  };

  // ===== Markdown + sanitize =====
  window.renderMarkdown = function (text) {
    if (window.marked && window.DOMPurify) {
      try {
        return window.DOMPurify.sanitize(window.marked.parse(text));
      } catch (e) { /* fallback dưới */ }
    }
    var div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  };

  // ===== Nút copy cho khối code =====
  window.decorateCode = function (scope) {
    var root = scope || document;
    var pres = root.querySelectorAll(".msg-body pre");
    pres.forEach(function (pre) {
      if (pre.closest(".code-block")) return;
      var code = pre.querySelector("code");
      var lang = "";
      if (code) {
        var m = (code.className || "").match(/language-([\w+#-]+)/);
        lang = m ? m[1] : "";
      }
      var wrap = document.createElement("div");
      wrap.className = "code-block";
      var head = document.createElement("div");
      head.className = "code-head";
      var langSpan = document.createElement("span");
      langSpan.textContent = lang || "code";
      var btn = document.createElement("button");
      btn.type = "button";
      btn.className = "copy-btn";
      btn.innerHTML = svgCopy() + "<span>Copy</span>";
      btn.addEventListener("click", function () {
        var text = (code || pre).innerText;
        navigator.clipboard.writeText(text).then(function () {
          btn.innerHTML = svgCheck() + "<span>" + t.copied + "</span>";
          setTimeout(function () { btn.innerHTML = svgCopy() + "<span>Copy</span>"; }, 1600);
        });
      });
      head.appendChild(langSpan);
      head.appendChild(btn);
      pre.parentNode.insertBefore(wrap, pre);
      wrap.appendChild(head);
      wrap.appendChild(pre);
    });
  };

  function svgCopy() {
    return '<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>';
  }
  function svgCheck() {
    return '<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>';
  }

  // ===== Nút copy tin nhắn =====
  document.addEventListener("click", function (e) {
    var btn = e.target.closest(".msg-copy");
    if (!btn) return;
    var msg = btn.closest(".msg");
    var body = msg ? msg.querySelector(".msg-body") : null;
    if (!body) return;
    var text = body.innerText.trim();
    navigator.clipboard.writeText(text).then(function () {
      btn.classList.add("copied");
      setTimeout(function () { btn.classList.remove("copied"); }, 1600);
    });
  });

  // ===== Mức dùng (thanh phân đoạn kiểu Claude) =====
  var usageState = null;
  var usageTimer = null;

  function fmtClock(date) {
    try {
      return new Intl.DateTimeFormat(LANG === "en" ? "en-US" : "vi-VN", {
        hour: LANG === "en" ? "numeric" : "2-digit",
        minute: "2-digit",
        hour12: LANG === "en"
      }).format(date);
    } catch (e) {
      return "";
    }
  }

  function fmtCountdown(secs) {
    secs = Math.max(0, Math.floor(secs));
    var h = Math.floor(secs / 3600);
    var m = Math.floor((secs % 3600) / 60);
    var s = secs % 60;
    var mm = (m < 10 ? "0" : "") + m;
    var ss = (s < 10 ? "0" : "") + s;
    return h > 0 ? h + ":" + mm + ":" + ss : mm + ":" + ss;
  }

  function renderUsage() {
    if (!usageState) return;
    var u = usageState;
    var percent = Math.round(((u.used || 0) / (u.limit || 200)) * 100);
    var blocked = !!u.blocked;
    var pill = document.getElementById("usage-pill");
    var card = document.getElementById("usage-card");

    if (pill) {
      if (u.used > 0 || blocked) pill.classList.remove("hidden");
      else pill.classList.add("hidden");
      pill.classList.toggle("full", blocked || percent >= 100);
      pill.classList.toggle("warn", !blocked && percent >= 80 && percent < 100);

      var segs = pill.querySelectorAll(".seg");
      var filled = Math.min(5, Math.max(u.used > 0 ? 1 : 0, Math.round(percent / 20)));
      segs.forEach(function (seg, i) {
        seg.classList.toggle("fill", i < filled);
      });

      var countEl = pill.querySelector(".usage-count");
      var resetEl = pill.querySelector(".usage-reset");
      var left = Math.max(0, (u.limit || 200) - (u.used || 0));
      var when = u.resets_at ? fmtClock(new Date(u.resets_at)) : "";
      if (blocked) {
        countEl.textContent = t.limitReached;
      } else {
        countEl.textContent = t.messagesLeft(left);
      }
      resetEl.textContent = when ? (blocked ? t.reopensAt : t.resetsAt) + " " + when : "";
    }

    if (card) {
      card.classList.toggle("full", blocked || percent >= 100);
      card.classList.toggle("warn", !blocked && percent >= 80 && percent < 100);
      var segs2 = card.querySelectorAll(".seg");
      var filled2 = Math.min(5, Math.max(u.used > 0 ? 1 : 0, Math.round(percent / 20)));
      segs2.forEach(function (seg, i) {
        seg.classList.toggle("fill", i < filled2);
      });
      var cCount = card.querySelector(".usage-count b");
      var cPct = card.querySelector(".usage-pct");
      var cNote = card.querySelector('[data-role="note"]');
      if (cCount) cCount.textContent = (u.used || 0) + " / " + (u.limit || 200);
      if (cPct) cPct.textContent = percent + "%";
      if (cNote) {
        var secs = u.resets_at ? Math.floor((new Date(u.resets_at) - Date.now()) / 1000) : 0;
        if (secs > 0) {
          var head = blocked ? t.blockedNote : t.normalNote;
          cNote.innerHTML = "";
          cNote.appendChild(document.createTextNode(head + " "));
          var b = document.createElement("b");
          b.className = "cd";
          b.textContent = fmtCountdown(secs);
          cNote.appendChild(b);
          cNote.appendChild(document.createTextNode("."));
        } else {
          cNote.textContent = "";
        }
      }
    }
  }

  function startUsageTicker() {
    if (usageTimer) return;
    usageTimer = setInterval(renderUsage, 1000);
  }

  window.updateUsage = function (data) {
    if (!data || typeof data !== "object") return;
    usageState = data;
    renderUsage();
    startUsageTicker();
  };

  document.addEventListener("DOMContentLoaded", function () {
    var pill = document.getElementById("usage-pill");
    var card = document.getElementById("usage-card");
    var root = card || pill;
    if (root) {
      var used = parseInt(root.getAttribute("data-used") || "0", 10) || 0;
      var limit = parseInt(root.getAttribute("data-limit") || "200", 10) || 200;
      var blocked = root.getAttribute("data-blocked") === "true";
      var resets = root.getAttribute("data-resets") || "";
      var snap = { used: used, limit: limit, blocked: blocked, resets_at: resets || null };
      // Nguồn sự thật là máy chủ; lấy trạng thái mới nhất khi tải trang
      fetch("/api/usage", { headers: { "X-CSRF-Token": csrfToken() } })
        .then(function (r) { return r.ok ? r.json() : null; })
        .then(function (j) { window.updateUsage(j || snap); })
        .catch(function () { window.updateUsage(snap); });
    }
  });

  // ===== Cài đặt: chủ đề & ngôn ngữ =====
  document.addEventListener("DOMContentLoaded", function () {
    var picker = document.getElementById("theme-picker");
    if (picker) {
      picker.addEventListener("click", function (e) {
        var btn = e.target.closest(".theme-option");
        if (!btn) return;
        var theme = btn.getAttribute("data-theme");
        applyTheme(theme);
        picker.querySelectorAll(".theme-option").forEach(function (b) {
          b.setAttribute("aria-pressed", b === btn ? "true" : "false");
        });
        savePrefs({ theme: theme });
      });
    }

    var langPicker = document.getElementById("lang-picker");
    if (langPicker) {
      langPicker.addEventListener("click", function (e) {
        var btn = e.target.closest(".lang-option");
        if (!btn) return;
        var lang = btn.getAttribute("data-lang");
        langPicker.querySelectorAll(".lang-option").forEach(function (b) {
          b.setAttribute("aria-pressed", b === btn ? "true" : "false");
        });
        savePrefs({ lang: lang }).then(function () {
          window.location.reload();
        });
      });
    }
  });

  function applyTheme(theme) {
    if (theme !== "light" && theme !== "dark") return;
    document.documentElement.dataset.theme = theme;
    try { localStorage.setItem("halis_theme", theme); } catch (e) {}
    var meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.setAttribute("content", theme === "dark" ? "#131010" : "#f8f8f8");
  }

  function savePrefs(payload) {
    return fetch("/api/settings/prefs", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": csrfToken() },
      body: JSON.stringify(payload)
    }).then(function (r) {
      if (!r.ok) throw new Error("http " + r.status);
      return r.json();
    }).catch(function (e) {
      window.toast && toast(t.errorGeneric, "error");
      throw e;
    });
  }

  function csrfToken() {
    var m = document.querySelector('meta[name="csrf-token"]');
    return m ? m.getAttribute("content") : "";
  }

  // ===== Thông báo toàn hệ thống =====
  document.addEventListener("DOMContentLoaded", function () {
    var ann = document.querySelector(".announcement");
    if (!ann) return;
    var key = ann.getAttribute("data-key") || "";
    if (key && localStorage.getItem("halis_ann_dismissed") === key) {
      ann.remove();
    }
    var close = ann.querySelector(".ann-close");
    if (close) {
      close.addEventListener("click", function () {
        localStorage.setItem("halis_ann_dismissed", key);
        ann.remove();
      });
    }
  });

  // ===== Heartbeat online =====
  document.addEventListener("DOMContentLoaded", function () {
    if (!document.querySelector('meta[name="csrf-token"]')) return;
    setInterval(function () {
      fetch("/api/ping", { method: "GET", keepalive: true }).catch(function () {});
    }, 45000);
  });

  // ===== Trang trí sau khi load =====
  document.addEventListener("DOMContentLoaded", function () {
    decorateCode(document);
  });
})();
