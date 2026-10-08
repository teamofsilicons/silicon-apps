import { expect, test } from "@playwright/test";
import { mock } from "./fixtures";

const portal = "https://developers.teamofsilicons.com";

test("store creation links leave the store and never expose creation controls", async ({
  page,
}) => {
  const mutations = await mock(page, true);
  await page.goto("/store");
  await expect(
    page.getByRole("button", { name: "Create app", exact: true }),
  ).toHaveCount(0);
  for (const name of [
    "Developers",
    "Create your first app",
    "Open developer platform",
    "Build an app",
  ])
    await expect(
      page.getByRole("link", { name, exact: name === "Developers" }),
    ).toHaveAttribute("href", portal + "/");
  await page.goto("/store/test-app");
  await expect(page.getByRole("link", { name: "Manage app" })).toHaveAttribute(
    "href",
    portal + "/apps/test-app/publishing",
  );
  expect(mutations.filter((m) => m.path !== "/telemetry")).toEqual([]);
});

for (const [legacy, destination] of [
  ["/developer", "/"],
  ["/developer/apps/test-app?step=3", "/apps/test-app/publishing?step=3"],
  ["/developer/apps/test-app?tab=authors", "/apps/test-app/authors"],
  ["/developer/invitations", "/invitations"],
]) {
  test(`legacy management route ${legacy} redirects to the shared portal`, async ({
    page,
  }) => {
    const mutations = await mock(page, true);
    await page.route(portal + "/**", (route) =>
      route.fulfill({
        contentType: "text/html",
        body: "<h1>Shared developer portal</h1>",
      }),
    );
    await page.goto(legacy);
    await expect(page).toHaveURL(portal + destination);
    await expect(
      page.getByRole("heading", { name: "Shared developer portal" }),
    ).toBeVisible();
    expect(mutations.filter((m) => m.path !== "/telemetry")).toEqual([]);
  });
}
