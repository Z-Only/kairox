#!/usr/bin/env node
// Playwright bridge — stdin/stdout JSON-line protocol.
// Launched by PlaywrightManager as a child process.
// Each line on stdin is a JSON request; each response is a JSON line on stdout.
// Stderr is used for diagnostics only.

const readline = require("readline");

let browser = null;
let page = null;

async function launchBrowser() {
  let pw;
  try {
    pw = require("playwright");
  } catch {
    try {
      // Fallback: try playwright-core (installed separately)
      pw = require("playwright-core");
    } catch {
      throw new Error("Playwright is not installed. Run: npx playwright install chromium");
    }
  }
  browser = await pw.chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 720 }
  });
  page = await context.newPage();
}

async function ensurePage() {
  if (!browser || !page) {
    await launchBrowser();
  }
}

async function handleAction(action) {
  switch (action.action) {
    case "navigate": {
      await ensurePage();
      await page.goto(action.url, {
        waitUntil: "domcontentloaded",
        timeout: 30000
      });
      const title = await page.title();
      const url = page.url();
      return {
        success: true,
        output: `Navigated to ${url}`,
        screenshot: null,
        current_url: url,
        title
      };
    }

    case "click": {
      await ensurePage();
      await page.click(action.selector, { timeout: 10000 });
      return {
        success: true,
        output: `Clicked element: ${action.selector}`,
        screenshot: null,
        current_url: page.url(),
        title: await page.title()
      };
    }

    case "type": {
      await ensurePage();
      await page.fill(action.selector, action.text, { timeout: 10000 });
      return {
        success: true,
        output: `Typed "${action.text}" into ${action.selector}`,
        screenshot: null,
        current_url: null,
        title: null
      };
    }

    case "screenshot": {
      await ensurePage();
      const buf = await page.screenshot({
        fullPage: action.full_page === true,
        type: "png"
      });
      const base64 = buf.toString("base64");
      return {
        success: true,
        output: "Screenshot captured",
        screenshot: base64,
        current_url: page.url(),
        title: await page.title()
      };
    }

    case "get_text": {
      await ensurePage();
      let text;
      if (action.selector) {
        const el = await page.$(action.selector);
        text = el ? await el.textContent() : "";
      } else {
        text = await page.textContent("body");
      }
      return {
        success: true,
        output: text || "",
        screenshot: null,
        current_url: page.url(),
        title: await page.title()
      };
    }

    case "get_state": {
      await ensurePage();
      return {
        success: true,
        output: "Browser state retrieved",
        screenshot: null,
        current_url: page.url(),
        title: await page.title()
      };
    }

    case "scroll": {
      await ensurePage();
      const amount = action.amount || 300;
      const dir = action.direction || "down";
      let deltaX = 0;
      let deltaY = 0;
      switch (dir) {
        case "down":
          deltaY = amount;
          break;
        case "up":
          deltaY = -amount;
          break;
        case "right":
          deltaX = amount;
          break;
        case "left":
          deltaX = -amount;
          break;
      }
      await page.mouse.wheel(deltaX, deltaY);
      return {
        success: true,
        output: `Scrolled ${dir} by ${amount} pixels`,
        screenshot: null,
        current_url: null,
        title: null
      };
    }

    case "hover": {
      await ensurePage();
      await page.hover(action.selector, { timeout: 10000 });
      return {
        success: true,
        output: `Hovered over: ${action.selector}`,
        screenshot: null,
        current_url: null,
        title: null
      };
    }

    case "wait": {
      await ensurePage();
      if (action.selector) {
        await page.waitForSelector(action.selector, {
          timeout: action.timeout_ms || 5000
        });
        return {
          success: true,
          output: `Waited for ${action.selector}`,
          screenshot: null,
          current_url: null,
          title: null
        };
      }
      await new Promise((r) => setTimeout(r, action.timeout_ms || 1000));
      return {
        success: true,
        output: `Waited ${action.timeout_ms || 1000}ms`,
        screenshot: null,
        current_url: null,
        title: null
      };
    }

    case "form_fill": {
      await ensurePage();
      await page.fill(action.selector, action.value, { timeout: 10000 });
      return {
        success: true,
        output: `Filled ${action.selector} with "${action.value}"`,
        screenshot: null,
        current_url: null,
        title: null
      };
    }

    case "close": {
      if (browser) {
        await browser.close();
        browser = null;
        page = null;
      }
      return {
        success: true,
        output: "Browser closed",
        screenshot: null,
        current_url: null,
        title: null
      };
    }

    default:
      return {
        success: false,
        output: `Unknown action: ${action.action}`,
        screenshot: null,
        current_url: null,
        title: null
      };
  }
}

// Main loop: read JSON lines from stdin, write JSON lines to stdout.
const rl = readline.createInterface({ input: process.stdin });

rl.on("line", async (line) => {
  let request;
  try {
    request = JSON.parse(line);
  } catch (err) {
    const errorResponse = { id: null, error: `Invalid JSON: ${err.message}` };
    process.stdout.write(JSON.stringify(errorResponse) + "\n");
    return;
  }

  try {
    const result = await handleAction(request.action);
    const response = { id: request.id, result };
    process.stdout.write(JSON.stringify(response) + "\n");
  } catch (err) {
    const errorResponse = { id: request.id, error: err.message };
    process.stdout.write(JSON.stringify(errorResponse) + "\n");
  }
});

rl.on("close", async () => {
  if (browser) {
    try {
      await browser.close();
    } catch {
      // ignore cleanup errors
    }
  }
  process.exit(0);
});

// Signal readiness
process.stderr.write("playwright-bridge: ready\n");
