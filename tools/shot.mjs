// Headless-browser screenshot + console capture, for verifying the engine
// actually renders. Usage: node tools/shot.mjs [url] [outfile] [waitMs]
import { chromium } from "playwright";

const url = process.argv[2] || "http://localhost:8137/";
const out = process.argv[3] || "tools/shot.png";
const waitMs = Number(process.argv[4] || 3500);

// BACKEND=webgpu (default) drives Dawn's software Vulkan; BACKEND=webgl drives
// software GL (SwiftShader). HEADED=1 launches the full Chromium (run it under
// xvfb-run) instead of chrome-headless-shell, which can't composite WebGPU.
const backend = process.env.BACKEND || "webgpu";
const headed = process.env.HEADED === "1";
const args = [
  "--no-sandbox",
  "--disable-gpu-sandbox",
  "--enable-unsafe-swiftshader",
  "--ignore-gpu-blocklist",
];
if (backend === "webgl") {
  args.push("--use-gl=swiftshader", "--disable-features=WebGPU");
} else {
  args.push("--enable-unsafe-webgpu");
}

const browser = await chromium.launch({ headless: !headed, args });

const page = await browser.newPage({ viewport: { width: 900, height: 760 } });
page.on("console", (m) => console.log("PAGE>", m.type(), m.text()));
page.on("pageerror", (e) => console.log("PAGEERR>", e.message));

await page.goto(url, { waitUntil: "load" });

// CLICK="a.card" left-clicks a selector and follows the same-tab navigation
// (to verify that plain <a href> links work).
const click = process.env.CLICK;
if (click) {
  await page.click(click);
  await page.waitForLoadState("load");
  console.log("NAV>", page.url());
}

// HOLD="KeyS,ArrowUp" holds keys down; TAP="Space" presses a key repeatedly
// (e.g. to keep a flappy bird aloft). Both need the canvas focused.
const hold = process.env.HOLD;
const tap = process.env.TAP;
const press = process.env.PRESS;
if (hold || tap || press) {
  await page
    .locator("#octiron-canvas")
    .click({ position: { x: 400, y: 540 } })
    .catch(() => {});
}
// PRESS="Space" taps a key once (held a few frames), e.g. to start then idle.
if (press) {
  await page.keyboard.down(press);
  await page.waitForTimeout(80);
  await page.keyboard.up(press);
}
if (hold) {
  for (const key of hold.split(",")) await page.keyboard.down(key);
}

if (tap) {
  // Hold each tap for a few frames so per-frame rising-edge detection sees it.
  const interval = 380;
  const holdMs = 90;
  let elapsed = 0;
  while (elapsed < waitMs) {
    await page.keyboard.down(tap);
    await page.waitForTimeout(holdMs);
    await page.keyboard.up(tap);
    await page.waitForTimeout(Math.max(0, interval - holdMs));
    elapsed += interval;
  }
} else {
  await page.waitForTimeout(waitMs);
}

// ELEMENT="#sel" screenshots just that element; FULLPAGE=1 captures the whole
// scrolled page (for the landing menu).
const element = process.env.ELEMENT;
if (element) {
  await page.locator(element).screenshot({ path: out });
} else {
  await page.screenshot({ path: out, fullPage: process.env.FULLPAGE === "1" });
}
await browser.close();
console.log("screenshot ->", out);
