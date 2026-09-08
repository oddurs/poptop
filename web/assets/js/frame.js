/* Drawing a poptop frame.
   ---------------------------------------------------------------------------
   Everything the page knows about turning a buffer into a picture lives here:
   the braille packing, the threshold rules that bars absorb where they cross
   them, the block bars, the per-process sparklines, and the status thresholds.

   Nothing in this file registers an event listener or knows what a cursor key
   is. That is `demo.js`, which drives the one interactive frame on the page.
   The separation exists because five sections need to draw a frame they do not
   want anyone to scrub — two moments side by side, one plot aggregated two
   ways, a buffer that is half empty — and copying the braille packing into
   each of them is how a page ends up with five drifting implementations of the
   thing the product is actually about.

   `Poptop.draw(root, data, state)` fills whichever `[data-*]` hooks it finds
   inside `root` and ignores the ones that are not there, so a still can be a
   plot alone and the hero can be the whole frame. */
window.Poptop = (function () {
  "use strict";

  var WARN = 50;
  var CRIT = 80;
  var BLOCKS = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

  // Dot bit for (half-column, row-within-cell), per the Unicode braille layout.
  var BIT = [
    [0x01, 0x02, 0x04, 0x40],
    [0x08, 0x10, 0x20, 0x80],
  ];

  /* ── formatting ───────────────────────────────────────────────────────── */

  function statusClass(v) {
    if (v >= CRIT) return "is-critical";
    if (v >= WARN) return "is-warn";
    return "is-ok";
  }

  function clock(seconds) {
    var s = Math.round(Math.abs(seconds));
    var m = (s / 60) | 0;
    return m ? m + "m" + String(s % 60).padStart(2, "0") + "s" : s + "s";
  }

  function bytes(kib) {
    if (kib >= 1024 * 1024) return (kib / 1024 / 1024).toFixed(1) + "G";
    if (kib >= 1024) return (kib / 1024).toFixed(1) + "M";
    return kib + "K";
  }

  function escapeHtml(s) {
    return String(s).replace(/[&<>"]/g, function (ch) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[ch];
    });
  }

  /* ── measuring ────────────────────────────────────────────────────────── */

  // A terminal draws as many columns as the window has cells. So does this:
  // the plot is measured against the actual glyph width rather than hard-coded
  // to a count that is right at one font size and one viewport.
  function measure(screen, plot, gutter, floor, ceiling) {
    var probe = document.createElement("span");
    probe.textContent = "⣿".repeat(50);
    probe.style.cssText = "position:absolute;visibility:hidden;white-space:pre";
    plot.appendChild(probe);
    var cell = probe.getBoundingClientRect().width / 50;
    probe.remove();
    if (!cell) return floor;

    var pad = parseFloat(getComputedStyle(screen).paddingLeft) * 2;
    var room = screen.clientWidth - pad - (gutter ? gutter.getBoundingClientRect().width : 0);
    var cols = Math.floor((room - cell * 1.5) / cell);
    // Below the floor the frame scrolls instead of shrinking, because a plot
    // narrower than this stops being readable before it stops fitting.
    return Math.max(floor, Math.min(ceiling, cols));
  }

  /* ── plotting ─────────────────────────────────────────────────────────── */

  // One half-column of the plot, counted from its left edge. It holds `step`
  // samples and reports the peak of them, because a spike that averages away is
  // the one thing this tool exists to keep. `mean` exists only so the page can
  // draw the mistake next to the fix.
  function slot(series, k, opt) {
    var last = opt.end - (opt.cols * 2 - 1 - k) * opt.step;
    if (last < opt.from) return null;
    var peak = 0;
    var total = 0;
    var seen = 0;
    for (var s = 0; s < opt.step; s++) {
      var i = last - s;
      if (i < opt.from || i >= series.length) continue;
      if (series[i] > peak) peak = series[i];
      total += series[i];
      seen++;
    }
    if (!seen) return null;
    return opt.aggregate === "mean" ? total / seen : peak;
  }

  /* Returns the plot as rows of braille, optionally with the last cell
     reversed to mark the cursor. */
  function braille(series, options) {
    var opt = {
      cols: options.cols,
      rows: options.rows || 5,
      step: options.step || 1,
      end: options.end,
      from: options.from || 0,
      aggregate: options.aggregate || "peak",
      thresholds: options.thresholds === undefined ? [WARN, CRIT] : options.thresholds,
      cursor: !!options.cursor,
    };
    var dots = opt.rows * 4;
    var cells = new Array(opt.rows);
    for (var r = 0; r < opt.rows; r++) cells[r] = new Array(opt.cols).fill(0x2800);

    for (var c = 0; c < opt.cols; c++) {
      var any = false;
      for (var half = 0; half < 2; half++) {
        var v = slot(series, c * 2 + half, opt);
        if (v === null) continue;
        any = true;
        var filled = Math.round((Math.min(v, 100) / 100) * dots);
        for (var y = 0; y < dots; y++) {
          if (y < dots - filled) continue;
          cells[(y / 4) | 0][c] |= BIT[half][y % 4];
        }
      }
      // Threshold rules, on alternate cells, only where a bar could have
      // reached: past the start of the buffer there is no scale to mark. A bar
      // absorbs the rule where it crosses it, which is what makes the scale
      // readable without relying on colour.
      if (any && c % 2 === 0) {
        opt.thresholds.forEach(function (t) {
          var y = Math.min(dots - 1, dots - Math.round((t / 100) * dots));
          var row = (y / 4) | 0;
          for (var h = 0; h < 2; h++) {
            var bit = BIT[h][y % 4];
            if (!(cells[row][c] & bit)) cells[row][c] |= bit;
          }
        });
      }
    }

    return cells.map(function (row) {
      var text = row
        .map(function (code) {
          return String.fromCharCode(code);
        })
        .join("");
      if (!opt.cursor) return escapeHtml(text);
      // The cursor marks which sample is under the readout. No hue: it is
      // reversed, the way the terminal marks position.
      return (
        escapeHtml(text.slice(0, opt.cols - 1)) +
        '<span class="term__cursor">' +
        escapeHtml(text.slice(opt.cols - 1)) +
        "</span>"
      );
    });
  }

  // The gutter names the series and anchors its scale, which is the whole job
  // of the labels down the left of a poptop plot.
  function gutter(name, rows) {
    var lines = new Array(rows).fill("   ");
    lines[0] = "100";
    lines[Math.floor(rows / 2)] = name;
    lines[rows - 1] = "  0";
    return lines.join("\n");
  }

  function bar(pct, width) {
    var eighths = Math.round((Math.min(pct, 100) / 100) * width * 8);
    var full = (eighths / 8) | 0;
    var rest = eighths % 8;
    return "█".repeat(full) + (rest ? BLOCKS[rest] : "");
  }

  function meter(pct, width) {
    var filled = Math.round((Math.min(pct, 100) / 100) * width);
    return "█".repeat(filled) + "░".repeat(Math.max(0, width - filled));
  }

  // A ten-cell braille history for one process, the table's HIST column.
  function sparkline(series, end, cells, from) {
    var out = "";
    for (var c = 0; c < cells; c++) {
      var code = 0x2800;
      for (var half = 0; half < 2; half++) {
        var i = end - (cells * 2 - 1 - (c * 2 + half));
        if (i < (from || 0) || i >= series.length) continue;
        var filled = Math.round((Math.min(series[i], 100) / 100) * 4);
        for (var y = 0; y < 4; y++) {
          if (y >= 4 - filled) code |= BIT[half][y];
        }
      }
      out += String.fromCharCode(code);
    }
    return out;
  }

  /* ── the frame ────────────────────────────────────────────────────────── */

  function stat(label, value, cls) {
    return (
      '<span class="term__stat">' +
      escapeHtml(label) +
      "&nbsp; <b" +
      (cls ? ' class="' + cls + '"' : "") +
      ">" +
      escapeHtml(value) +
      "</b></span>"
    );
  }

  function processes(data, i) {
    return data.procs
      .map(function (p) {
        return { p: p, cpu: p.cpu[i], rss: p.rss[i] };
      })
      .sort(function (a, b) {
        return b.cpu - a.cpu;
      });
  }

  /* Fill every hook present inside `root`.

     `state` carries: cursor, cols, step, rows, from, aggregate, live, dim, and
     `bare` for a still that wants no cursor mark. Anything absent falls back to
     something sensible, so a still can pass three fields. */
  function draw(root, data, state) {
    var q = function (sel) {
      return root.querySelector(sel);
    };
    var n = data.cpu.length;
    var i = Math.max(0, Math.min(n - 1, state.cursor));
    var s = {
      cols: state.cols || 68,
      rows: state.rows || 5,
      step: state.step || 1,
      end: i,
      from: state.from || 0,
      aggregate: state.aggregate || "peak",
      cursor: state.bare ? false : true,
    };

    var head = q("[data-head]");
    if (head) {
      head.innerHTML =
        stat("CPU", data.cpu[i].toFixed(1) + "%", statusClass(data.cpu[i])) +
        stat("WAIT", data.wait[i].toFixed(1) + "%", statusClass(data.wait[i])) +
        stat("RUN", data.run[i] + "/" + data.cores) +
        stat("BLOCKED", String(data.blocked[i])) +
        stat("MEM", data.mem[i].toFixed(1) + "%", statusClass(data.mem[i])) +
        '<span class="term__stat term__bar">' +
        escapeHtml(meter(data.mem[i], 12)) +
        "</span>";
    }

    // `spike` is CPU too — a different recording of it, for the section that
    // draws mean against peak — so its gutter says what it is rather than
    // naming the array it came from.
    var LABEL = { cpu: "CPU", mem: "MEM", wait: "WAIT", spike: "CPU" };
    Object.keys(LABEL).forEach(function (key) {
      var plot = q("[data-plot-" + key + "]");
      if (!plot || !data[key]) return;
      plot.innerHTML = braille(data[key], s).join("\n");
      var g = q("[data-gutter-" + key + "]");
      if (g) g.textContent = gutter(LABEL[key], s.rows);
    });

    var span = q("[data-span]");
    if (span) {
      var shown = s.cols * 2 * s.step * data.interval;
      span.textContent =
        clock(shown) + " shown, " + s.step + "s/slot · " + clock(n * data.interval) + " buffered";
    }

    var state_ = q("[data-state]");
    if (state_) {
      state_.textContent = state.live ? "LIVE" : "PAUSED";
      state_.setAttribute("data-live", state.live ? "true" : "false");
    }
    var offset = q("[data-offset]");
    if (offset) {
      offset.textContent = state.live ? "now" : "-" + clock((n - 1 - i) * data.interval);
    }

    var readout = q("[data-readout]");
    if (readout) {
      readout.innerHTML =
        "CPU " +
        '<span class="' + statusClass(data.cpu[i]) + '">' + data.cpu[i].toFixed(1) + "%</span>" +
        "&nbsp; MEM " +
        '<span class="' + statusClass(data.mem[i]) + '">' + data.mem[i].toFixed(1) + "%</span>";
    }

    var tbody = q("[data-procs]");
    if (tbody) {
      var rows = processes(data, i);
      var count = q("[data-count]");
      if (count) count.textContent = String(rows.length);
      tbody.innerHTML = rows
        .map(function (row, idx) {
          var p = row.p;
          var moved =
            state.previous && state.previous[p.pid] !== undefined && state.previous[p.pid] !== idx
              ? ' data-moved="' + (state.previous[p.pid] > idx ? "up" : "down") + '"'
              : "";
          var head =
            "<tr" + (idx === 0 ? ' aria-selected="true"' : "") + moved +
            '><td class="term__num ' + statusClass(row.cpu) + '">' + row.cpu.toFixed(1) +
            '</td><td class="term__bar">' + escapeHtml(bar(row.cpu, 6)) +
            '</td><td class="term__num">' + escapeHtml(bytes(row.rss)) +
            "</td><td>" + escapeHtml(p.state) + "</td>";
          // A still is narrow and is making a point about *which process*, so it
          // drops the columns that are not that. The one it must never lose is
          // COMMAND, which is the answer the whole tool exists to give.
          var middle = state.compact
            ? ""
            : '<td class="term__num">' + p.threads +
              '</td><td class="term__bar">' + escapeHtml(sparkline(p.cpu, i, 10, s.from)) + "</td>";
          return (
            head + middle +
            '<td class="term__num">' + p.pid +
            "</td><td>" + escapeHtml(p.cmd) + "</td></tr>"
          );
        })
        .join("");
    }

    return { index: i, order: order(data, i) };
  }

  /* Which row each pid occupies at a given sample, so a caller can mark what
     moved between two draws. */
  function order(data, i) {
    var out = {};
    processes(data, i).forEach(function (row, idx) {
      out[row.p.pid] = idx;
    });
    return out;
  }

  /* ── stills ───────────────────────────────────────────────────────────── */

  /* Every `[data-frame]` on the page is a still: a frame skeleton with its
     configuration in the attribute, drawn once and never listened to. */
  function stills(data) {
    document.querySelectorAll("[data-frame]").forEach(function (el) {
      var config;
      try {
        config = JSON.parse(el.getAttribute("data-frame"));
      } catch (e) {
        return;
      }
      var screen = el.querySelector("[data-screen]") || el;
      var plot = el.querySelector("[data-plot-cpu], [data-plot-mem], [data-plot-wait]");
      var cols = config.cols;
      if (!cols && plot) {
        cols = measure(
          screen,
          plot,
          el.querySelector("[data-gutter-cpu], [data-gutter-mem], [data-gutter-wait]"),
          config.floor || 24,
          config.ceiling || 160
        );
      }
      config.cols = cols || 40;
      config.bare = config.bare !== false;
      try {
        draw(el, data, config);
        el.setAttribute("data-drawn", "true");
      } catch (e) {
        el.setAttribute("data-drawn", "failed");
      }
    });
  }

  return {
    WARN: WARN,
    CRIT: CRIT,
    braille: braille,
    gutter: gutter,
    bar: bar,
    meter: meter,
    sparkline: sparkline,
    statusClass: statusClass,
    clock: clock,
    bytes: bytes,
    escapeHtml: escapeHtml,
    measure: measure,
    draw: draw,
    order: order,
    stills: stills,
  };
})();
