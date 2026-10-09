/**
 * /mcp: the store's MCP server over Streamable HTTP (protocol 2025-06-18; 2025-03-26 and 2024-11-05 are negotiated
 * too), the same server as the developer site's (developer/app/mcp/route.ts). Stateless: every POST carries JSON-RPC
 * and gets its answer in the response, as JSON, or as a one-event SSE stream when the client accepts only
 * text/event-stream. No session id is issued, and the server never opens a stream of its own, so GET answers 405 (as
 * the transport allows). The tools are read-only (lib/mcp/tools.ts); a caller's own `Authorization: Bearer` Silicon
 * Apps token is passed to the Apps API so private apps shared with it are included.
 *
 * Limits: 60 requests a minute per client address (429 with Retry-After), bodies up to 64 KB, batches up to 32.
 */
import { handleBody, rpcError, SUPPORTED_VERSIONS, INVALID_REQUEST, type JsonRpcResponse } from "@/lib/mcp/protocol";
import { SERVER, TOOL_NAMES, toolsFor } from "@/lib/mcp/tools";
import { RATE_LIMITS } from "@/lib/site";
import { CORS_HEADERS, preflight } from "@/lib/server/public-response";
import { rateLimit, rateLimitedError } from "@/lib/server/rate-limit";

export const dynamic = "force-dynamic";

const MAX_BODY = 64 * 1024;

function reply(status: number, body: JsonRpcResponse | JsonRpcResponse[] | null, headers: Record<string, string>, sse = false): Response {
  const base = { "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff", ...CORS_HEADERS, ...headers };
  if (body === null) return new Response(null, { status: 202, headers: base });
  const json = JSON.stringify(body);
  if (sse) {
    return new Response(`event: message\ndata: ${json}\n\n`, { status, headers: { ...base, "Content-Type": "text/event-stream; charset=utf-8", Connection: "keep-alive" } });
  }
  return new Response(`${json}\n`, { status, headers: { ...base, "Content-Type": "application/json; charset=utf-8" } });
}

/** JSON unless the client says it can only read an event stream. */
function wantsEventStream(request: Request): boolean {
  const accept = (request.headers.get("accept") ?? "").toLowerCase();
  return accept.includes("text/event-stream") && !accept.includes("application/json") && !accept.includes("*/*");
}

/** The caller's own Silicon Apps token, when it sent one. Never read from cookies: a browser page cannot borrow a session here. */
function bearerOf(request: Request): string | undefined {
  const value = request.headers.get("authorization") ?? "";
  const match = /^Bearer\s+([\x21-\x7e]{8,4096})$/i.exec(value.trim());
  return match?.[1];
}

export async function POST(request: Request) {
  const decision = rateLimit(request, "mcp", RATE_LIMITS.mcp);
  const headers = decision.headers;
  const sse = wantsEventStream(request);
  if (!decision.ok) {
    const problem = rateLimitedError(decision, RATE_LIMITS.mcp.windowSeconds);
    return reply(429, rpcError(null, -32000, problem.message, { ...problem, retry_after: decision.reset }), headers, sse);
  }
  const version = request.headers.get("mcp-protocol-version");
  if (version && !(SUPPORTED_VERSIONS as readonly string[]).includes(version)) {
    return reply(400, rpcError(null, INVALID_REQUEST, `This server does not speak MCP protocol version ${version.slice(0, 40)}.`, { supported: SUPPORTED_VERSIONS }), headers, sse);
  }
  const type = (request.headers.get("content-type") ?? "").toLowerCase();
  if (type && !type.startsWith("application/json")) {
    return reply(415, rpcError(null, INVALID_REQUEST, "Send the JSON-RPC message as application/json."), headers, sse);
  }
  const declared = Number(request.headers.get("content-length") ?? "0");
  if (declared > MAX_BODY) return reply(413, rpcError(null, INVALID_REQUEST, `The body is larger than ${MAX_BODY / 1024} KB.`), headers, sse);
  const text = await request.text();
  if (text.length > MAX_BODY) return reply(413, rpcError(null, INVALID_REQUEST, `The body is larger than ${MAX_BODY / 1024} KB.`), headers, sse);
  const answer = await handleBody(text, toolsFor(bearerOf(request)), SERVER);
  const failed = answer !== null && !Array.isArray(answer) && answer.error && (answer.error.code === -32700 || (answer.error.code === INVALID_REQUEST && answer.id === null));
  return reply(failed ? 400 : 200, answer, headers, sse);
}

/** No server-initiated stream: this server is stateless. A GET (a browser, a crawler) gets 405 and how to use it. */
export function GET() {
  return new Response(
    `${JSON.stringify({
      error: { code: "method_not_allowed", message: "This MCP server takes JSON-RPC over POST (Streamable HTTP) and opens no event stream of its own.", hint: "POST an initialize request to /mcp with Content-Type: application/json, then tools/list and tools/call." },
      server: { name: SERVER.name, title: SERVER.title, version: SERVER.version, protocol_versions: SUPPORTED_VERSIONS, tools: TOOL_NAMES },
    }, null, 2)}\n`,
    { status: 405, headers: { Allow: "POST, OPTIONS", "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store", ...CORS_HEADERS } },
  );
}

export function DELETE() {
  return new Response(null, { status: 405, headers: { Allow: "POST, OPTIONS", ...CORS_HEADERS } });
}

export const OPTIONS = () => preflight("POST, GET, OPTIONS");
