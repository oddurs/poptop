/* The scrubbable frames.
   ---------------------------------------------------------------------------
   Drawing lives in `frame.js`. This file is the part that makes a frame
   respond: keyboard, pointer, playback, and the one scripted moment the page
   spends on the landing hero.

   The buffer is generated once, on the server, from a fixed seed, so the same
   incident is on the page every time and a static export is byte-identical
   between builds. */
(function () {
  "use strict";

  var P = window.Poptop;
  var payload = document.getElementById("demo-buffer");
  if (!P || !payload) return;

  var data;
  try {
    data = JSON.parse(payload.textContent);
    if (!data.cpu || !data.cpu.length || !data.procs || !data.procs.length) {
      throw new Error("buffer is empty");
    }
  } catch (e) {
    document.querySelectorAll("[data-demo], [data-frame]").forEach(fail);
    return;
  }

  // Every still on the page, drawn once.
  P.stills(data);

  // Then every frame that answers to a person.
  document.querySelectorAll("[data-demo]").forEach(function (root) {
    try {
      drive(root, data);
    } catch (e) {
      fail(root);
    }
  });

  /* If anything fails, the frame is an empty box with a caption claiming it is
     interactive. Say so instead, and point at the thing that does work. */
  function fail(root) {
    root.removeAttribute("tabindex");
    root.setAttribute("data-demo-failed", "true");
    var screen = root.querySelector("[data-screen]");
    if (screen) {
      screen.innerHTML =
        '<p class="term__failed">This frame could not start. The README has the ' +
        'same session as text: <a href="https://github.com/oddurs/poptop">' +
        "github.com/oddurs/poptop</a>.</p>";
    }
  }

  function drive(root, data) {
    var config = {};
    try {
      config = JSON.parse(root.getAttribute("data-demo") || "{}");
    } catch (e) {}

    var ZOOMS = [1, 2, 4, 8]; // samples per half-cell
    var n = data.cpu.length;
    var quiet = config.quiet != null ? config.quiet : 40;
    var focus = config.focus != null ? config.focus : data.focus != null ? data.focus : n - 1;

    var cursor = config.intro ? quiet : focus;
    var zoom = 0;
    var live = false;
    var timer = null;
    var pending = null;
    var cols = 68;
    var intro = null; // the scripted opening, while it is still running
    var previous = null;

    var el = {
      screen: root.querySelector("[data-screen]"),
      plot: root.querySelector("[data-plot-cpu], [data-plot-mem]"),
      gutter: root.querySelector("[data-gutter-cpu], [data-gutter-mem]"),
      say: document.querySelector(root.getAttribute("data-say") || "[data-say]"),
      caption: config.caption ? document.querySelector(config.caption) : null,
    };

    var calm = window.matchMedia("(prefers-reduced-motion: reduce)");

    function fit() {
      if (!el.screen || !el.plot) return;
      cols = P.measure(el.screen, el.plot, el.gutter, 40, 160);
    }

    function render() {
      pending = null;
      var out = P.draw(root, data, {
        cursor: cursor,
        cols: cols,
        step: ZOOMS[zoom],
        rows: config.rows || 5,
        live: live,
        previous: previous,
      });
      previous = out.order;

      if (el.say && !live) {
        var i = out.index;
        var top = data.procs
          .map(function (p) {
            return { cmd: p.cmd, cpu: p.cpu[i] };
          })
          .sort(function (a, b) {
            return b.cpu - a.cpu;
          })[0];
        el.say.textContent =
          "At -" + P.clock((n - 1 - i) * data.interval) + ": CPU " +
          data.cpu[i].toFixed(0) + " percent, memory " + data.mem[i].toFixed(0) +
          " percent. Busiest process " + top.cmd + " at " + top.cpu.toFixed(0) + " percent.";
      }
    }

    function schedule() {
      if (pending) return;
      pending = requestAnimationFrame(function () {
        try {
          render();
        } catch (e) {
          setLive(false);
          fail(root);
        }
      });
    }

    /* ── controls ───────────────────────────────────────────────────────── */

    function stopIntro() {
      if (!intro) return;
      cancelAnimationFrame(intro);
      intro = null;
      root.removeAttribute("data-intro");
      if (el.caption) el.caption.setAttribute("data-moment", "then");
    }

    function scrub(delta) {
      stopIntro();
      setLive(false);
      cursor = Math.max(0, Math.min(n - 1, cursor + delta));
      schedule();
    }

    function setZoom(delta) {
      stopIntro();
      var next = Math.max(0, Math.min(ZOOMS.length - 1, zoom + delta));
      if (next === zoom) return;
      zoom = next;
      schedule();
    }

    function setLive(on) {
      if (on) stopIntro();
      if (live === on) return;
      live = on;
      if (timer) clearInterval(timer);
      timer = null;
      if (live) {
        timer = setInterval(function () {
          cursor = cursor >= n - 1 ? quiet : cursor + 1;
          schedule();
        }, 320);
      }
      schedule();
    }

    root.addEventListener("keydown", function (ev) {
      var big = ev.shiftKey ? 10 : 1;
      switch (ev.key) {
        case "ArrowLeft": scrub(-big); break;
        case "ArrowRight": scrub(big); break;
        case "+": case "=": setZoom(-1); break;
        case "-": case "_": setZoom(1); break;
        case " ": stopIntro(); setLive(!live); break;
        case "Home": stopIntro(); setLive(false); cursor = 0; schedule(); break;
        case "End": stopIntro(); setLive(false); cursor = n - 1; schedule(); break;
        default: return;
      }
      ev.preventDefault();
    });

    var dragging = false;
    var dragX = 0;

    if (el.screen) {
      el.screen.addEventListener("pointerdown", function (ev) {
        dragging = true;
        dragX = ev.clientX;
        el.screen.setPointerCapture(ev.pointerId);
        stopIntro();
        setLive(false);
        root.focus({ preventScroll: true });
      });

      el.screen.addEventListener("pointermove", function (ev) {
        if (!dragging) return;
        // One half-cell of travel is one slot, so dragging tracks the plot
        // rather than some invented sensitivity.
        var cell = el.screen.clientWidth / (cols * 2);
        var moved = (dragX - ev.clientX) / cell;
        if (Math.abs(moved) < 1) return;
        dragX = ev.clientX;
        scrub(-Math.trunc(moved) * ZOOMS[zoom]);
      });

      ["pointerup", "pointercancel"].forEach(function (name) {
        el.screen.addEventListener(name, function () {
          dragging = false;
        });
      });
    }

    // Controls anywhere on the page may name this frame.
    var act = function (button) {
      var a = button.getAttribute("data-act");
      if (a === "live") setLive(!live);
      else if (a === "back") scrub(-10);
      else if (a === "forward") scrub(10);
      else if (a === "zoom-in") setZoom(-1);
      else if (a === "zoom-out") setZoom(1);
      else if (a === "oldest") { setLive(false); cursor = 0; schedule(); }
      else if (a === "newest") { setLive(false); cursor = n - 1; schedule(); }
    };
    root.querySelectorAll("[data-act]").forEach(function (b) {
      b.addEventListener("click", function () { act(b); });
    });
    if (root.id) {
      document.querySelectorAll('[data-act-for="' + root.id + '"]').forEach(function (b) {
        b.addEventListener("click", function () {
          root.scrollIntoView({ block: "nearest", behavior: calm.matches ? "auto" : "smooth" });
          act(b);
        });
      });
    }

    /* ── the opening ────────────────────────────────────────────────────── */

    /* One scripted moment: the frame opens on a quiet stretch — what you see
       when you arrive — then travels backwards to the incident. The travel is
       the gesture the product is named for, which is why it is worth the only
       animation on the page. */
    function playIntro() {
      if (!config.intro) return;
      if (calm.matches) {
        cursor = focus;
        if (el.caption) el.caption.setAttribute("data-moment", "then");
        schedule();
        return;
      }
      root.setAttribute("data-intro", "true");
      if (el.caption) el.caption.setAttribute("data-moment", "now");

      var hold = 900;
      var travel = 1500;
      var from = quiet;
      var to = focus;
      var start = null;

      intro = requestAnimationFrame(function step(now) {
        if (start === null) start = now;
        var t = now - start;
        if (t < hold) {
          intro = requestAnimationFrame(step);
          return;
        }
        var k = Math.min(1, (t - hold) / travel);
        // Ease out: it leaves quickly and arrives gently, so the moment it
        // lands on is the one that reads.
        var eased = 1 - Math.pow(1 - k, 3);
        cursor = Math.round(from + (to - from) * eased);
        render();
        if (k < 1) {
          intro = requestAnimationFrame(step);
        } else {
          intro = null;
          root.removeAttribute("data-intro");
          if (el.caption) el.caption.setAttribute("data-moment", "then");
        }
      });
    }

    var resizing = null;
    window.addEventListener("resize", function () {
      clearTimeout(resizing);
      resizing = setTimeout(function () {
        fit();
        schedule();
      }, 120);
    });

    fit();
    render();

    if (config.intro) {
      // Only once it is actually on screen, and only once.
      if (typeof IntersectionObserver === "function") {
        var seen = false;
        new IntersectionObserver(function (entries, obs) {
          entries.forEach(function (entry) {
            if (!entry.isIntersecting || seen) return;
            seen = true;
            obs.disconnect();
            playIntro();
          });
        }, { threshold: 0.3 }).observe(root);
      } else {
        playIntro();
      }
    }
  }
})();
