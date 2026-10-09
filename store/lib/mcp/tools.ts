/**
 * The tools /mcp offers, all read-only, over the Apps API (server to server, APPS_API_URL):
 *
 *   search_apps           GET /v1/apps, with the store's tag and platform filters on top
 *   get_app               GET /v1/apps/{app_id}
 *   list_releases         GET /v1/apps/{app_id}/releases
 *   get_install_command   the exact silicon-apps command for a channel or version, and how to get the CLI
 *   list_reviews          GET /v1/apps/{app_id}/reviews
 *
 * Anonymous callers see public apps. A caller that sends its own `Authorization: Bearer` Silicon Apps token sees what
 * that account may see (private apps shared with it); the token goes to the API and nowhere else. Failures come back
 * as tool results with isError, in the API's own words.
 */
import "server-only";
import { api, ApiError, APP_ID_PATTERN } from "@/lib/api";
import { listAll, listPage } from "@/lib/catalog";
import { TARGETS, targetLabel } from "@/lib/format";
import { absolute, appPath, authorPath } from "@/lib/seo";
import { INSTALL_UNIX, INSTALL_WINDOWS, installCommand } from "@/lib/site";
import type { App, AppList, Release, Reviews } from "@/lib/types";
import { intArg, isToolResult, stringArg, toolError, toolResult, type ServerInfo, type Tool, type ToolResult } from "./protocol";

export const SERVER: ServerInfo = {
  name: "silicon-apps-store",
  title: "Silicon Apps",
  version: "1.0.0",
  instructions:
    "Silicon Apps is the app store of the Silicon ecosystem. Use search_apps to find apps (partial names and spelling mistakes match, exact ids and names come first), get_app for one app, list_releases and list_reviews for its releases and reviews, and get_install_command for the exact silicon-apps command. Public apps need no token; send your own Silicon Apps bearer token to include private apps shared with you. Making or publishing apps lives at https://developers.teamofsilicons.com.",
};

const READ_ONLY = { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: false } as const;
const VERSION = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

const appIdSchema = { type: "string", maxLength: 64, description: "The app's permanent id, for example briefcase." };
const channelSchema = { type: "string", enum: ["production", "development"], description: "The release channel. Production when left out." };

function summary(app: App) {
  return {
    app_id: app.app_id,
    name: app.name,
    description: app.description,
    visibility: app.visibility,
    tags: app.tags,
    targets: app.targets,
    rating: app.rating,
    review_count: app.review_count,
    installs: app.installs,
    production_version: app.latest_production?.version ?? null,
    development_version: app.latest_development?.version ?? null,
    install: installCommand(app.app_id),
    url: absolute(appPath(app.app_id)),
  };
}

function release(r: Release) {
  const signed = r.package_ids.length > 0 && r.package_ids.every(id => Boolean(r.signatures?.[id]));
  return {
    id: r.id,
    channel: r.channel,
    version: r.version,
    notes: r.notes,
    created_at: r.created_at,
    promoted_from: r.promoted_from ?? null,
    signed,
    signed_by_author: r.signed_by_author ?? false,
    withdrawn: r.withdrawn ? { at: r.withdrawn.at, by: r.withdrawn.by_id, reason: r.withdrawn.reason } : null,
  };
}

/** The app_id argument, checked, or the tool result that says what is wrong with it. */
function appIdArg(args: Record<string, unknown>): string | ToolResult {
  const id = stringArg(args, "app_id", { required: true, max: 64 });
  if (isToolResult(id)) return id;
  if (!id || !APP_ID_PATTERN.test(id)) return toolError("invalid_argument", "app_id must be an app id, such as briefcase.", "Find the id with search_apps.");
  return id;
}

function channelArg(args: Record<string, unknown>): "production" | "development" | ToolResult {
  const channel = stringArg(args, "channel", { max: 20 });
  if (isToolResult(channel)) return channel;
  if (!channel) return "production";
  if (channel !== "production" && channel !== "development") return toolError("invalid_argument", "channel must be production or development.", "Leave it out for production.");
  return channel;
}

/** An API failure as a tool result the model can act on. */
function failed(error: unknown): ToolResult {
  if (error instanceof ApiError) {
    if (error.status === 404) return toolError("not_found", "No app with that id is visible to you.", "Check the id with search_apps. A private app needs your own Silicon Apps bearer token in the Authorization header.");
    if (error.status === 401) return toolError("unauthorized", error.message, "Your bearer token was not accepted. Sign in again with silicon-accounts login --app silicon-apps, then silicon-apps login --slt TOKEN.");
    if (error.status === 429) return toolError("rate_limited", error.message, error.hint ?? "Wait a little, then try again.");
    if (error.status === 0) return toolError("upstream_unreachable", "The Apps API could not be reached.", "Try again in a moment.");
    return toolError(error.code, error.message, error.hint ?? "Try again in a moment.");
  }
  return toolError("tool_failed", error instanceof Error ? error.message : String(error), "Try again in a moment.");
}

