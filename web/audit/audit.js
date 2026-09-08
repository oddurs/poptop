const { chromium } = require("playwright-core");
const BASE = process.env.BASE || "http://127.0.0.1:3000";

// playwright-core drives a browser rather than shipping one. `channel` finds
// the installed Chrome on any platform; CHROME overrides it where the binary is
// somewhere unusual, which is how this runs on a CI image.
const LAUNCH = process.env.CHROME
  ? { executablePath: process.env.CHROME }
  : { channel: "chrome" };
const fs = require("fs");

const OUT = process.env.OUT || ".";

const PAGES = [
  ["home", "/"],
  ["docs-index", "/docs"],
  ["docs-keys", "/docs/keys"],
  ["docs-colour", "/docs/design/colour"],
  ["design", "/design"],
  ["community", "/community"],
  ["contributing", "/community/contributing"],
  ["roadmap", "/roadmap"],
  ["notfound", "/does-not-exist"],
];

const VIEWPORTS = [
  ["desktop", 1440, 900],
  ["laptop", 1180, 800],
  ["tablet", 820, 1180],
  ["phone", 390, 844],
];

// ── in-page audit ───────────────────────────────────────────────────────────
const AUDIT = () => {
  const problems = [];
  const add = (kind, detail, el) =>
    problems.push({
      kind,
      detail,
      where: el
        ? el.tagName.toLowerCase() +
          (el.id ? "#" + el.id : "") +
          (typeof el.className === "string" && el.className
            ? "." + el.className.trim().split(/\s+/).slice(0, 3).join(".")
            : "")
        : null,
      text: el ? (el.textContent || "").trim().slice(0, 60) : null,
    });

  const vw = document.documentElement.clientWidth;

  // 1. horizontal overflow of the document
  if (document.documentElement.scrollWidth > vw + 1) {
    add("overflow-document", `scrollWidth ${document.documentElement.scrollWidth} > ${vw}`);
    document.querySelectorAll("*").forEach((el) => {
      const r = el.getBoundingClientRect();
      if (r.width === 0) return;
      if (r.right > vw + 1 && !el.closest("[data-screen], .table-scroll, pre, table")) {
        add("overflow-element", `right ${Math.round(r.right)} > ${vw}`, el);
      }
    });
  }

  // 2. contrast
  const parseRgb = (s) => {
    const m = s.match(/rgba?\(([^)]+)\)/);
    if (!m) return null;
    const p = m[1].split(/[,\s/]+/).filter(Boolean).map(Number);
    return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 };
  };
  const lum = (c) => {
    const f = (v) => {
      v /= 255;
      return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
    };
    return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b);
  };
  const over = (fg, bg) => {
    if (fg.a >= 1) return fg;
    return {
      r: fg.r * fg.a + bg.r * (1 - fg.a),
      g: fg.g * fg.a + bg.g * (1 - fg.a),
      b: fg.b * fg.a + bg.b * (1 - fg.a),
      a: 1,
    };
  };
  const bgOf = (el) => {
    let n = el;
    while (n && n !== document.documentElement) {
      const c = parseRgb(getComputedStyle(n).backgroundColor);
      if (c && c.a > 0.95) return c;
      n = n.parentElement;
    }
    return parseRgb(getComputedStyle(document.body).backgroundColor) || { r: 255, g: 255, b: 255, a: 1 };
  };

  const seen = new Set();
  document.querySelectorAll("p,a,span,li,td,th,h1,h2,h3,h4,button,summary,code,kbd,figcaption,div").forEach((el) => {
    const direct = Array.from(el.childNodes).some(
      (n) => n.nodeType === 3 && n.textContent.trim().length > 1
    );
    if (!direct) return;
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) return;
    const cs = getComputedStyle(el);
    if (cs.visibility === "hidden" || cs.opacity === "0") return;
    // The terminal frame is a picture of a terminal; its own palette is under
    // test elsewhere, in the tool's CI.
    if (el.closest("[data-demo]")) return;
    const fg0 = parseRgb(cs.color);
    if (!fg0) return;
    const bg = bgOf(el);
    const fg = over(fg0, bg);
    const l1 = lum(fg), l2 = lum(bg);
    const ratio = (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
    const px = parseFloat(cs.fontSize);
    const bold = parseInt(cs.fontWeight, 10) >= 700;
    const large = px >= 24 || (px >= 18.66 && bold);
    const floor = large ? 3 : 4.5;
    const key = cs.color + "|" + cs.fontSize + "|" + cs.fontWeight;
    if (ratio < floor && !seen.has(key)) {
      seen.add(key);
      add("contrast", `${ratio.toFixed(2)}:1 (needs ${floor}) ${cs.color} on rgb(${Math.round(bg.r)},${Math.round(bg.g)},${Math.round(bg.b)}) at ${cs.fontSize}/${cs.fontWeight}`, el);
    }
  });

  // 3. heading order
  const heads = [...document.querySelectorAll("h1,h2,h3,h4,h5,h6")].filter(
    (h) => h.getBoundingClientRect().height > 0
  );
  const h1s = heads.filter((h) => h.tagName === "H1");
  if (h1s.length !== 1) add("heading-h1-count", `${h1s.length} visible h1`);
  let prev = 0;
  heads.forEach((h) => {
    const lvl = +h.tagName[1];
    if (prev && lvl > prev + 1) add("heading-skip", `h${prev} then h${lvl}`, h);
    prev = lvl;
  });

  // 4. accessible names
  document.querySelectorAll("a,button").forEach((el) => {
    const r = el.getBoundingClientRect();
    if (r.width === 0) return;
    const name =
      (el.getAttribute("aria-label") || "") + (el.textContent || "") + (el.getAttribute("title") || "");
    if (!name.trim()) add("no-accessible-name", el.tagName, el);
  });
  document.querySelectorAll("img").forEach((el) => {
    if (el.getAttribute("alt") === null) add("img-no-alt", el.src, el);
  });

  // 5. tap targets (WCAG 2.2 AA: 24x24 css px, unless spaced)
  if (window.innerWidth < 700) {
    document.querySelectorAll("a,button,summary,[tabindex]").forEach((el) => {
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) return;
      if (el.closest("[data-demo], .prose, .skip")) return;
      // WCAG 2.2 SC 2.5.8 exempts a target sitting in a sentence, where its
      // size is set by the line-height of the text around it. Detect that as
      // "the parent has text of its own alongside this link".
      const parent = el.parentElement;
      const inSentence =
        parent &&
        Array.from(parent.childNodes).some(
          (n) => n.nodeType === 3 && n.textContent.trim().length > 1
        );
      if (inSentence) return;
      if (r.height < 24 || r.width < 24) {
        add("tap-target", `${Math.round(r.width)}×${Math.round(r.height)}`, el);
      }
    });
  }

  // 6. line length. The design system targets under 80 characters and states
  //    the measure in ems, because `ch` is the advance of "0" and this face
  //    sets digits far wider than its average letter — a `66ch` column here
  //    held 103 characters.
  //
  //    Measured off the widest *line box*, via a Range, rather than the
  //    element's width: a row laid out in two columns is not one long line, and
  //    an earlier version of this check reported every link list as a
  //    violation.
  document.querySelectorAll("p, li, dd").forEach((el) => {
    if (el.closest("[data-demo], [data-frame], pre, .caption, figcaption")) return;
    const cs = getComputedStyle(el);
    if (cs.display === "none") return;
    const text = el.textContent.trim();
    if (text.length < 90) return;
    // Skip anything that lays its own children out in columns: a two-column row
    // is not one long line, and a range spanning both reports their union.
    const laidOut = [...el.children].some((child) => {
      const d = getComputedStyle(child).display;
      return d === "flex" || d === "grid" || d === "block" || d === "table";
    });
    if (laidOut) return;

    const range = document.createRange();
    range.selectNodeContents(el);
    const rects = [...range.getClientRects()].filter((r) => r.width > 20 && r.height > 4);
    if (!rects.length) return;
    const widest = Math.max(...rects.map((r) => r.width));

    const c = document.createElement("canvas").getContext("2d");
    c.font = `${cs.fontStyle} ${cs.fontWeight} ${parseFloat(cs.fontSize)}px ${cs.fontFamily}`;
    const avg = c.measureText(text.slice(0, 500)).width / Math.min(text.length, 500);
    if (!avg) return;

    const chars = Math.round(widest / avg);
    if (chars > 80) add("line-length", `${chars} characters a line`, el);
  });

  // 7. text that is clipped by its own box
  document.querySelectorAll("*").forEach((el) => {
    if (el.children.length) return;
    const cs = getComputedStyle(el);
    if (cs.overflow === "visible" || cs.overflowX === "auto" || cs.overflowX === "scroll") return;
    if (el.closest(".sr-only") || el.classList.contains("sr-only")) return;
    if (el.scrollWidth > el.clientWidth + 2 && cs.textOverflow !== "ellipsis") {
      add("clipped-text", `${el.scrollWidth} > ${el.clientWidth}`, el);
    }
  });

  return problems;
};

