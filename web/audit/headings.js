const { chromium } = require("playwright-core");

// playwright-core drives a browser rather than shipping one. `channel` finds
// the installed Chrome on any platform; CHROME overrides it where the binary is
// somewhere unusual, which is how this runs on a CI image.
const BASE = process.env.BASE || BASE;
const LAUNCH = process.env.CHROME
  ? { executablePath: process.env.CHROME }
  : { channel: "chrome" };
const PAGES = ["/", "/docs", "/docs/keys", "/docs/design/colour", "/design", "/community", "/community/contributing", "/roadmap", "/nope"];
let bad = 0;
(async () => {
  const b = await chromium.launch(LAUNCH);
  const p = await (await b.newContext({ viewport: { width: 1280, height: 900 } })).newPage();
  for (const path of PAGES) {
    await p.goto(BASE + path, { waitUntil: "networkidle" });
    const rows = await p.evaluate(() => {
      const body = parseFloat(getComputedStyle(document.body).fontSize);
      return [...document.querySelectorAll("h1,h2,h3,h4")]
        .filter((h) => h.getBoundingClientRect().height > 0)
        .map((h) => {
          const cs = getComputedStyle(h);
          const size = parseFloat(cs.fontSize);
          const weight = parseInt(cs.fontWeight, 10);
          // A heading that is neither larger nor heavier than body text is not
          // rendering as a heading, whatever the markup says.
          // Some headings on this site are deliberately small — a label on a
          // hairline is still the section's heading. The failure being caught
          // is a heading that is neither larger nor heavier than body text,
          // which is what a heading looks like when its rule was dropped.
          const looksLikeBody = size <= body && weight < 500;
          return { tag: h.tagName, cls: h.className, size, weight, looksLikeBody,
                   text: h.textContent.trim().slice(0, 30) };
        });
    });
    const broken = rows.filter((r) => r.looksLikeBody);
    if (broken.length) {
      bad += broken.length;
      console.log(`FAIL ${path}`);
      broken.forEach((r) => console.log(`       ${r.tag}.${r.cls} ${r.size}px/${r.weight} « ${r.text} »`));
    } else {
      const h1 = rows.find((r) => r.tag === "H1");
      console.log(`  ok ${path.padEnd(26)} ${rows.length} headings, h1 ${h1 ? h1.size + "px/" + h1.weight : "—"}`);
    }
  }
  console.log(bad ? `\n${bad} headings rendering as body text` : "\nevery heading renders as a heading");
  await b.close();
  process.exit(bad ? 1 : 0);
})();
