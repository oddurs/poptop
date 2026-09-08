const { chromium } = require("playwright-core");

// playwright-core drives a browser rather than shipping one. `channel` finds
// the installed Chrome on any platform; CHROME overrides it where the binary is
// somewhere unusual, which is how this runs on a CI image.
const LAUNCH = process.env.CHROME
  ? { executablePath: process.env.CHROME }
  : { channel: "chrome" };
const BASE = "http://127.0.0.1:3000";

let failures = 0;
const ok = (label, cond, extra = "") => {
  if (!cond) failures++;
  console.log(`${cond ? "  ok  " : "FAIL  "}${label}${extra ? "   " + extra : ""}`);
};

(async () => {
  const browser = await chromium.launch(LAUNCH);
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(BASE + "/", { waitUntil: "networkidle" });

  // The page now holds seven frames. Everything below is about the hero, so
  // every selector is scoped to it — an unscoped `[data-plot-cpu]` matches all
  // of them and Playwright rightly refuses to guess.
  const HERO = "#hero-frame ";
  const text = (sel) => page.locator(HERO + sel).innerText();
  // Everything below redraws on the next animation frame, so read after one.
  const settled = async () => {
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  };
  const pause = async () => {
    await page.locator("#hero-frame").focus();
    if ((await text("[data-state]")) === "LIVE") await page.keyboard.press("Space");
    await settled();
  };

  console.log("\n— the frame —");
  ok("draws five process rows", (await page.locator(HERO + "[data-procs] tr").count()) === 5);
  ok("draws braille", /[⠀-⣿]/.test(await text("[data-plot-cpu]")));

  await pause();
  ok("Space pauses", (await text("[data-state]")) === "PAUSED", await text("[data-state]"));

  const b1 = await text("[data-offset]");
  await page.keyboard.press("ArrowLeft");
  await settled();
  const a1 = await text("[data-offset]");
  ok("← scrubs back", b1 !== a1, `${b1} -> ${a1}`);

  await page.keyboard.press("ArrowRight");
  await settled();
  ok("→ scrubs forward", (await text("[data-offset]")) === b1, `${a1} -> ${await text("[data-offset]")}`);

  await page.keyboard.down("Shift");
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.up("Shift");
  await settled();
  const ten = await text("[data-offset]");
  ok("Shift+← jumps ten, not one", ten !== a1 && ten !== b1, `${b1} -> ${ten}`);

  const s1 = await text("[data-span]");
  await page.keyboard.press("-");
  await settled();
  const s2 = await text("[data-span]");
  ok("- zooms out", /2s\/slot/.test(s2), s2.split("·")[0].trim());
  await page.keyboard.press("+");
  await settled();
  ok("+ zooms back in", (await text("[data-span]")) === s1);

  await page.keyboard.press("Home");
  await settled();
  const oldest = await text("[data-offset]");
  ok("Home jumps to the oldest sample", oldest !== ten, oldest);
  await page.keyboard.press("End");
  await settled();
  ok("End jumps to the newest", (await text("[data-offset]")) === "-0s", await text("[data-offset]"));

  await pause();
  const box = await page.locator(HERO + "[data-screen]").boundingBox();
  const o1 = await text("[data-offset]");
  await page.mouse.move(box.x + box.width * 0.6, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.3, box.y + box.height / 2, { steps: 12 });
  await page.mouse.up();
  await settled();
  ok("dragging the plot scrubs", o1 !== (await text("[data-offset]")), `${o1} -> ${await text("[data-offset]")}`);

  await pause();
  const beforeBtn = await text("[data-offset]");
  await page.locator(HERO + '[data-act="back"]').click();
  await settled();
  ok("the ← button scrubs", beforeBtn !== (await text("[data-offset]")));
  await page.locator(HERO + '[data-act="live"]').click();
  await settled();
  ok("the live button resumes", (await text("[data-state]")) === "LIVE", await text("[data-state]"));

  // A reader on a screen reader gets a spoken readout of the paused sample.
  await pause();
  await page.keyboard.press("ArrowLeft");
  await settled();
  const said = await page.locator("[data-say]").first().evaluate((el) => el.textContent);
  ok("announces the paused sample", /CPU \d+ percent/.test(said), said.slice(0, 64));

  console.log("\n— the rest of the page —");
  await page.locator("[data-copy]").first().click();
  await page.waitForTimeout(150);
  ok("copy button confirms", (await page.locator("[data-copy]").first().getAttribute("data-copied")) === "true");

  const t1 = await page.getAttribute("html", "data-theme");
  await page.locator("[data-theme-toggle]").click();
  const t2 = await page.getAttribute("html", "data-theme");
  ok("theme toggle changes theme", t1 !== t2, `${t1} -> ${t2}`);
  await page.reload({ waitUntil: "networkidle" });
  ok("theme survives a reload", (await page.getAttribute("html", "data-theme")) === t2);

  await page.goto(BASE + "/docs/keys", { waitUntil: "networkidle" });
  await page.keyboard.press("Tab");
  ok("first Tab reaches the skip link", (await page.evaluate(() => document.activeElement.className)).includes("skip"));
  ok("skip link targets main", (await page.evaluate(() => document.activeElement.getAttribute("href"))) === "#main");

  const ring = await page.evaluate(() => {
    document.querySelector(".nav a").focus();
    const cs = getComputedStyle(document.activeElement);
    return cs.outlineStyle + " " + cs.outlineWidth;
  });
  ok("focus ring is drawn", !ring.startsWith("none"), ring);

  // The on-page contents marks what you are reading.
  await page.goto(BASE + "/docs/design/colour", { waitUntil: "networkidle" });
  let marked = true;
  for (const y of [0, 800, 1600, 2600, 4000]) {
    await page.evaluate((y) => window.scrollTo(0, y), y);
    await page.waitForTimeout(250);
    const n = await page.locator("[data-toc] a[data-reading]").count();
    if (n !== 1) { marked = false; console.log(`        at ${y}px: ${n} marked`); }
  }
  ok("contents marks exactly one heading at every scroll depth", marked);

  console.log(`\n  page errors: ${errors.length}`);
  errors.slice(0, 3).forEach((e) => console.log("    " + e));
  console.log(`  failures: ${failures}`);
  await browser.close();
  process.exit(failures ? 1 : 0);
})();