(async () => {
  const browser = await chromium.launch(LAUNCH);
  const report = [];

  for (const [vname, w, h] of VIEWPORTS) {
    for (const scheme of ["dark", "light"]) {
      const ctx = await browser.newContext({
        viewport: { width: w, height: h },
        deviceScaleFactor: 2,
        colorScheme: scheme,
        reducedMotion: "no-preference",
      });
      const page = await ctx.newPage();
      const console_ = [];
      page.on("console", (m) => m.type() === "error" && console_.push(m.text()));
      page.on("pageerror", (e) => console_.push("pageerror: " + e.message));
      const failed = [];
      page.on("requestfailed", (r) => failed.push(r.url() + " " + (r.failure() || {}).errorText));
      page.on("response", (r) => r.status() >= 400 && r.status() !== 404 && failed.push(r.url() + " " + r.status()));

      for (const [name, path] of PAGES) {
        // The frame animates; freeze it so screenshots are comparable.
        await page.goto(BASE + path, { waitUntil: "networkidle" });
        await page.evaluate(() => document.fonts.ready);
        await page.evaluate(() => {
          const d = document.querySelector("[data-demo]");
          if (d) d.dispatchEvent(new KeyboardEvent("keydown", { key: " ", bubbles: true }));
        });
        await page.waitForTimeout(250);

        const problems = await page.evaluate(AUDIT);
        report.push({ view: vname, scheme, page: name, path, problems, console: [...console_], failed: [...failed] });
        console_.length = 0;
        failed.length = 0;

        if (vname === "desktop" || (vname === "phone" && scheme === "dark")) {
          await page.screenshot({
            path: `${OUT}/shot-${name}-${vname}-${scheme}.png`,
            fullPage: name !== "home",
          });
        }
      }
      await ctx.close();
    }
  }

  await browser.close();
  fs.writeFileSync(`${OUT}/report.json`, JSON.stringify(report, null, 1));

  // summary
  const counts = {};
  report.forEach((r) =>
    r.problems.forEach((p) => {
      const k = p.kind;
      counts[k] = (counts[k] || 0) + 1;
    })
  );
  console.log("\n=== findings ===");
  Object.entries(counts).sort((a, b) => b[1] - a[1]).forEach(([k, v]) => console.log(`${String(v).padStart(4)}  ${k}`));
  const errs = report.flatMap((r) => r.console.concat(r.failed));
  console.log(`${String(errs.length).padStart(4)}  console errors / failed requests`);
  errs.slice(0, 10).forEach((e) => console.log("      " + e));
})();
