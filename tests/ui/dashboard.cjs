// Optional browser smoke test. Requires a running UI on an EMPTY-HARDWARE host.
// npm install --prefix .tools/ui-tests playwright-core
// Supply RFV_BASE_URL, RFV_UI_TOKEN and RFV_CHROMIUM_EXECUTABLE.
// Test input values are algorithm assumptions, never RF captures.
const { chromium } = require("playwright-core");
const assert = require("node:assert/strict");
(async () => {
  const browser = await chromium.launch({
    executablePath: process.env.RFV_CHROMIUM_EXECUTABLE,
    args: ["--no-sandbox", "--disable-dev-shm-usage"],
    headless: true,
  });
  try {
    const page = await browser.newPage({
      viewport: { width: 1440, height: 1000 },
    });
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.goto(process.env.RFV_BASE_URL || "http://127.0.0.1:8080");
    await page.fill("#token", process.env.RFV_UI_TOKEN || "");
    await page.click("#connect");
    await page.waitForFunction(() =>
      document.getElementById("error").textContent.includes("DEVICE_NOT_FOUND"),
    );
    for (const name of [
      "Live Monitor",
      "CSI Research",
      "RF Coverage",
      "Link Budget",
      "RF Optimizer",
      "Range Extenders",
      "Hardware",
      "Diagnostics",
      "Recordings",
    ]) {
      await page.getByRole("button", { name, exact: true }).click();
    }
    await page
      .getByRole("button", { name: "Link Budget", exact: true })
      .click();
    const values = {
      tx_power: 20,
      tx_gain: 2,
      rx_gain: 0,
      tx_loss: 1,
      rx_loss: 0,
      distance: 100,
      frequency: 2400,
      fade_reserve: 10,
      sensitivity: -85,
    };
    for (const [key, value] of Object.entries(values)) {
      await page.fill("#b_" + key, String(value));
      await page.fill("#source_" + key, "algorithm test assumption");
      await page.selectOption("#basis_" + key, "estimated");
    }
    await page.click("#calculate");
    await page.waitForFunction(() =>
      document
        .getElementById("budgetOutput")
        .textContent.includes("predicted_received_power"),
    );
    await page
      .getByRole("button", { name: "Range Extenders", exact: true })
      .click();
    await page.fill("#nodeId", "algorithm-test-node");
    await page.fill("#nodeSource", "UI test only");
    await page.click("#addNode");
    await page.waitForFunction(() =>
      document
        .getElementById("nodes")
        .textContent.includes("algorithm-test-node"),
    );
    await page.setViewportSize({ width: 390, height: 844 });
    await page
      .getByRole("button", { name: "Live Monitor", exact: true })
      .click();
    assert.equal(
      await page.evaluate(
        () => document.documentElement.scrollWidth > innerWidth,
      ),
      false,
      "mobile horizontal overflow",
    );
    assert.deepEqual(errors, []);
    console.log(
      "PASS: nine pages, Rust budget/node APIs, missing hardware, mobile layout",
    );
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
