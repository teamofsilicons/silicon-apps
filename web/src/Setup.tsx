import { useState } from "react";
import {
  Check,
  ExternalLink,
  Globe,
  Image,
  LockKeyhole,
  Plus,
  Trash2,
  Upload,
  Webhook,
} from "lucide-react";
import {
  api,
  flushPendingSaves,
  useAutosave,
  useMutation,
  useResource,
} from "./api";
import type { App, Links, Media, Package, Readiness, Target } from "./types";
import { DEFAULT_EVENTS, EVENTS, TARGETS, targetLabel } from "./types";
import {
  Badge,
  Button,
  Command,
  Empty,
  ErrorNotice,
  Input,
  Loading,
  SaveStatus,
  SecretModal,
  Section,
  Textarea,
  safeUrl,
} from "./ui";
import { Releases } from "./Management";
type Props = { app: App; refresh: () => void };
export function Setup({
  app,
  step,
  refresh,
  go,
}: Props & { step: number; go: (step: number) => Promise<void> }) {
  return step === 1 ? (
    <Details app={app} refresh={refresh} />
  ) : step === 2 ? (
    <Access app={app} refresh={refresh} />
  ) : step === 3 ? (
    <Packages app={app} refresh={refresh} />
  ) : step === 4 ? (
    <AppLinks app={app} refresh={refresh} />
  ) : step === 5 ? (
    <AppMedia app={app} refresh={refresh} />
  ) : step === 6 ? (
    <Webhooks app={app} />
  ) : (
    <Publish app={app} refresh={refresh} go={go} />
  );
}
function Details({ app, refresh }: Props) {
  const save = useAutosave(
    `/apps/${app.app_id}`,
    { name: app.name, description: app.description, tags: app.tags },
    "PATCH",
    refresh,
  );
  const [tags, setTags] = useState(app.tags.join(", "));
  return (
    <Section
      title="Make a good introduction"
      description="Tell Carbons and Silicons what your app makes possible."
    >
      <div className="form-stack">
        <Input
          label="App name"
          value={save.draft.name}
          onChange={(e) => save.update({ name: e.target.value })}
          maxLength={120}
        />
        <Input
          label="App ID"
          value={app.app_id}
          readOnly
          description="Your app’s permanent identity. It cannot be changed."
        />
        <Textarea
          label="Description"
          value={save.draft.description}
          onChange={(e) => save.update({ description: e.target.value })}
          rows={7}
          maxLength={600}
          description={`${save.draft.description.length}/600 characters. At least 200 characters before publishing.`}
        />
        <Input
          label="Tags"
          value={tags}
          onChange={(e) => {
            setTags(e.target.value);
            save.update({
              tags: [
                ...new Set(
                  e.target.value
                    .split(",")
                    .map((t) => t.trim())
                    .filter(Boolean),
                ),
              ].slice(0, 20),
            });
          }}
          description={`${save.draft.tags.length}/20 tags. Separate tags with commas.`}
        />
        <SaveStatus {...save} />
      </div>
    </Section>
  );
}
function Access({ app, refresh }: Props) {
  const save = useAutosave(
    `/apps/${app.app_id}/access`,
    {
      visibility: app.visibility,
      domains: app.domains || [],
      account_ids: app.account_ids || [],
    },
    "PUT",
    refresh,
  );
  const [domains, setDomains] = useState(save.draft.domains.join("\n"));
  const [accounts, setAccounts] = useState(save.draft.account_ids.join("\n"));
  return (
    <Section
      title="Choose who can find your app"
      description="Public apps are available to everyone. Private apps are only visible to the accounts you share them with."
    >
      <div className="form-stack">
        {!app.is_admin && (
          <div className="notice">
            <LockKeyhole size={18} />
            <p>
              Only this app’s administrator can change access. All authors can
              manage the other publishing steps.
            </p>
          </div>
        )}
        <fieldset className="access-options" disabled={!app.is_admin}>
          <legend className="sr-only">App visibility</legend>
          {(["public", "private"] as const).map((value) => (
            <label
              key={value}
              className={`access-option ${save.draft.visibility === value ? "selected" : ""}`}
            >
              <input
                type="radio"
                name="visibility"
                value={value}
                checked={save.draft.visibility === value}
                onChange={() => save.update({ visibility: value })}
              />
              {value === "public" ? (
                <Globe size={22} />
              ) : (
                <LockKeyhole size={22} />
              )}
              <span>
                <strong>{value === "public" ? "Public" : "Private"}</strong>
                <small>
                  {value === "public"
                    ? "Anyone can discover and install it, without signing in."
                    : "Only invited accounts and verified email domains can access it."}
                </small>
              </span>
            </label>
          ))}
        </fieldset>
        {save.draft.visibility === "private" && (
          <>
            <Textarea
              label="Share with accounts"
              placeholder={"c:alice\nsi:assistant"}
              value={accounts}
              disabled={!app.is_admin}
              onChange={(e) => setAccounts(e.target.value)}
              onBlur={() =>
                save.update({
                  account_ids: accounts
                    .split(/[\n,]/)
                    .map((x) => x.trim())
                    .filter(Boolean),
                })
              }
              description="One c:id or si:id per line. Accounts are resolved to their permanent UUIDs."
              rows={4}
            />
            <Textarea
              label="Allowed email domains"
              placeholder={"teamofsilicons.com\nexample.com"}
              value={domains}
              disabled={!app.is_admin}
              onChange={(e) => setDomains(e.target.value)}
              onBlur={() =>
                save.update({
                  domains: domains
                    .split(/[\n,]/)
                    .map((x) => x.trim().replace(/^@/, ""))
                    .filter(Boolean),
                })
              }
              description="One domain per line. Access requires a verified email on that domain."
              rows={3}
            />
          </>
        )}
        <SaveStatus {...save} />
      </div>
    </Section>
  );
}
function Packages({ app, refresh }: Props) {
  const packages = useResource<{ items: Package[] }>(
    `/apps/${app.app_id}/packages`,
  );
  const targetQuery = [
    ...new Set(packages.data?.items.map((p) => p.target) || []),
  ].join(",");
  const targets = useResource<{
    items: Target[];
    total_population: number | null;
    total_reach: number | null;
  }>(`/targets?targets=${encodeURIComponent(targetQuery)}`);
  const mutation = useMutation(() => {
    packages.reload();
    refresh();
  });
  const [target, setTarget] = useState<string>("macos-aarch64");
  const [file, setFile] = useState<File | null>(null);
  const [uploaded, setUploaded] = useState("");
  const [dragging, setDragging] = useState(false);
  const [fileError, setFileError] = useState<Error>();
  const selectFile = (selected?: File) => {
    if (selected && !selected.name.endsWith(".tar.gz")) {
      setFileError(
        new Error("Choose a .tar.gz package created with apps pack."),
      );
      return;
    }
    setFileError(undefined);
    setFile(selected || null);
    setUploaded("");
  };
  const supported = new Set(packages.data?.items.map((p) => p.target));
  const population = targets.data?.total_reach;
  return (
    <>
      <Section
        title="Give your app a command line"
        description="Every app begins with a CLI. Upload a package for at least one platform."
      >
        <div className="requirements-box">
          <h3>Three commands, on every platform</h3>
          <p>
            These commands help every Silicon find its way around your app. They
            are run when each package is uploaded.
          </p>
          <div className="required-command">
            <code>{app.app_id} --help</code>
            <span>Show the app’s help.</span>
          </div>
          <div className="required-command">
            <code>{app.app_id} accounts --json</code>
            <span>Return your app_id.</span>
          </div>
          <div className="required-command">
            <code>{app.app_id} login status --json</code>
            <span>
              Report authenticated, and the signed-in account when present.
            </span>
          </div>
        </div>
        <div className="form-stack">
          <label className="field-label">
            Package target
            <select
              className="native-input"
              value={target}
              onChange={(e) => setTarget(e.target.value)}
            >
              {TARGETS.map((t) => (
                <option value={t} key={t}>
                  {targetLabel(t)}
                </option>
              ))}
            </select>
          </label>
          <label
            className={`dropzone ${dragging ? "dragging" : ""}`}
            onDragOver={(e) => {
              e.preventDefault();
              setDragging(true);
            }}
            onDragLeave={() => setDragging(false)}
            onDrop={(e) => {
              e.preventDefault();
              setDragging(false);
              selectFile(e.dataTransfer.files[0]);
            }}
          >
            <Upload size={26} />
            <strong>
              {file ? file.name : "Choose a package or drop it here"}
            </strong>
            <span>A .tar.gz archive containing apps.yaml</span>
            <input
              type="file"
              accept=".tar.gz,application/gzip"
              onChange={(e) => selectFile(e.target.files?.[0])}
            />
          </label>
          <ErrorNotice error={fileError} />
          <Button
            disabled={!file}
            loading={mutation.pending}
            onClick={async () => {
              if (!file) return;
              const result = await mutation.run(() =>
                api<Package>(`/apps/${app.app_id}/packages/${target}`, {
                  method: "POST",
                  body: file,
                }),
              );
              if (result) {
                setUploaded(
                  `Package accepted for ${targetLabel(target)}. All three commands passed.`,
                );
                setFile(null);
              }
            }}
          >
            <Upload size={16} /> Upload and validate
          </Button>
          {uploaded && (
            <div className="notice success" role="status">
              <Check size={18} />
              {uploaded}
            </div>
          )}
          <ErrorNotice error={mutation.error} />
          <p className="small muted">
            Build locally with <code>apps pack</code>. An optional install
            script runs after installation.
          </p>
        </div>
      </Section>
      <Section
        title="Total addressable market"
        description="The Carbons and Silicons your uploaded targets can reach, based on accounts that registered a platform with Silicon Apps."
      >
        <div className="reach-summary">
          <strong>
            {population == null ? "Not available" : population.toLocaleString()}
          </strong>
          <span>{supported.size} of 9 targets supported</span>
        </div>
        <div className="target-grid">
          {(
            targets.data?.items ||
            TARGETS.map((target) => ({
              target,
              population: null,
              runner_available: false,
            }))
          ).map((t) => (
            <div className="target-row" key={t.target}>
              <div className="row">
                <span
                  className={`target-check ${supported.has(t.target) ? "supported" : ""}`}
                >
                  {supported.has(t.target) && <Check size={12} />}
                </span>
                <span>{targetLabel(t.target)}</span>
              </div>
              <span className="muted small">
                {t.population == null
                  ? "Not available"
                  : t.population.toLocaleString()}
              </span>
            </div>
          ))}
        </div>
        <ErrorNotice error={targets.error} retry={targets.reload} />
      </Section>
      <Section title="Uploaded packages">
        <ErrorNotice error={packages.error} retry={packages.reload} />
        {packages.loading ? (
          <Loading />
        ) : packages.data?.items.length ? (
          <div className="stack">
            {packages.data.items.map((pkg) => (
              <details className="package-detail" key={pkg.id}>
                <summary>
                  <span>{targetLabel(pkg.target)}</span>
                  <Badge tone="success">Validated</Badge>
                </summary>
                <p className="small muted">
                  {pkg.command} · {(pkg.size / 1024).toFixed(1)} KB
                </p>
                <code className="hash">SHA-256 {pkg.sha256}</code>
                {pkg.validation.map((v, i) => (
                  <div key={i} className="validation-result">
                    <strong>{v.command}</strong>
                    <p>Expected: {v.expected}</p>
                    <pre>
                      {v.stdout ||
                        v.stderr ||
                        `Exited with code ${v.exit_code}`}
                    </pre>
                  </div>
                ))}
              </details>
            ))}
          </div>
        ) : (
          <p className="muted">Your accepted packages will appear here.</p>
        )}
      </Section>
      <Releases
        app={app}
        refresh={refresh}
        compact
        packagesRevision={packages.data?.items.length || 0}
      />
    </>
  );
}
function AppLinks({ app, refresh }: Props) {
  const save = useAutosave(
    `/apps/${app.app_id}`,
    { links: { ...app.links, custom: app.links?.custom || [] } },
    "PATCH",
    refresh,
  );
  const links = save.draft.links;
  const update = (next: Partial<Links>) =>
    save.update({ links: { ...links, ...next } });
  return (
    <Section
      title="Connect the rest of your app"
      description="These links are optional. Add the places people can learn more or use your app."
    >
      <div className="form-stack">
        {(
          [
            { id: "developer_docs", label: "Developer docs" },
            { id: "website", label: "Website" },
            { id: "android", label: "Android app" },
            { id: "ios", label: "iOS app" },
          ] as const
        ).map((field) => (
          <Input
            key={field.id}
            label={field.label}
            type="url"
            value={links[field.id] || ""}
            placeholder="https://…"
            onChange={(e) => update({ [field.id]: e.target.value })}
          />
        ))}
        <div className="row between">
          <h3>Custom links</h3>
          <Button
            variant="secondary"
            size="sm"
            disabled={(links.custom?.length || 0) >= 4}
            onClick={() =>
              update({
                custom: [
                  ...(links.custom || []),
                  { label: "", url: "", logo: "" },
                ],
              })
            }
          >
            <Plus size={15} /> Add link
          </Button>
        </div>
        {links.custom?.map((link, index) => (
          <div className="editor-group" key={index}>
            <div className="row between">
              <span className="small">Link {index + 1}</span>
              <Button
                variant="ghost"
                size="sm"
                aria-label={`Remove link ${index + 1}`}
                onClick={() =>
                  update({
                    custom: links.custom?.filter((_, i) => i !== index),
                  })
                }
              >
                <Trash2 size={16} />
              </Button>
            </div>
            {(["label", "url", "logo"] as const).map((field) => (
              <Input
                key={field}
                label={
                  field === "label"
                    ? "Label"
                    : field === "url"
                      ? "URL"
                      : "Logo URL"
                }
                type={field === "label" ? "text" : "url"}
                value={link[field]}
                onChange={(e) =>
                  update({
                    custom: links.custom?.map((x, i) =>
                      i === index ? { ...x, [field]: e.target.value } : x,
                    ),
                  })
                }
              />
            ))}
          </div>
        ))}
        <SaveStatus {...save} />
      </div>
    </Section>
  );
}
function AppMedia({ app, refresh }: Props) {
  const save = useAutosave(
    `/apps/${app.app_id}`,
    {
      logo: app.logo,
      logo_alt: app.logo_alt || "",
      banner: app.banner,
      banner_alt: app.banner_alt || "",
      carousel: app.carousel || [],
    },
    "PATCH",
    refresh,
  );
  const mutation = useMutation();
  const [uploading, setUploading] = useState("");
  const updateMedia = (index: number, next: Partial<Media>) =>
    save.update({
      carousel: save.draft.carousel.map((item, i) =>
        i === index ? { ...item, ...next } : item,
      ),
    });
  const upload = async (
    file: File | undefined,
    destination: "logo" | "banner" | "carousel",
  ) => {
    if (!file) return;
    setUploading(destination);
    const result = await mutation.run(async () => {
      if (file.size > 100 * 1024 * 1024)
        throw new Error("Media must be 100 MB or smaller.");
      if (
        ![
          "image/png",
          "image/jpeg",
          "image/webp",
          "image/gif",
          "video/mp4",
          "video/webm",
        ].includes(file.type)
      )
        throw new Error("Choose a PNG, JPEG, WebP, GIF, MP4, or WebM file.");
      if (destination !== "carousel" && !file.type.startsWith("image/"))
        throw new Error("Choose an image for your logo or banner.");
      return api<{ url: string; kind: "image" | "video" }>(
        `/apps/${app.app_id}/media`,
        { method: "POST", body: file, contentType: file.type },
      );
    });
    if (result) {
      if (destination === "carousel")
        save.update({
          carousel: [
            ...save.draft.carousel,
            { url: result.url, kind: result.kind, alt: "" },
          ],
        });
      else save.update({ [destination]: result.url });
    }
    setUploading("");
  };
  return (
    <Section
      title="Show what your app can do"
      description="Add a logo, a banner, and up to 20 images or videos. All media is optional."
    >
      <div className="form-stack">
        {(["logo", "banner"] as const).map((kind) => (
          <div key={kind} className="stack">
            <div className="row between">
              <h3>{kind === "logo" ? "App logo" : "Banner"}</h3>
              <label className="upload-button">
                <Upload size={14} />
                {uploading === kind ? "Uploading…" : "Upload image"}
                <input
                  type="file"
                  accept="image/png,image/jpeg,image/webp,image/gif"
                  disabled={mutation.pending}
                  onChange={(e) => void upload(e.target.files?.[0], kind)}
                />
              </label>
            </div>
            <Input
              label={kind === "logo" ? "Logo URL" : "Banner URL"}
              value={save.draft[kind]}
              onChange={(e) => save.update({ [kind]: e.target.value })}
              placeholder="https://… or an uploaded media URL"
            />
            {safeUrl(save.draft[kind]) && (
              <img
                className={`media-preview ${kind === "logo" ? "logo-preview" : ""}`}
                src={safeUrl(save.draft[kind])}
                alt={`${kind} preview`}
              />
            )}
            <Textarea
              label={kind === "logo" ? "Logo alt text" : "Banner alt text"}
              value={save.draft[`${kind}_alt`]}
              maxLength={10000}
              rows={2}
              description="For Silicons and assistive technology. Not shown as a caption."
              onChange={(e) => save.update({ [`${kind}_alt`]: e.target.value })}
            />
          </div>
        ))}
        <div className="row between">
          <h3>Carousel ({save.draft.carousel.length}/20)</h3>
          <div className="row">
            <label
              className={`upload-button ${save.draft.carousel.length >= 20 ? "disabled" : ""}`}
            >
              <Upload size={14} />
              {uploading === "carousel" ? "Uploading…" : "Upload media"}
              <input
                type="file"
                accept="image/png,image/jpeg,image/webp,image/gif,video/mp4,video/webm"
                disabled={mutation.pending || save.draft.carousel.length >= 20}
                onChange={(e) => void upload(e.target.files?.[0], "carousel")}
              />
            </label>
            <Button
              variant="secondary"
              size="sm"
              disabled={save.draft.carousel.length >= 20}
              onClick={() =>
                save.update({
                  carousel: [
                    ...save.draft.carousel,
                    { url: "", kind: "image", alt: "" },
                  ],
                })
              }
            >
              <Plus size={15} /> Add URL
            </Button>
          </div>
        </div>
        <ErrorNotice error={mutation.error} />
        {save.draft.carousel.map((media, index) => (
          <div className="editor-group" key={index}>
            <div className="row between">
              <h3>Media {index + 1}</h3>
              <Button
                variant="ghost"
                aria-label={`Remove media ${index + 1}`}
                onClick={() =>
                  save.update({
                    carousel: save.draft.carousel.filter((_, i) => i !== index),
                  })
                }
              >
                <Trash2 size={16} />
              </Button>
            </div>
            <label className="field-label">
              Type
              <select
                className="native-input"
                value={media.kind}
                onChange={(e) =>
                  updateMedia(index, { kind: e.target.value as Media["kind"] })
                }
              >
                <option value="image">Image</option>
                <option value="video">Video</option>
              </select>
            </label>
            <Input
              label="Media URL"
              value={media.url}
              onChange={(e) => updateMedia(index, { url: e.target.value })}
            />
            <Textarea
              label="Alt text for Silicons"
              value={media.alt}
              maxLength={10000}
              rows={3}
              description={`${media.alt.length}/10,000 characters. Readable by Silicons and assistive technology, never shown as a caption.`}
              onChange={(e) => updateMedia(index, { alt: e.target.value })}
            />
          </div>
        ))}
        <SaveStatus {...save} />
      </div>
    </Section>
  );
}
type WebhookConfig = {
  url?: string | null;
  events?: string[] | null;
  secret_set?: boolean;
};
function Webhooks({ app }: { app: App }) {
  const resource = useResource<WebhookConfig>(`/apps/${app.app_id}/webhook`);
  return (
    <Section
      title="Stay in sync with Silicon Accounts"
      description="Receive signed updates when an account that signed into your app changes. Silicon Accounts stores this configuration and delivers every webhook."
    >
      <ErrorNotice error={resource.error} retry={resource.reload} />
      {resource.loading && !resource.data ? (
        <Loading label="Loading webhook configuration…" />
      ) : (
        <WebhookForm app={app} config={resource.data || {}} />
      )}
    </Section>
  );
}
function WebhookForm({ app, config }: { app: App; config: WebhookConfig }) {
  const mutation = useMutation();
  const [secret, setSecret] = useState<string | null>(null);
  const [hasSecret, setHasSecret] = useState(!!config.secret_set);
  const save = useAutosave(
    `/apps/${app.app_id}/webhook`,
    { url: config.url || "", events: config.events || DEFAULT_EVENTS },
    "PUT",
    (result) => {
      const value = result as { secret?: string; webhook_secret?: string };
      const generated = value.secret || value.webhook_secret;
      if (generated) {
        setSecret(generated);
        setHasSecret(true);
      }
    },
  );
  return (
    <div className="form-stack">
      <div className="notice">
        <Webhook size={20} />
        <p>
          Your endpoint receives only the updates you choose. Every delivery is
          signed with your webhook secret.
        </p>
      </div>
      <Button
        variant="secondary"
        loading={mutation.pending}
        onClick={async () => {
          const result = await mutation.run(() =>
            api<{ webhook_secret: string }>(
              `/apps/${app.app_id}/webhook/rotate`,
              { method: "POST", body: {} },
            ),
          );
          if (result) {
            setSecret(result.webhook_secret);
            setHasSecret(true);
          }
        }}
      >
        {hasSecret
          ? "Generate a new webhook secret"
          : "Configure webhook for updates from Silicon Accounts"}
      </Button>
      {hasSecret && (
        <p className="small success row">
          <Check size={14} /> A webhook secret is configured.
        </p>
      )}
      <Input
        label="Webhook endpoint"
        type="url"
        value={save.draft.url}
        placeholder="https://your-app.com/webhooks/accounts"
        description="Add an endpoint to save your update preferences. Changes save automatically."
        onChange={(e) => save.update({ url: e.target.value })}
      />
      <fieldset className="event-options">
        <legend>Updates to receive</legend>
        {EVENTS.map((event) => (
          <label key={event}>
            <input
              type="checkbox"
              checked={save.draft.events.includes(event)}
              onChange={(e) =>
                save.update({
                  events: e.target.checked
                    ? [...save.draft.events, event]
                    : save.draft.events.filter((x) => x !== event),
                })
              }
            />
            <span>
              <strong>{event.replace(/_/g, " ")}</strong>
              <code>{event}</code>
            </span>
          </label>
        ))}
      </fieldset>
      <ErrorNotice error={mutation.error} />
      <SaveStatus {...save} />
      <SecretModal
        secret={secret}
        title="Save your webhook secret"
        onClose={() => setSecret(null)}
      />
    </div>
  );
}
function Publish({
  app,
  refresh,
  go,
}: Props & { go: (step: number) => Promise<void> }) {
  const readiness = useResource<Readiness>(`/apps/${app.app_id}/readiness`);
  const mutation = useMutation(() => {
    refresh();
    readiness.reload();
  });
  const [published, setPublished] = useState(false);
  return (
    <Section
      title={app.published ? "Your app is live" : "Ready for the ecosystem?"}
      description="Review the details below. Publishing makes your app available immediately to everyone with access."
    >
      <div className="form-stack">
        <dl className="review-details">
          <div>
            <dt>App</dt>
            <dd>
              {app.name} <span className="muted">({app.app_id})</span>
            </dd>
          </div>
          <div>
            <dt>Description</dt>
            <dd>{app.description || "Not added"}</dd>
          </div>
          <div>
            <dt>Access</dt>
            <dd>
              {app.visibility === "public"
                ? "Public, available to everyone"
                : "Private, shared accounts and domains only"}
            </dd>
          </div>
          <div>
            <dt>Supported targets</dt>
            <dd>
              {app.targets.length
                ? app.targets.map(targetLabel).join(", ")
                : "No release packages yet"}
            </dd>
          </div>
          <div>
            <dt>Production</dt>
            <dd>
              {app.latest_production?.version || "No production release yet"}
            </dd>
          </div>
          <div>
            <dt>Development</dt>
            <dd>
              {app.latest_development?.version || "No development release yet"}
            </dd>
          </div>
          <div>
            <dt>Authors</dt>
            <dd>{app.authors.map((x) => x.id).join(", ")}</dd>
          </div>
        </dl>
        {!app.latest_production && (
          <div className="notice">
            <PackageIcon />
            <p>
              Default installs use production releases. After creating a
              development release, promote it in Releases so people can install
              with <code>apps install {app.app_id}</code>.
            </p>
          </div>
        )}
        <ErrorNotice error={readiness.error} retry={readiness.reload} />
        {readiness.loading ? (
          <Loading label="Checking required details…" />
        ) : readiness.data?.errors.length ? (
          <div className="requirements-box">
            <h3>A few things to finish</h3>
            {readiness.data.errors.map((error, index) => (
              <button
                className="readiness-error"
                key={index}
                onClick={() =>
                  void go(
                    error.field.includes("package") ||
                      error.field.includes("release")
                      ? 3
                      : error.field.includes("access")
                        ? 2
                        : 1,
                  )
                }
              >
                <span>{error.message}</span>
                <ExternalLink size={14} />
              </button>
            ))}
          </div>
        ) : (
          <div className="notice success">
            <Check size={18} />
            <p>
              The required steps are complete. Your app is ready to publish.
            </p>
          </div>
        )}
        <ErrorNotice error={mutation.error} />
        {published ? (
          <p role="status" className="success">
            Your app is published and available in the store.
          </p>
        ) : (
          <Button
            loading={mutation.pending}
            disabled={!readiness.data?.ready}
            onClick={async () => {
              await flushPendingSaves();
              const result = await mutation.run(() =>
                api(`/apps/${app.app_id}/publish`, {
                  method: "POST",
                  body: {},
                }),
              );
              if (result) setPublished(true);
            }}
          >
            {app.published ? "Publish changes" : "Publish app"}
          </Button>
        )}
        <p className="small muted">
          There is no review queue. Your authors choose when the app is ready.
        </p>
      </div>
    </Section>
  );
}
import { Package as PackageIcon } from "lucide-react";
