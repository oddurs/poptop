/* The landing page's sections, checked in a real browser.
   ---------------------------------------------------------------------------
   Every section below the hero argues with a picture, and a picture that failed
   to draw is not an empty state — it is a section making a claim with nothing
   behind it. These are the assertions that would fail if one broke. */
const { chromium } = require("playwright-core");

// playwright-core drives a browser rather than shipping one. `channel` finds
// the installed Chrome on any platform; CHROME overrides it where the binary is
// somewhere unusual, which is how this runs on a CI image.
const LAUNCH = process.env.CHROME
  ? { executablePath: process.env.CHROME }
  : { channel: "chrome" };

const BASE = process.env.BASE || "http://127.0.0.1:3000";

let failures = 0;
const ok = (label, cond, extra = "") => {
  if (!cond) failures++;
  console.log(`${cond ? "  ok  " : "FAIL  "}${label}${extra ? "   " + extra : ""}`);
};

(async () => {
  const browser = await chromium.launch(LAUNCH);
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 1000 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));

  await page.goto(BASE + "/", { waitUntil: "networkidle" });
  await page.waitForTimeout(3200); // let the hero's one moment finish

  console.log("\n— every picture drew —");
  const stills = await page.evaluate(() =>
    [...document.querySelectorAll("[data-frame]")].map((el) => ({
      drawn: el.getAttribute("data-drawn"),
      plot: (el.querySelector("[data-plot-cpu],[data-plot-mem],[data-plot-wait],[data-plot-spike]") || {})
        .textContent || "",
      rows: el.querySelectorAll("[data-procs] tr").length,
      wantsTable: !!el.querySelector("[data-procs]"),
      // One still on the page is meant to be blank: the closing frame, which is
      // what poptop looks like a second after you start it. It says so.
      empty: (JSON.parse(el.getAttribute("data-frame") || "{}") || {}).empty === true,
    }))
  );
  ok("every section's still is present", stills.length >= 7, `${stills.length} stills`);
  stills.forEach((s, i) => {
    ok(`still ${i} drew`, s.drawn === "true", s.drawn);
    // Braille, not an empty box. U+2800 alone is blank, so require filled cells
    // — except where blankness is the point, which is asserted the other way.
    if (s.empty) {
      ok(`still ${i} is blank, as the closing frame should be`, !/[⠁-⣿]/.test(s.plot));
    } else {
      ok(`still ${i} has a plot with data in it`, /[⠁-⣿]/.test(s.plot));
      if (s.wantsTable) ok(`still ${i} listed processes`, s.rows > 0, `${s.rows} rows`);
    }
  });

  console.log("\n— the sections say what they claim —");
  const ids = await page.evaluate(() =>
    [...document.querySelectorAll("main section.section")].map((s) => s.id)
  );
  ["problem", "start", "table", "aggregation", "colour", "versus", "keys", "install"].forEach((id) =>
    ok(`section ${id} exists`, ids.includes(id))
  );

  // Peak versus mean is only an argument if the two plots differ.
  const [meanPlot, peakPlot] = await page.evaluate(() =>
    [...document.querySelectorAll("#aggregation [data-plot-spike]")].map((e) => e.textContent)
  );
  ok("mean and peak draw different pictures", meanPlot !== peakPlot);
  const filled = (s) => (s.match(/[⡀-⣿]/g) || []).length;
  ok("peak keeps more than mean", filled(peakPlot) > filled(meanPlot) * 1.3,
    `peak ${filled(peakPlot)} vs mean ${filled(meanPlot)} filled cells`);

  console.log("\n— the hero's one moment —");
  ok("the hero ended on the incident", (await page.getAttribute("[data-hero-caption]", "data-moment")) === "then");
  const animated = await page.evaluate(() =>
    [...document.querySelectorAll("*")].filter((el) => {
      const cs = getComputedStyle(el);
      return cs.animationName !== "none" && cs.animationIterationCount !== "1";
    }).length
  );
  ok("nothing else on the page is animating", animated === 0, `${animated} looping animations`);

  console.log("\n— colour vision —");
  for (const state of ["protan", "deutan", "mono", "normal"]) {
    await page.click(`[data-vision-set="${state}"]`);
    await page.waitForTimeout(400); // past the chip transition
    const r = await page.evaluate(() => ({
      vision: document.querySelector("[data-cvd]").getAttribute("data-vision"),
      chips: [...document.querySelectorAll("[data-swatch]")].map((e) => getComputedStyle(e).backgroundColor),
      conv: document.querySelector('[data-delta="convention"]').textContent,
      pop: document.querySelector('[data-delta="poptop"]').textContent,
      frameOk: getComputedStyle(document.querySelector("[data-cvd-frame]")).getPropertyValue("--ok").trim(),
    }));
    ok(`${state} applies`, r.vision === state);
    if (state === "protan") {
      const [g, y] = r.chips;
      ok("the convention's green and yellow collapse together", g === y || near(g, y),
        `${g} vs ${y}`);
      ok("poptop's stay apart", !near(r.chips[3], r.chips[4]));
      ok("the convention is reported under the floor", parseFloat(r.conv) < 8, r.conv);
      ok("poptop is reported above it", parseFloat(r.pop) >= 8, r.pop);
    }
    if (state === "mono") {
      const grey = r.chips.every((c) => {
        const [a, b, d] = c.match(/\d+/g).map(Number);
        return a === b && b === d;
      });
      ok("monochrome leaves no hue at all", grey);
      ok("the frame is monochrome too", /^#(\w\w)\1\1$/.test(r.frameOk), r.frameOk);
      ok("no separation figure is claimed", r.conv === "—" && r.pop === "—", `${r.conv} / ${r.pop}`);
    }
  }

  // The control is a radio group: one tab stop, arrows within it.
  await page.focus('[data-vision-set="normal"]');
  await page.keyboard.press("ArrowRight");
  await page.waitForTimeout(200);
  ok("arrow keys move within the control",
    (await page.getAttribute("[data-cvd]", "data-vision")) === "protan");
  const marked = await page.evaluate(() => {
    const on = document.querySelector('[aria-checked="true"]');
    const cs = getComputedStyle(on);
    // Marked by fill and weight, not only by hue — in this section above all.
    return cs.backgroundColor !== "rgba(0, 0, 0, 0)" && parseInt(cs.fontWeight, 10) >= 500;
  });
  ok("the current state is marked by more than colour", marked);

  console.log("\n— the second scrubbable frame —");
  const off = () => page.locator("#table-frame [data-procs] tr:first-child").innerText();
  await page.locator("#table-frame").focus();
  const before = await off();
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.press("ArrowLeft");
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  ok("the table frame scrubs by keyboard", before !== (await off()));

  console.log("\n— the keymap drives the hero —");
  const heroOffset = () => page.locator("#hero-frame [data-offset]").innerText();
  const b4 = await heroOffset();
  await page.locator('.keymap [data-act-for="hero-frame"][data-act="back"]').click();
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  ok("a keycap in the keymap scrubs the frame above", b4 !== (await heroOffset()),
    `${b4} -> ${await heroOffset()}`);

  console.log(`\n  page errors: ${errors.filter((e) => !e.includes("404")).length}`);
  console.log(`  failures: ${failures}`);
  await browser.close();
  process.exit(failures ? 1 : 0);
})();

function near(a, b) {
  const [x] = [a, b].map((c) => c.match(/\d+/g).map(Number));
  const y = b.match(/\d+/g).map(Number);
  return Math.abs(x[0] - y[0]) + Math.abs(x[1] - y[1]) + Math.abs(x[2] - y[2]) < 40;
}
