import { test, expect } from "@playwright/test";

test("runs wasm32-unknown-unknown demo", async ({ page }) => {
  await page.goto("./wasm32-unknown-unknown.html");

  // Expect a title "to contain" a substring.
  await expect(page).toHaveTitle(/WASM32-UNKNOWN-UNKNOWN/);

  await page
    .getByText("Loading wasm32-unknown-unknown version of demo...")
    .waitFor();
  await page.getByText("Proving :true").waitFor();
  await page.getByText("wasm32-unknown-unknown demo completed.").waitFor();
});

test("runs wasm32-wasi demo", async ({ page }) => {
  await page.goto("./wasm32-wasi.html");

  // Expect a title "to contain" a substring.
  await expect(page).toHaveTitle(/WASM32-WASI/);

  await page.getByText("Loading wasm32-wasi version of demo...").waitFor();
  await page.getByText("Proving :true").waitFor();
  await page.getByText("wasm32-wasi demo completed.").waitFor();
});
