import { expect, test } from "@playwright/test";
import { signIn } from "./helpers";

test("live catalog changes refresh the page and preserve an unfinished review", async ({ page, context, baseURL }) => {
  // This suite's API has isolated fixture identities; never use them on a remote host.
  expect(new URL(baseURL!).hostname).toBe("127.0.0.1");
  // The auth suite signs Ada out; use Nova's independent fixture session.
  await signIn(context, baseURL!, "nova");
  const stream = page.waitForResponse(response => response.url().includes("/v1/events/stream?"));
  await page.goto("/apps/ring");
  expect((await stream).status()).toBe(200);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Ring");
  const change = async (name: string) => {
    const response = await page.request.patch("/v1/apps/ring", {
      headers: { Authorization: "Bearer dev:nova:si:nova", Origin: baseURL!, "Idempotency-Key": crypto.randomUUID() },
      data: { name },
    });
    expect(response.status()).toBe(200);
  };
  try {
    await change("Ring Live");
    await expect(page.getByRole("heading", { level: 1 })).toHaveText("Ring Live");
    const review = page.locator("textarea[name=text]");
    await review.fill("Keep this unfinished review.");
    await change("Ring Changed Again");
    // Wait past the refresh debounce; the event must not replace this form's page.
    await page.waitForTimeout(1200);
    await expect(page.getByRole("heading", { level: 1 })).toHaveText("Ring Live");
    await expect(review).toHaveValue("Keep this unfinished review.");
  } finally {
    await change("Ring");
  }
});
