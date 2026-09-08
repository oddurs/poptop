/* The colour-vision control.
   ---------------------------------------------------------------------------
   Every colour it shows was simulated on the server by `src/cvd.rs`, which is
   pinned against the tool's own implementation. This file swaps between
   palettes it was handed and computes nothing, so there is no third copy of the
   Machado matrices to drift from the other two. */
(function () {
  "use strict";

  var root = document.querySelector("[data-cvd]");
  var payload = document.getElementById("cvd-palettes");
  if (!root || !payload) return;

  var palettes;
  try {
    palettes = JSON.parse(payload.textContent);
  } catch (e) {
    return;
  }

  var options = [].slice.call(root.querySelectorAll("[data-vision-set]"));
  var note = root.querySelector("[data-cvd-note]");

  function apply(key) {
    var set = palettes[key];
    if (!set) return;

    root.setAttribute("data-vision", key);
    if (note) note.textContent = set.note;

    ["convention", "poptop"].forEach(function (row) {
      var data = set[row];
      if (!data) return;
      data.chips.forEach(function (hex, i) {
        var chip = root.querySelector('[data-swatch="' + row + "-" + i + '"]');
        if (!chip) return;
        chip.style.setProperty("--chip", hex);
        // The label colour is chosen per chip, on the server, by luminance —
        // a fixed one would be unreadable on at least one simulated colour.
        if (data.inks) chip.style.setProperty("--chip-ink", data.inks[i]);
      });
      var delta = root.querySelector('[data-delta="' + row + '"]');
      if (delta) {
        delta.textContent = data.delta;
        // Below 8 the two closest colours are one colour. Said with a class as
        // well as a number, because this section is about not trusting hue.
        // `—` parses to NaN, and NaN < 8 is false, so the monochrome state
        // reports nothing rather than reporting a failure.
        delta.parentElement.setAttribute("data-under", parseFloat(data.delta) < 8 ? "true" : "false");
      }
    });

    // The frame beside the swatches is simulated too. A plot that stayed
    // honest-looking while the swatches changed would be arguing against the
    // section it sits in.
    if (set.frame) {
      var names = ["--ok", "--warn", "--critical", "--series-cpu", "--series-mem"];
      var scope = document.querySelector("[data-cvd-frame]");
      if (scope) {
        names.forEach(function (name, i) {
          scope.style.setProperty(name, set.frame[i]);
        });
      }
    }

    options.forEach(function (b) {
      var on = b.getAttribute("data-vision-set") === key;
      b.setAttribute("aria-checked", on ? "true" : "false");
      b.tabIndex = on ? 0 : -1;
    });
  }

  options.forEach(function (b, i) {
    b.addEventListener("click", function () {
      apply(b.getAttribute("data-vision-set"));
      b.focus();
    });
    // A radio group is one tab stop; the arrows move within it.
    b.tabIndex = i === 0 ? 0 : -1;
    b.addEventListener("keydown", function (ev) {
      var step = ev.key === "ArrowRight" || ev.key === "ArrowDown" ? 1
        : ev.key === "ArrowLeft" || ev.key === "ArrowUp" ? -1 : 0;
      if (!step) return;
      ev.preventDefault();
      var next = options[(i + step + options.length) % options.length];
      apply(next.getAttribute("data-vision-set"));
      next.focus();
    });
  });
})();
