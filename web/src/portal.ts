export const DEVELOPERS_URL = (
  import.meta.env.VITE_DEVELOPERS_URL || "https://developers.teamofsilicons.com"
).replace(/\/$/, "");

export function developerUrl(path = "/") {
  return `${DEVELOPERS_URL}/${path.replace(/^\/+/, "")}`;
}

export function docsUrl(pathname = "/docs", search = "", hash = "") {
  const suffix = pathname.replace(/^\/docs(?:\/|$)/, "").replace(/^\/+/, "");
  return developerUrl(
    `/docs/apps${suffix ? `/${suffix}` : ""}${search}${hash}`,
  );
}

export function legacyDeveloperUrl(pathname: string, search: string) {
  const path = pathname.replace(/^\/developer\/?/, "");
  const params = new URLSearchParams(search);
  const match = /^apps\/([a-z0-9_-]+)\/?$/.exec(path);
  if (match) {
    const tab = params.get("tab");
    const section =
      tab && ["releases", "authors", "history"].includes(tab)
        ? tab
        : "publishing";
    params.delete("tab");
    const query = params.toString();
    return developerUrl(
      `/apps/${match[1]}/${section}${query ? `?${query}` : ""}`,
    );
  }
  return developerUrl(`/${path}${search}`);
}
