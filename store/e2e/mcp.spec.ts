/** The MCP server at /mcp: Streamable HTTP, stateless, five read-only tools, rate limited. */
import { expect, test, type APIRequestContext } from "@playwright/test";

const HEADERS = { "Content-Type": "application/json", Accept: "application/json, text/event-stream" };

async function rpc(request: APIRequestContext, method: string, params: Record<string, unknown> = {}, headers: Record<string, string> = {}) {
  const response = await request.post("/mcp", { headers: { ...HEADERS, ...headers }, data: { jsonrpc: "2.0", id: 1, method, params } });
  return { response, body: await response.json() };
}

async function call(request: APIRequestContext, name: string, args: Record<string, unknown>, headers: Record<string, string> = {}) {
  const { body } = await rpc(request, "tools/call", { name, arguments: args }, headers);
  return body.result as { structuredContent: Record<string, unknown>; isError?: boolean; content: Array<{ text: string }> };
}

test("initialize negotiates the protocol version and lists the five read-only tools", async ({ request }) => {
  const { response, body } = await rpc(request, "initialize", { protocolVersion: "2025-03-26", capabilities: {}, clientInfo: { name: "e2e", version: "1" } });
  expect(response.status()).toBe(200);
  expect(body.result.protocolVersion).toBe("2025-03-26");
  expect(body.result.serverInfo.name).toBe("silicon-apps-store");
  expect(response.headers()["ratelimit-limit"]).toBe("60");
  const newest = await rpc(request, "initialize", { protocolVersion: "1999-01-01" });
  expect(newest.body.result.protocolVersion).toBe("2025-06-18");
  const { body: list } = await rpc(request, "tools/list");
  const tools = list.result.tools as Array<{ name: string; annotations: { readOnlyHint: boolean } }>;
  expect(tools.map(tool => tool.name)).toEqual(["search_apps", "get_app", "list_releases", "get_install_command", "list_reviews"]);
  for (const tool of tools) expect(tool.annotations.readOnlyHint).toBe(true);
});

test("search_apps, get_app, list_releases, get_install_command and list_reviews", async ({ request }) => {
  const search = await call(request, "search_apps", { query: "brifcase" });
  const apps = search.structuredContent.apps as Array<{ app_id: string; install: string; url: string }>;
  expect(apps[0].app_id).toBe("briefcase");
  expect(apps[0].install).toBe("silicon-apps install briefcase");
  expect(apps[0].url).toBe("https://apps.teamofsilicons.com/apps/briefcase");
  const tagged = await call(request, "search_apps", { tag: "productivity", target: "linux-x86_64" });
  expect((tagged.structuredContent.apps as Array<{ app_id: string }>).map(app => app.app_id).sort()).toEqual(["briefcase", "notes", "remind"]);

  const app = (await call(request, "get_app", { app_id: "briefcase" })).structuredContent;
  expect(app.name).toBe("Briefcase");
  expect(app.signed).toBe(true);
  expect(app.signed_by_author).toBe(true);
  expect(app.signed_by).toEqual(["c:shubham"]);
  expect((app.withdrawn_releases as Array<{ version: string; reason: string }>)[0]).toMatchObject({ version: "3.4.3", reason: expect.stringContaining("space") });

  const releases = (await call(request, "list_releases", { app_id: "briefcase", channel: "production" })).structuredContent.releases as Array<{ version: string; withdrawn: unknown; signed: boolean }>;
  expect(releases.map(release => release.version).sort()).toEqual(["3.4.2", "3.4.3"]);
  expect(releases.find(release => release.version === "3.4.3")!.withdrawn).toBeTruthy();
  expect(releases.find(release => release.version === "3.4.2")!.signed).toBe(true);

  const install = (await call(request, "get_install_command", { app_id: "orbit", channel: "development" })).structuredContent;
  expect(install).toMatchObject({ available: true, command: "silicon-apps install 'orbit>dev'", version: "0.4.0" });
  const exact = (await call(request, "get_install_command", { app_id: "briefcase", version: "3.4.2" })).structuredContent;
  expect(exact.command).toBe("silicon-apps install 'briefcase@3.4.2'");
  const none = (await call(request, "get_install_command", { app_id: "orbit" })).structuredContent;
  expect(none).toMatchObject({ available: false, alternatives: ["silicon-apps install 'orbit>dev'"] });

  const reviews = (await call(request, "list_reviews", { app_id: "dm" })).structuredContent;
  expect(reviews.count).toBe(3);
  expect((reviews.reviews as Array<{ reviewer: string }>)[0].reviewer).toBe("si:nova");
});

test("failures are tool results with what to do; protocol errors are JSON-RPC errors", async ({ request }) => {
  const missing = await call(request, "get_app", { app_id: "team-vault" });
  expect(missing.isError).toBe(true);
  expect((missing.structuredContent.error as { code: string }).code).toBe("not_found");
  const bad = await call(request, "get_app", {});
  expect(bad.isError).toBe(true);
  const unknown = await rpc(request, "tools/call", { name: "publish_app", arguments: {} });
  expect(unknown.body.error.code).toBe(-32602);
  const parse = await request.post("/mcp", { headers: HEADERS, data: Buffer.from("{not json") });
  expect(parse.status()).toBe(400);
  expect((await parse.json()).error.code).toBe(-32700);
  const get = await request.get("/mcp");
  expect(get.status()).toBe(405);
  expect(get.headers().allow).toContain("POST");
});

test("a client that reads only event streams gets one SSE event", async ({ request }) => {
  const response = await request.post("/mcp", { headers: { "Content-Type": "application/json", Accept: "text/event-stream" }, data: { jsonrpc: "2.0", id: 7, method: "ping" } });
  expect(response.headers()["content-type"]).toContain("text/event-stream");
  expect(await response.text()).toBe('event: message\ndata: {"jsonrpc":"2.0","id":7,"result":{}}\n\n');
});

test("your own bearer token includes the private apps shared with you", async ({ request }) => {
  const anonymous = await call(request, "search_apps", { query: "vault" });
  expect((anonymous.structuredContent.apps as Array<{ app_id: string }>).map(app => app.app_id)).not.toContain("team-vault");
  const mira = await call(request, "search_apps", { query: "vault" }, { Authorization: "Bearer dev:mira:c:mira" });
  expect((mira.structuredContent.apps as Array<{ app_id: string }>).map(app => app.app_id)).toContain("team-vault");
});

test("past 60 requests a minute from one address it answers 429 with Retry-After", async ({ request }) => {
  // A client address of its own (Caddy sets X-Forwarded-For in production), so the other tests keep their budget.
  const headers = { ...HEADERS, "X-Forwarded-For": `198.51.100.${Math.floor(Math.random() * 200) + 1}` };
  let last = null as Awaited<ReturnType<APIRequestContext["post"]>> | null;
  for (let index = 0; index < 61; index++) last = await request.post("/mcp", { headers, data: { jsonrpc: "2.0", id: index, method: "ping" } });
  expect(last!.status()).toBe(429);
  expect(Number(last!.headers()["retry-after"])).toBeGreaterThan(0);
  const body = await last!.json();
  expect(body.error.data.code).toBe("rate_limited");
});
