// HalisAI - JS chung
(function () {
  "use strict";

  // ===== HTMX config =====
  document.addEventListener("DOMContentLoaded", function () {
    if (window.htmx) {
      window.htmx.config.includeIndicatorStyles = false;
      var meta = document.querySelector('meta[name="csrf-token"]');
      var csrf = meta ? meta.getAttribute("content") : "";
      document.body.addEventListener("htmx:configRequest", function (e) {
        e.detail.headers["X-CSRF-Token"] = csrf;
      });
      // Thong bao loi HTMX
      document.body.addEventListener("htmx:responseError", function (e) {
        var msg = "Có lỗi xảy ra khi thực hiện thao tác.";
        try {
          var text = e.detail.xhr.responseText || "";
          if (text.indexOf("Quá nhanh") !== -1) msg = "Bạn thao tác quá nhanh, hãy thử lại sau.";
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
      } catch (e) { /* fallback duoi */ }
    }
    var div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  };

  // ===== The copy button cho code block =====
  window.decorateCode = function (scope) {
    var root = scope || document;
    var pres = root.querySelectorAll(".msg-body pre");
    pres.forEach(function (pre) {
      if (pre.closest(".code-block")) return;
      var code = pre.querySelector("code");
      var lang = "";
      if (code) {
        var m = (code.className || "").match(/language-([\w+-]+)/);
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
          btn.innerHTML = svgCheck() + "<span>Đã copy</span>";
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

  // ===== Announcement dismiss =====
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

  // ===== Trang tri sau khi load =====
  document.addEventListener("DOMContentLoaded", function () {
    decorateCode(document);
  });
})();
