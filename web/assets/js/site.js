/* Site chrome: theme choice, copy buttons, and the docs contents highlight.
   Small enough to stay one file, and inlined into every page so no request
   blocks the first paint. */
(function () {
  "use strict";

  /* ── theme ────────────────────────────────────────────────────────────
     Three states, not two: light, dark, and "whatever the system says",
     which is the default and stores nothing. */
  var toggle = document.querySelector("[data-theme-toggle]");
  if (toggle) {
    var label = function () {
      var chosen = document.documentElement.getAttribute("data-theme");
      toggle.textContent = chosen === "light" ? "☀" : chosen === "dark" ? "☾" : "◐";
      toggle.setAttribute(
        "aria-label",
        "Colour theme: " + (chosen || "system") + ". Click to change."
      );
    };
    label();
    toggle.addEventListener("click", function () {
      var order = ["", "light", "dark"];
      var now = document.documentElement.getAttribute("data-theme") || "";
      var next = order[(order.indexOf(now) + 1) % order.length];
      if (next) {
        document.documentElement.setAttribute("data-theme", next);
        try {
          localStorage.setItem("poptop-theme", next);
        } catch (e) {}
      } else {
        document.documentElement.removeAttribute("data-theme");
        try {
          localStorage.removeItem("poptop-theme");
        } catch (e) {}
      }
      label();
    });
  }

  /* ── copy ─────────────────────────────────────────────────────────────
     The button says what happened, then goes back to saying what it does. */
  document.querySelectorAll("[data-copy]").forEach(function (button) {
    button.addEventListener("click", function () {
      var text = button.getAttribute("data-copy");
      var done = function () {
        button.setAttribute("data-copied", "true");
        button.textContent = "copied";
        setTimeout(function () {
          button.removeAttribute("data-copied");
          button.textContent = "copy";
        }, 1600);
      };
      if (navigator.clipboard) {
        navigator.clipboard.writeText(text).then(done, function () {});
      }
    });
  });

  /* ── contents ─────────────────────────────────────────────────────────
     Marks the heading you are reading. Passive: it never scrolls the page
     or rewrites the address bar under you. */
  var toc = document.querySelector("[data-toc]");
  if (toc && typeof IntersectionObserver === "function") {
    var links = {};
    toc.querySelectorAll("a[href^='#']").forEach(function (a) {
      links[decodeURIComponent(a.getAttribute("href").slice(1))] = a;
    });
    var headings = Object.keys(links)
      .map(function (id) {
        return document.getElementById(id);
      })
      .filter(Boolean);

    // The heading you are reading is the last one you have scrolled past — not
    // whichever headings happen to be inside some band. Tracking the band left
    // the contents marking nothing at all through the long stretches between
    // headings, which is most of a page.
    var mark = function () {
      var current = headings[0];
      headings.forEach(function (h) {
        if (h.getBoundingClientRect().top <= 96) current = h;
      });
      Object.keys(links).forEach(function (id) {
        links[id].toggleAttribute("data-reading", !!current && current.id === id);
      });
    };

    // The observer is only a cheap way to be told that something crossed; the
    // answer is recomputed from positions either way.
    var observer = new IntersectionObserver(mark, {
      rootMargin: "-96px 0px 0px 0px",
      threshold: [0, 1],
    });
    headings.forEach(function (h) {
      observer.observe(h);
    });
    mark();
  }
})();
