/**
 * The Model Context Protocol, as /mcp speaks it: JSON-RPC 2.0 messages over Streamable HTTP, stateless (no session),
 * tools only. Free of Next and of server-only modules, so tests can drive it with their own tools.
 *
 *   initialize     negotiates the protocol version (the client's when we speak it, else our newest) and says what
 *                  the server offers: tools, and instructions for using them
 *   ping           {}
 *   tools/list     every tool with its input schema and annotations (all read-only)
 *   tools/call     runs one tool; a tool's own failure is a result with isError and a message saying what to do
 *   notifications  (no id) are accepted and answered with nothing
 *
 * Errors use JSON-RPC's codes: -32700 parse error, -32600 invalid request, -32601 unknown method, -32602 bad params,
 * -32603 internal error.
 */

export const SUPPORTED_VERSIONS = ["2025-06-18", "2025-03-26", "2024-11-05"] as const;
export const LATEST_VERSION = SUPPORTED_VERSIONS[0];

export type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue | undefined };
type Id = string | number | null;

export interface JsonRpcRequest {
  jsonrpc: "2.0";
  id?: Id;
  method: string;
  params?: Record<string, unknown>;
}

export interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: Id;
  result?: unknown;
  error?: { code: number; message: string; data?: unknown };
}

export interface ToolResult {
  content: Array<{ type: "text"; text: string }>;
  structuredContent?: Record<string, unknown>;
  isError?: boolean;
}

export interface ToolDefinition {
  name: string;
  title: string;
  description: string;
  inputSchema: Record<string, unknown>;
  annotations?: Record<string, unknown>;
}

export interface Tool {
  definition: ToolDefinition;
  call(args: Record<string, unknown>): Promise<ToolResult>;
}

export interface ServerInfo {
  name: string;
  title: string;
  version: string;
  instructions: string;
}

export const PARSE_ERROR = -32700;
export const INVALID_REQUEST = -32600;
export const METHOD_NOT_FOUND = -32601;
export const INVALID_PARAMS = -32602;
export const INTERNAL_ERROR = -32603;

export function rpcError(id: Id, code: number, message: string, data?: unknown): JsonRpcResponse {
  return { jsonrpc: "2.0", id, error: data === undefined ? { code, message } : { code, message, data } };
}

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);
const validId = (value: unknown): value is Id => value === null || typeof value === "string" || (typeof value === "number" && Number.isFinite(value));

/** A tool's answer: the data as JSON text (for every client) and as structured content (for clients that read it). */
export function toolResult(value: unknown, text?: string): ToolResult {
  const structured = isObject(value) ? value : { result: value };
  return { content: [{ type: "text", text: text ?? JSON.stringify(value, null, 2) }], structuredContent: structured };
}

/** A tool that could not do what was asked: a result the model reads (isError), never a protocol error. */
export function toolError(code: string, message: string, hint: string): ToolResult {
  return { content: [{ type: "text", text: `${message} ${hint}` }], structuredContent: { error: { code, message, hint } }, isError: true };
}

/** The protocol version to answer with: the client's when we speak it, else our newest. */
export function negotiateVersion(requested: unknown): string {
  return typeof requested === "string" && (SUPPORTED_VERSIONS as readonly string[]).includes(requested) ? requested : LATEST_VERSION;
}

