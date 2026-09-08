const { chromium } = require("playwright-core");

// playwright-core drives a browser rather than shipping one. `channel` finds
// the installed Chrome on any platform; CHROME overrides it where the binary is
// somewhere unusual, which is how this runs on a CI image.
const LAUNCH = process.env.CHROME
  ? { executablePath: process.env.CHROME }
  : { channel: "chrome" };
const BASE = "http://127.0.0.1:3000";

(async () => {
  const browser = await chromium.launch(LAUNCH);
  for (const [name, path] of [["home", "/"], ["docs", "/docs/design/colour"]]) {
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    const page = await ctx.newPage();
    // Slow the wire down so the swap window is real rather than instantaneous.
    const cdp = await ctx.newCDPSession(page);
    await cdp.send("Network.enable");
    await cdp.send("Network.emulateNetworkConditions", {
      offline: false, latency: 150, downloadThroughput: (1.6 * 1024 * 1024) / 8, uploadThroughput: 750 * 1024 / 8,
    });

    await page.addInitScript(() => {
      window.__cls = 0;
      new PerformanceObserver((l) => {
        for (const e of l.getEntries()) if (!e.hadRecentInput) window.__cls += e.value;
      }).observe({ type: "layout-shift", buffered: true });
    });

    const bytes = { doc: 0, css: 0, js: 0, font: 0 };
    page.on("response", async (r) => {
      const u = r.url();
      let n = 0;
      try { n = (await r.body()).length; } catch (e) {}
      if (u.endsWith(".woff2")) bytes.font += n;
      else if (u.endsWith(".css")) bytes.css += n;
      else if (u.endsWith(".js")) bytes.js += n;
      else if (r.request().resourceType() === "document") bytes.doc += n;
    });

    await page.goto(BASE + path, { waitUntil: "load" });
    await page.waitForTimeout(2500);

    const m = await page.evaluate(() => {
      const nav = performance.getEntriesByType("navigation")[0];
      const paints = {};
      performance.getEntriesByType("paint").forEach((p) => (paints[p.name] = Math.round(p.startTime)));
      const font = performance.getEntriesByType("resource").filter((r) => r.name.endsWith(".woff2"));
      return {
        cls: +window.__cls.toFixed(4),
        fcp: paints["first-contentful-paint"],
        domContentLoaded: Math.round(nav.domContentLoadedEventEnd),
        loaded: Math.round(nav.loadEventEnd),
        transferDoc: nav.transferSize,
        fonts: font.map((f) => ({ at: Math.round(f.startTime), took: Math.round(f.duration) })),
        loadedFaces: [...document.fonts].filter((f) => f.status === "loaded").length,
      };
    });
    console.log(`\n${name}  (1.6 Mbps, 150 ms RTT)`);
    console.log(`  CLS ${m.cls}   FCP ${m.fcp}ms   DCL ${m.domContentLoaded}ms   load ${m.loaded}ms`);
    console.log(`  document ${(bytes.doc/1024).toFixed(1)} KB raw / ${(m.transferDoc/1024).toFixed(1)} KB on the wire`);
    console.log(`  css ${(bytes.css/1024).toFixed(1)} KB   js ${(bytes.js/1024).toFixed(1)} KB   fonts ${(bytes.font/1024).toFixed(1)} KB`);
    console.log(`  font requests: ${JSON.stringify(m.fonts)}  faces loaded: ${m.loadedFaces}`);
    await ctx.close();
  }
  await browser.close();
})();