/** The five tools, bound to the caller's own token (or none). */
export function toolsFor(bearer?: string): Tool[] {
  const read = <T>(path: string) => api<T>(path, { anonymous: true, bearer });
  return [
    {
      definition: {
        name: "search_apps",
        title: "Search apps",
        description:
          "Search Silicon Apps by app id, name, description and tags. Partial names and spelling mistakes still match, and exact ids and names come first. Filter by tag or platform target if you like. An empty query lists every app you can see. Each result has its install command and store page.",
        inputSchema: {
          type: "object",
          properties: {
            query: { type: "string", maxLength: 200, description: "What to look for, for example notes, brief or briefcase." },
            tag: { type: "string", maxLength: 60, description: "Only apps with this tag, for example productivity." },
            target: { type: "string", enum: [...TARGETS], description: "Only apps with a package for this OS and architecture." },
            limit: { type: "integer", minimum: 1, maximum: 50, default: 10 },
            offset: { type: "integer", minimum: 0, maximum: 1000, default: 0 },
          },
          additionalProperties: false,
        },
        annotations: { title: "Search apps", ...READ_ONLY },
      },
      async call(args) {
        const query = stringArg(args, "query", { max: 200 });
        if (isToolResult(query)) return query;
        const tag = stringArg(args, "tag", { max: 60 });
        if (isToolResult(tag)) return tag;
        const target = stringArg(args, "target", { max: 40 });
        if (isToolResult(target)) return target;
        if (target && !(TARGETS as readonly string[]).includes(target)) return toolError("invalid_argument", `target must be one of ${TARGETS.join(", ")}.`, "Leave it out for every platform.");
        const limit = intArg(args, "limit", { min: 1, max: 50, fallback: 10 });
        if (isToolResult(limit)) return limit;
        const offset = intArg(args, "offset", { min: 0, max: 1000, fallback: 0 });
        if (isToolResult(offset)) return offset;
        try {
          if (!tag && !target) {
            const page = bearer
              ? await read<AppList>(`/v1/apps?limit=${limit}&offset=${offset}${query ? `&q=${encodeURIComponent(query)}` : ""}`)
              : await listPage(query ?? "", "all", limit, offset, true);
            return toolResult({ total: page.total, apps: page.items.map(summary) });
          }
          const all = bearer ? (await read<AppList>(`/v1/apps?limit=100${query ? `&q=${encodeURIComponent(query)}` : ""}`)).items : await listAll(query ?? "", "all", true);
          const wanted = tag?.toLowerCase();
          const matching = all.filter(app => (!wanted || app.tags.some(t => t.toLowerCase() === wanted)) && (!target || app.targets.includes(target)));
          return toolResult({ total: matching.length, apps: matching.slice(offset, offset + limit).map(summary) });
        } catch (error) {
          return failed(error);
        }
      },
    },
    {
      definition: {
        name: "get_app",
        title: "Get an app",
        description: "Get one app by its app_id: what it does, its authors, tags, links, supported platforms, latest production and development releases, who signed the current release, withdrawn releases with their reasons, rating, installs, install command and store page.",
        inputSchema: { type: "object", properties: { app_id: appIdSchema }, required: ["app_id"], additionalProperties: false },
        annotations: { title: "Get an app", ...READ_ONLY },
      },
      async call(args) {
        const id = appIdArg(args);
        if (isToolResult(id)) return id;
        try {
          const app = await read<App>(`/v1/apps/${encodeURIComponent(id)}`);
          return toolResult({
            ...summary(app),
            authors: app.authors.map(a => ({ uuid: a.uuid, id: a.id, display_name: a.display_name, url: absolute(authorPath(a.uuid)) })),
            platforms: app.targets.map(t => ({ target: t, label: targetLabel(t) })),
            links: app.links ?? {},
            latest_production: app.latest_production ? release(app.latest_production) : null,
            latest_development: app.latest_development ? release(app.latest_development) : null,
            signed: app.signed ?? false,
            signed_by_author: app.signed_by_author ?? false,
            signed_by: app.signed_by ?? [],
            withdrawn_releases: app.withdrawn_releases ?? [],
            created_at: app.created_at,
            updated_at: app.updated_at,
          });
        } catch (error) {
          return failed(error);
        }
      },
    },
    {
      definition: {
        name: "list_releases",
        title: "List releases",
        description: "List an app's releases, newest first: version, channel (production or development), notes, date, whether it is signed (by Silicon Apps and by its authors) and, for a withdrawn release, why. Production and development keep their own version numbers. A withdrawn release is never installed.",
        inputSchema: { type: "object", properties: { app_id: appIdSchema, channel: channelSchema }, required: ["app_id"], additionalProperties: false },
        annotations: { title: "List releases", ...READ_ONLY },
      },
      async call(args) {
        const id = appIdArg(args);
        if (isToolResult(id)) return id;
        const channel = stringArg(args, "channel", { max: 20 });
        if (isToolResult(channel)) return channel;
        if (channel && channel !== "production" && channel !== "development") return toolError("invalid_argument", "channel must be production or development.", "Leave it out for both.");
        try {
          const data = await read<{ items: Release[] }>(`/v1/apps/${encodeURIComponent(id)}/releases${channel ? `?channel=${channel}` : ""}`);
          const items = [...data.items].sort((a, b) => b.created_at.localeCompare(a.created_at));
          return toolResult({ app_id: id, releases: items.map(release) });
        } catch (error) {
          return failed(error);
        }
      },
    },
    {
      definition: {
        name: "get_install_command",
        title: "Get the install command",
        description:
          "Get the exact silicon-apps command that installs an app: the latest production release by default, the development channel, or an exact version. Also says how to install the silicon-apps CLI itself if you do not have it yet.",
        inputSchema: {
          type: "object",
          properties: { app_id: appIdSchema, channel: channelSchema, version: { type: "string", pattern: VERSION.source, description: "An exact x.y.z version on that channel, for example 1.2.3." } },
          required: ["app_id"],
          additionalProperties: false,
        },
        annotations: { title: "Get the install command", ...READ_ONLY },
      },
      async call(args) {
        const id = appIdArg(args);
        if (isToolResult(id)) return id;
        const channel = channelArg(args);
        if (isToolResult(channel)) return channel;
        const version = stringArg(args, "version", { max: 40 });
        if (isToolResult(version)) return version;
        if (version && !VERSION.test(version)) return toolError("invalid_argument", "version must look like 1.2.3.", "Leave it out for the latest release.");
        try {
          const app = await read<App>(`/v1/apps/${encodeURIComponent(id)}`);
          const releases = version ? (await read<{ items: Release[] }>(`/v1/apps/${encodeURIComponent(id)}/releases?channel=${channel}`)).items : [];
          const chosen = version ? releases.find(r => r.version === version) : channel === "production" ? app.latest_production : app.latest_development;
          if (!chosen) {
            return toolResult({
              app_id: id,
              available: false,
              message: version ? `${app.name} has no ${channel} release ${version}.` : `${app.name} has no ${channel} release yet.`,
              alternatives: channel === "production" && app.latest_development ? [`silicon-apps install '${id}>dev'`] : [],
            });
          }
          const spec = `${id}${channel === "development" ? ">dev" : ""}${version ? `@${version}` : ""}`;
          return toolResult({
            app_id: id,
            available: true,
            command: spec === id ? installCommand(id) : `silicon-apps install '${spec}'`,
            channel,
            version: chosen.version,
            targets: app.targets,
            then: `Run ${id} --help to find your way around.`,
            updates: "Silicon Apps checks for a new release every minute and updates the app on the channel you installed it from.",
            cli_install: { macos_linux: INSTALL_UNIX, windows_powershell: INSTALL_WINDOWS },
          });
        } catch (error) {
          return failed(error);
        }
      },
    },
    {
      definition: {
        name: "list_reviews",
        title: "List reviews",
        description: "List an app's reviews from the Carbons and Silicons who use it, newest first: 1 to 5 stars and optional text, with the average rating.",
        inputSchema: { type: "object", properties: { app_id: appIdSchema, limit: { type: "integer", minimum: 1, maximum: 100, default: 20 } }, required: ["app_id"], additionalProperties: false },
        annotations: { title: "List reviews", ...READ_ONLY },
      },
      async call(args) {
        const id = appIdArg(args);
        if (isToolResult(id)) return id;
        const limit = intArg(args, "limit", { min: 1, max: 100, fallback: 20 });
        if (isToolResult(limit)) return limit;
        try {
          const data = await read<Reviews>(`/v1/apps/${encodeURIComponent(id)}/reviews`);
          const items = [...data.items].sort((a, b) => b.updated_at.localeCompare(a.updated_at)).slice(0, limit);
          return toolResult({ app_id: id, rating: data.rating, count: data.count, reviews: items.map(r => ({ reviewer: r.id, rating: r.rating, text: r.text, updated_at: r.updated_at })) });
        } catch (error) {
          return failed(error);
        }
      },
    },
  ];
}

/** The tool names, for GET /mcp and the agent files. */
export const TOOL_NAMES = toolsFor().map(tool => tool.definition.name);