/** Answers one JSON-RPC message, or null when it needs no answer (a notification, or a response from the client). */
export async function handleMessage(message: unknown, tools: Tool[], server: ServerInfo): Promise<JsonRpcResponse | null> {
  if (!isObject(message) || message.jsonrpc !== "2.0") {
    return rpcError(isObject(message) && validId(message.id) ? message.id : null, INVALID_REQUEST, "Not a JSON-RPC 2.0 message: it needs \"jsonrpc\": \"2.0\" and a method.");
  }
  // A response or an error sent back by the client (we never ask the client anything): nothing to answer.
  if (!("method" in message) && ("result" in message || "error" in message)) return null;
  if (typeof message.method !== "string") return rpcError(validId(message.id) ? message.id : null, INVALID_REQUEST, "The message has no method.");
  const isNotification = !("id" in message);
  if (!isNotification && !validId(message.id)) return rpcError(null, INVALID_REQUEST, "The id must be a string, a number or null.");
  const id = isNotification ? null : (message.id as Id);
  if (message.params !== undefined && !isObject(message.params)) return isNotification ? null : rpcError(id, INVALID_PARAMS, "params must be an object.");
  const params = (message.params ?? {}) as Record<string, unknown>;

  if (isNotification) return null;

  switch (message.method) {
    case "initialize":
      return {
        jsonrpc: "2.0",
        id,
        result: {
          protocolVersion: negotiateVersion(params.protocolVersion),
          capabilities: { tools: { listChanged: false } },
          serverInfo: { name: server.name, title: server.title, version: server.version },
          instructions: server.instructions,
        },
      };
    case "ping":
      return { jsonrpc: "2.0", id, result: {} };
    case "tools/list":
      return { jsonrpc: "2.0", id, result: { tools: tools.map(tool => tool.definition) } };
    case "tools/call": {
      const name = params.name;
      if (typeof name !== "string") return rpcError(id, INVALID_PARAMS, "tools/call needs params.name, the tool to run.");
      const tool = tools.find(entry => entry.definition.name === name);
      if (!tool) return rpcError(id, INVALID_PARAMS, `There is no tool called "${name.slice(0, 60)}".`, { tools: tools.map(entry => entry.definition.name) });
      const args = params.arguments === undefined ? {} : params.arguments;
      if (!isObject(args)) return rpcError(id, INVALID_PARAMS, "params.arguments must be an object.");
      try {
        return { jsonrpc: "2.0", id, result: await tool.call(args) };
      } catch (error) {
        return { jsonrpc: "2.0", id, result: toolError("tool_failed", `${name} failed: ${error instanceof Error ? error.message : String(error)}.`, "Try again in a moment.") };
      }
    }
    case "resources/list":
      return { jsonrpc: "2.0", id, result: { resources: [] } };
    case "resources/templates/list":
      return { jsonrpc: "2.0", id, result: { resourceTemplates: [] } };
    case "prompts/list":
      return { jsonrpc: "2.0", id, result: { prompts: [] } };
    default:
      return rpcError(id, METHOD_NOT_FOUND, `This server has no method "${message.method.slice(0, 60)}".`, { methods: ["initialize", "ping", "tools/list", "tools/call"] });
  }
}

/**
 * Answers a request body: one message, or a batch (an array, as protocol 2025-03-26 allows). Returns null when
 * nothing needs an answer (only notifications or client responses), which HTTP answers with 202.
 */
export async function handleBody(text: string, tools: Tool[], server: ServerInfo): Promise<JsonRpcResponse | JsonRpcResponse[] | null> {
  let body: unknown;
  try {
    body = JSON.parse(text);
  } catch {
    return rpcError(null, PARSE_ERROR, "The body is not valid JSON.");
  }
  if (Array.isArray(body)) {
    if (!body.length) return rpcError(null, INVALID_REQUEST, "An empty batch has nothing to answer.");
    const answers = (await Promise.all(body.slice(0, 32).map(message => handleMessage(message, tools, server)))).filter((answer): answer is JsonRpcResponse => answer !== null);
    return answers.length ? answers : null;
  }
  return handleMessage(body, tools, server);
}

/* ------------------------------------------------------------------------------------------------------------------ */
/* Argument checks for tools                                                                                           */
/* ------------------------------------------------------------------------------------------------------------------ */

export function stringArg(args: Record<string, unknown>, name: string, { required = false, max = 200 }: { required?: boolean; max?: number } = {}): string | null | ToolResult {
  const value = args[name];
  if (value === undefined || value === null || value === "") {
    return required ? toolError("missing_argument", `${name} is required.`, `Pass ${name} as a string.`) : null;
  }
  if (typeof value !== "string") return toolError("invalid_argument", `${name} must be a string.`, `Pass ${name} as text.`);
  if (value.length > max) return toolError("invalid_argument", `${name} is ${value.length} characters; the most is ${max}.`, `Shorten ${name}.`);
  return value;
}

export function intArg(args: Record<string, unknown>, name: string, { min, max, fallback }: { min: number; max: number; fallback: number }): number | ToolResult {
  const value = args[name];
  if (value === undefined || value === null || value === "") return fallback;
  const number = typeof value === "string" && /^\d+$/.test(value) ? Number(value) : value;
  if (typeof number !== "number" || !Number.isInteger(number) || number < min || number > max) return toolError("invalid_argument", `${name} must be a whole number from ${min} to ${max}.`, `Leave it out for ${fallback}.`);
  return number;
}

export const isToolResult = (value: unknown): value is ToolResult => isObject(value) && Array.isArray(value.content);
