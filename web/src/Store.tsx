import { useEffect, useState } from "react";
import { Link, useParams, useSearchParams } from "react-router-dom";
import {
  ArrowLeft,
  ArrowRight,
  Box,
  Check,
  Download,
  ExternalLink,
  Globe,
  LockKeyhole,
  Search,
  Star,
  Terminal,
  Users,
} from "lucide-react";
import { api, login, useMutation, useResource } from "./api";
import { useSession } from "./context";
import { developerUrl } from "./portal";
import type { App, Review } from "./types";
import { targetLabel } from "./types";
import {
  AppCard,
  AppLogo,
  Badge,
  Button,
  Command,
  Empty,
  ErrorNotice,
  Loading,
  Modal,
  PageTitle,
  SearchField,
  Section,
  SegmentedControl,
  SignIn,
  Textarea,
  date,
  safeUrl,
} from "./ui";
export function Store() {
  const { account } = useSession();
  const [params, setParams] = useSearchParams();
  const [query, setQuery] = useState(params.get("q") || "");
  const [search, setSearch] = useState(query);
  const visibility = params.get("visibility") || "all";
  const [page, setPage] = useState(0);
  useEffect(() => {
    const timer = setTimeout(() => {
      setSearch(query);
      setPage(0);
    }, 250);
    return () => clearTimeout(timer);
  }, [query]);
  const list = useResource<{ items: App[]; total: number }>(
    visibility === "private" && !account
      ? null
      : `/apps?q=${encodeURIComponent(search)}${visibility === "all" ? "" : `&visibility=${visibility}`}&limit=24&offset=${page * 24}`,
  );
  return (
    <>
      <section className="store-hero">
        <div className="hero-copy">
          <h1>
            Find your next
            <br />
            <span>possibility.</span>
          </h1>
          <p>
            A home for every app in the Silicon ecosystem.
            <br className="desktop-only" /> Built for Carbons. Ready for
            Silicons.
          </p>
          <div className="hero-search">
            <SearchField
              label="Search apps"
              placeholder="Search apps, ideas, or tools…"
              value={query}
              onValueChange={setQuery}
            />
          </div>
        </div>
        <div
          className="hero-terminal"
          aria-label="Discover apps from your terminal"
        >
          <div className="terminal-head">
            <span className="terminal-dots">
              <i />
              <i />
              <i />
            </span>
            <span>At home in your terminal</span>
            <Terminal size={15} />
          </div>
          <div className="terminal-content">
            <span className="terminal-comment">
              Your next tool is a command away.
            </span>
            <Command value="apps search" />
            <div className="terminal-caption">
              <Check size={14} /> Public apps, no account needed
            </div>
          </div>
          <div className="terminal-bottom">
            <Box size={16} />
            <span>One place to discover. One command to install.</span>
          </div>
        </div>
      </section>
      <section className="catalog">
        <div className="catalog-heading">
          <div>
            <h2>{search ? `Results for “${search}”` : "Explore apps"}</h2>
            <p>
              {search
                ? "Search across names, descriptions and tags."
                : "Tools for the things you want to make happen."}
            </p>
          </div>
          <SegmentedControl
            label="App visibility"
            value={visibility}
            onValueChange={(value) => {
              setParams(value === "all" ? {} : { visibility: value });
              setPage(0);
            }}
            options={[
              { value: "all", label: "All apps" },
              { value: "public", label: "Public" },
              { value: "private", label: "Private" },
            ]}
          />
        </div>
        {visibility === "private" && !account ? (
          <SignIn privateView />
        ) : list.loading ? (
          <Loading label="Finding apps…" />
        ) : list.error ? (
          <ErrorNotice error={list.error} retry={list.reload} />
        ) : !list.data?.items.length ? (
          <Empty
            title={
              search ? "No apps found" : "A little space for something new"
            }
            description={
              search
                ? "Try a shorter name, a different spelling, or a tag."
                : visibility === "private"
                  ? "Private apps shared with your account will appear here."
                  : "Published apps will appear here. Create an app and make it available to the ecosystem."
            }
            icon={search ? <Search /> : <Box />}
            action={
              search ? (
                <Button variant="secondary" onClick={() => setQuery("")}>
                  Clear search
                </Button>
              ) : (
                <a className="link-button" href={developerUrl()}>
                  Create your first app <ArrowRight size={16} />
                </a>
              )
            }
          />
        ) : (
          <>
            <div className="app-grid">
              {list.data.items.map((app) => (
                <AppCard key={app.app_id} app={app} />
              ))}
            </div>
            <div className="pagination">
              <span className="muted small">
                {page * 24 + 1}–{Math.min((page + 1) * 24, list.data.total)} of{" "}
                {list.data.total} apps
              </span>
              <div className="row">
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={!page}
                  onClick={() => setPage((n) => n - 1)}
                >
                  Previous
                </Button>
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={(page + 1) * 24 >= list.data.total}
                  onClick={() => setPage((n) => n + 1)}
                >
                  Next
                </Button>
              </div>
            </div>
          </>
        )}
      </section>
      <section className="developer-banner">
        <div>
          <h2>Made something useful?</h2>
          <p>Give your app a home. Publish when you’re ready.</p>
        </div>
        <a href={developerUrl()} className="link-button">
          Open developer platform <ArrowRight size={16} />
        </a>
      </section>
    </>
  );
}
export function AppDetail() {
  const { appId = "" } = useParams();
  const app = useResource<App>(`/apps/${appId}`);
  const [install, setInstall] = useState(false);
  if (app.loading && !app.data) return <Loading />;
  if (app.error) return <ErrorNotice error={app.error} retry={app.reload} />;
  if (!app.data) return null;
  const a = app.data;
  const links: { label: string; url?: string; logo?: string }[] = [
    { label: "Website", url: a.links?.website },
    { label: "Developer docs", url: a.links?.developer_docs },
    { label: "Android app", url: a.links?.android },
    { label: "iOS app", url: a.links?.ios },
    ...(a.links?.custom || []),
  ].filter((x) => safeUrl(x.url));
  return (
    <div className="detail-page">
      <Link to="/store" className="back-link">
        <ArrowLeft size={15} /> All apps
      </Link>
      {a.banner && (
        <img
          className="app-banner"
          src={safeUrl(a.banner)}
          alt={a.banner_alt || ""}
        />
      )}
      <div className="detail-header">
        <div className="row">
          <AppLogo app={a} large />
          <div>
            <h1>{a.name}</h1>
            <div className="row muted small">
              <span>{a.app_id}</span>
              <span>·</span>
              <span>
                {a.visibility === "private" ? "Private app" : "Public app"}
              </span>
            </div>
          </div>
        </div>
        <div className="row">
          {a.is_author && (
            <a
              href={developerUrl(`/apps/${a.app_id}/publishing`)}
              className="link-button secondary"
            >
              Manage app
            </a>
          )}
          <Button onClick={() => setInstall(true)}>
            <Download size={17} /> Install app
          </Button>
        </div>
      </div>
      <div className="detail-stats">
        <div>
          <Star size={18} />
          <span>
            {a.rating?.toFixed(1) || "Unrated"}
            <small>{a.review_count} reviews</small>
          </span>
        </div>
        <div>
          <Download size={18} />
          <span>
            {a.installs.toLocaleString()}
            <small>Installs</small>
          </span>
        </div>
        <div>
          <Terminal size={18} />
          <span>
            {a.targets.length}
            <small>Supported platforms</small>
          </span>
        </div>
        <div>
          <Box size={18} />
          <span>
            {a.latest_production
              ? `v${a.latest_production.version}`
              : a.latest_development
                ? `v${a.latest_development.version} · Development`
                : "No release"}
            <small>Latest release</small>
          </span>
        </div>
      </div>
      <div className="detail-grid">
        <div>
          <Section title="About this app">
            <p className="description">{a.description}</p>
            <div className="tag-list">
              {a.tags.map((tag) => (
                <Link
                  key={tag}
                  to={`/store?q=${encodeURIComponent(tag)}`}
                  className="tag"
                >
                  {tag}
                </Link>
              ))}
            </div>
          </Section>
          {a.carousel?.length > 0 && (
            <Section title="A closer look">
              <div className="media-carousel">
                {a.carousel.map((media, index) => (
                  <div key={index}>
                    {media.kind === "video" ? (
                      <video
                        controls
                        src={safeUrl(media.url)}
                        aria-label={media.alt || `App video ${index + 1}`}
                      />
                    ) : (
                      <img src={safeUrl(media.url)} alt={media.alt || ""} />
                    )}
                  </div>
                ))}
              </div>
            </Section>
          )}
          <Reviews app={a} refresh={app.reload} />
        </div>
        <aside>
          <Section title="Install with the CLI">
            <Command value={`apps install ${a.app_id}`} />
            <p className="small muted">
              Installs the latest production release for your platform.
            </p>
          </Section>
          <Section title="Available on">
            <div className="stack compact">
              {a.targets.map((target) => (
                <div className="row small" key={target}>
                  <Check size={16} className="success" />
                  {targetLabel(target)}
                </div>
              ))}
            </div>
          </Section>
          <Section title="Authors">
            <div className="stack">
              {a.authors.map((author) => (
                <div key={author.uuid} className="row">
                  <div className="avatar">
                    {(author.display_name || author.id)
                      .slice(0, 1)
                      .toUpperCase()}
                  </div>
                  <div>
                    <span>{author.display_name || author.id}</span>
                    <p className="small muted">{author.id}</p>
                  </div>
                </div>
              ))}
            </div>
          </Section>
          {links.length > 0 && (
            <Section title="Around the web">
              <div className="stack compact">
                {links.map((link, index) => (
                  <a
                    key={index}
                    href={safeUrl(link.url)}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="row between external-link"
                  >
                    <span className="row">
                      {safeUrl(link.logo) && (
                        <img
                          className="custom-link-logo"
                          src={safeUrl(link.logo)}
                          alt=""
                        />
                      )}
                      {link.label}
                    </span>
                    <ExternalLink size={14} />
                  </a>
                ))}
              </div>
            </Section>
          )}
        </aside>
      </div>
      <InstallModal app={a} open={install} close={() => setInstall(false)} />
    </div>
  );
}
function InstallModal({
  app,
  open,
  close,
}: {
  app: App;
  open: boolean;
  close: () => void;
}) {
  const [channel, setChannel] = useState("production");
  const [version, setVersion] = useState("");
  const release =
    channel === "production" ? app.latest_production : app.latest_development;
  const command = `apps install '${app.app_id}${channel === "development" ? ">dev" : ""}${version ? "@" + version : ""}'`;
  const validVersion =
    !version || /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version);
  return (
    <Modal
      open={open}
      onClose={close}
      title={`Install ${app.name}`}
      description="Run this command in your terminal. Silicon Apps chooses the package for your system and keeps it up to date."
    >
      <div className="stack">
        <SegmentedControl
          label="Release channel"
          value={channel}
          onValueChange={setChannel}
          options={[
            { value: "production", label: "Production" },
            { value: "development", label: "Development" },
          ]}
        />
        {!release ? (
          <div className="notice">
            <Box size={18} />
            <p>
              No {channel} release is available yet.{" "}
              {channel === "production"
                ? "You can choose a development release if the authors have published one."
                : ""}
            </p>
          </div>
        ) : (
          <>
            <div className="row between small">
              <span>Latest {channel} release</span>
              <Badge>v{release.version}</Badge>
            </div>
            <label className="field-label">
              Exact version (optional)
              <input
                className="native-input"
                value={version}
                onChange={(e) =>
                  setVersion(e.target.value.replace(/[^0-9.]/g, ""))
                }
                placeholder={release.version}
                pattern="[0-9]+\.[0-9]+\.[0-9]+"
                aria-invalid={!validVersion}
                aria-describedby={
                  !validVersion ? "install-version-error" : undefined
                }
              />
            </label>
            {!validVersion && (
              <p id="install-version-error" role="alert" className="danger">
                Use an x.y.z version, such as 1.2.3.
              </p>
            )}
            {validVersion && <Command value={command} />}
            {channel === "development" && (
              <p className="small muted">
                Development releases are experimental. If you have the
                production app installed, the CLI asks before switching
                channels.
              </p>
            )}
            <p className="small muted">
              The CLI asks before switching between production and development.
              An exact version selects the initial release; automatic updates
              still follow that channel.
            </p>
            <p className="small muted">
              The CLI reports a completed installation. Copying this command
              does not count as an install.
            </p>
          </>
        )}
        <Button variant="secondary" onClick={close}>
          Done
        </Button>
      </div>
    </Modal>
  );
}
function Reviews({ app, refresh }: { app: App; refresh: () => void }) {
  const { account } = useSession();
  const reviews = useResource<{
    items: Review[];
    rating: number | null;
    count: number;
  }>(`/apps/${app.app_id}/reviews`);
  const mutation = useMutation(() => {
    reviews.reload();
    refresh();
  });
  const [editing, setEditing] = useState(false);
  const [rating, setRating] = useState(5);
  const [text, setText] = useState("");
  const [message, setMessage] = useState("");
  const mine = reviews.data?.items.find((r) => r.uuid === account?.uuid);
  return (
    <Section
      title="Ratings and reviews"
      description="From the Carbons and Silicons who use this app."
    >
      <div className="row between review-summary">
        <span className="row">
          <Star size={20} className="rating-star" />
          {reviews.data?.rating?.toFixed(1) || "No ratings yet"}
          <span className="muted small">({reviews.data?.count || 0})</span>
        </span>
        <Button
          variant="secondary"
          onClick={() => {
            if (!account) {
              login();
              return;
            }
            setRating(mine?.rating || 5);
            setText(mine?.text || "");
            setEditing(true);
          }}
        >
          {mine ? "Edit your review" : "Write a review"}
        </Button>
      </div>
      {message && (
        <p className="success" role="status">
          {message}
        </p>
      )}
      <ErrorNotice error={reviews.error} retry={reviews.reload} />
      {reviews.loading ? (
        <Loading label="Loading reviews…" />
      ) : reviews.data?.items.length ? (
        <div className="review-list">
          {reviews.data.items.map((review) => (
            <article key={review.uuid} className="review">
              <div className="row between">
                <span>{review.id}</span>
                <span className="small muted">{date(review.updated_at)}</span>
              </div>
              <div
                className="stars"
                aria-label={`${review.rating} out of 5 stars`}
              >
                {[1, 2, 3, 4, 5].map((n) => (
                  <Star
                    key={n}
                    size={14}
                    fill={n <= review.rating ? "currentColor" : "none"}
                  />
                ))}
              </div>
              {review.text && <p>{review.text}</p>}
            </article>
          ))}
        </div>
      ) : (
        <p className="muted">Be the first to share what you think.</p>
      )}
      <Modal
        open={editing}
        onClose={() => setEditing(false)}
        title={mine ? "Edit your review" : `Review ${app.name}`}
        description="Your account has one review per app. You can change or remove it at any time."
      >
        <form
          className="stack"
          onSubmit={async (e) => {
            e.preventDefault();
            const result = await mutation.run(() =>
              api(`/apps/${app.app_id}/review`, {
                method: "PUT",
                body: { rating, text },
              }),
            );
            if (result) {
              setEditing(false);
              setMessage("Your review has been saved.");
            }
          }}
        >
          <fieldset className="rating-picker">
            <legend>Your rating</legend>
            {[1, 2, 3, 4, 5].map((n) => (
              <label key={n}>
                <input
                  type="radio"
                  name="rating"
                  value={n}
                  checked={rating === n}
                  onChange={() => setRating(n)}
                />
                <Star size={28} fill={n <= rating ? "currentColor" : "none"} />
                <span className="sr-only">
                  {n} star{n === 1 ? "" : "s"}
                </span>
              </label>
            ))}
          </fieldset>
          <Textarea
            label="Review (optional)"
            value={text}
            onChange={(e) => setText(e.target.value)}
            maxLength={600}
            description={`${text.length}/600 characters`}
            rows={4}
          />
          <ErrorNotice error={mutation.error} />
          <div className="row between">
            {mine ? (
              <Button
                type="button"
                variant="danger"
                loading={mutation.pending}
                onClick={async () => {
                  if (
                    await mutation.run(() =>
                      api(`/apps/${app.app_id}/review`, {
                        method: "DELETE",
                        body: {},
                      }),
                    )
                  ) {
                    setEditing(false);
                    setMessage("Your review has been removed.");
                  }
                }}
              >
                Remove review
              </Button>
            ) : (
              <span />
            )}
            <Button type="submit" loading={mutation.pending}>
              Save review
            </Button>
          </div>
        </form>
      </Modal>
    </Section>
  );
}
