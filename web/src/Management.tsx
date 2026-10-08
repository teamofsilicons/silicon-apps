import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  ArrowRight,
  Check,
  Clock,
  KeyRound,
  LogOut,
  Mail,
  Plus,
  ShieldCheck,
  Trash2,
  UserMinus,
  UserPlus,
  Users,
} from "lucide-react";
import { api, useMutation, useResource } from "./api";
import { useSession } from "./context";
import type { App, Author, Invite, Package, Release } from "./types";
import { targetLabel } from "./types";
import {
  Badge,
  Button,
  Empty,
  ErrorNotice,
  Input,
  Loading,
  Modal,
  PageTitle,
  SecretModal,
  Section,
  SignIn,
  Textarea,
  date,
} from "./ui";
export function Releases({
  app,
  refresh,
  compact = false,
  packagesRevision = 0,
}: {
  app: App;
  refresh: () => void;
  compact?: boolean;
  packagesRevision?: number;
}) {
  const releases = useResource<{ items: Release[] }>(
    `/apps/${app.app_id}/releases`,
  );
  const packages = useResource<{ items: Package[] }>(
    `/apps/${app.app_id}/packages`,
    packagesRevision,
  );
  const mutation = useMutation(() => {
    releases.reload();
    refresh();
  });
  const [create, setCreate] = useState(false);
  const [version, setVersion] = useState("");
  const [notes, setNotes] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  const [promote, setPromote] = useState<Release | null>(null);
  const [production, setProduction] = useState("");
  const [message, setMessage] = useState("");
  const validVersion = (value: string) =>
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(value);
  return (
    <Section
      title={compact ? "Create your first release" : "Releases"}
      description="Every release begins in development. Promote it with an independent production version when it’s ready."
    >
      <div className="row between section-actions">
        <span className="small muted">
          {releases.data?.items.length || 0} releases
        </span>
        <Button
          onClick={() => {
            setCreate(true);
            mutation.clearError();
          }}
        >
          <Plus size={16} /> Create release
        </Button>
      </div>
      {message && (
        <div className="notice success" role="status">
          <Check size={18} />
          {message}
        </div>
      )}
      <ErrorNotice error={releases.error} retry={releases.reload} />
      {releases.loading ? (
        <Loading />
      ) : releases.data?.items.length ? (
        <div className="release-list">
          {releases.data.items.map((release) => (
            <article className="release" key={release.id}>
              <div className="row between">
                <div className="row">
                  <h3>v{release.version}</h3>
                  <Badge
                    tone={release.channel === "production" ? "success" : "info"}
                  >
                    {release.channel}
                  </Badge>
                </div>
                {release.channel === "development" && (
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => {
                      setPromote(release);
                      setProduction("");
                      mutation.clearError();
                    }}
                  >
                    Promote <ArrowRight size={14} />
                  </Button>
                )}
              </div>
              <p className="small muted">
                {date(release.created_at)} · {release.package_ids.length}{" "}
                package{release.package_ids.length === 1 ? "" : "s"}
              </p>
              {release.notes && (
                <p className="release-notes">{release.notes}</p>
              )}
              {release.promoted_from && (
                <p className="small muted">
                  Promoted from development release {release.promoted_from}
                </p>
              )}
            </article>
          ))}
        </div>
      ) : (
        <Empty
          title="A release brings your packages together"
          description="Choose a version and the packages it should contain. New releases keep all your app’s other details."
        />
      )}
      <Modal
        open={create}
        onClose={() => setCreate(false)}
        title="Create development release"
        description="Select one validated package for each target you want to include."
      >
        <form
          className="stack"
          onSubmit={async (e) => {
            e.preventDefault();
            const result = await mutation.run(() =>
              api<Release>(`/apps/${app.app_id}/releases`, {
                method: "POST",
                body: { version, notes, package_ids: selected },
              }),
            );
            if (result) {
              setCreate(false);
              setMessage(`Development release ${result.version} created.`);
              setVersion("");
              setNotes("");
              setSelected([]);
            }
          }}
        >
          <Input
            label="Development version"
            placeholder="1.0.0"
            value={version}
            onChange={(e) => setVersion(e.target.value)}
            pattern="[0-9]+\.[0-9]+\.[0-9]+"
            required
          />
          <Textarea
            label="Release notes (optional)"
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            rows={3}
          />
          <fieldset className="package-picker">
            <legend>Validated packages</legend>
            {packages.data?.items.length ? (
              packages.data.items.map((pkg) => (
                <label key={pkg.id}>
                  <input
                    type="checkbox"
                    checked={selected.includes(pkg.id)}
                    onChange={(e) =>
                      setSelected((current) =>
                        e.target.checked
                          ? [
                              ...current.filter(
                                (id) =>
                                  packages.data?.items.find((p) => p.id === id)
                                    ?.target !== pkg.target,
                              ),
                              pkg.id,
                            ]
                          : current.filter((id) => id !== pkg.id),
                      )
                    }
                  />
                  <span>
                    {targetLabel(pkg.target)}
                    <small>
                      {date(pkg.created_at)} · {pkg.sha256.slice(0, 12)}
                    </small>
                  </span>
                </label>
              ))
            ) : (
              <p className="muted">
                Upload and validate a package in the Packages step first.
              </p>
            )}
          </fieldset>
          <ErrorNotice error={packages.error} retry={packages.reload} />
          <ErrorNotice error={mutation.error} />
          <Button
            type="submit"
            disabled={!selected.length || !validVersion(version)}
            loading={mutation.pending}
          >
            Create release
          </Button>
        </form>
      </Modal>
      <Modal
        open={!!promote}
        onClose={() => setPromote(null)}
        title="Promote to production"
        description={`The validated packages from development ${promote?.version || ""} become a new production release. Existing releases remain unchanged.`}
      >
        <form
          className="stack"
          onSubmit={async (e) => {
            e.preventDefault();
            if (!promote) return;
            const result = await mutation.run(() =>
              api<Release>(
                `/apps/${app.app_id}/releases/${promote.id}/promote`,
                { method: "POST", body: { version: production } },
              ),
            );
            if (result) {
              setPromote(null);
              setMessage(`Production release ${result.version} is available.`);
            }
          }}
        >
          <Input
            label="Production version"
            value={production}
            onChange={(e) => setProduction(e.target.value)}
            placeholder="1.0.0"
            required
            description="Production and development versions are independent."
          />
          <ErrorNotice error={mutation.error} />
          <Button
            type="submit"
            disabled={!validVersion(production)}
            loading={mutation.pending}
          >
            Promote release
          </Button>
        </form>
      </Modal>
    </Section>
  );
}
export function Authors({ app, refresh }: { app: App; refresh: () => void }) {
  const navigate = useNavigate();
  const { account } = useSession();
  const authors = useResource<{ items: Author[] }>(
    `/apps/${app.app_id}/authors`,
  );
  const invites = useResource<{ items: Invite[] }>(
    `/apps/${app.app_id}/invites`,
  );
  const mutation = useMutation(() => {
    authors.reload();
    invites.reload();
    refresh();
  });
  const [to, setTo] = useState("");
  const [message, setMessage] = useState("");
  const [confirm, setConfirm] = useState<{
    title: string;
    description: string;
    action: () => Promise<unknown>;
  } | null>(null);
  const [secret, setSecret] = useState<string | null>(null);
  const list = authors.data?.items || app.authors;
  return (
    <>
      <Section
        title="Build with others"
        description="Invite Carbons and Silicons to become co-authors. They appear on your app after accepting, and every author can contribute."
      >
        <form
          className="invite-form"
          onSubmit={async (e) => {
            e.preventDefault();
            const result = await mutation.run(() =>
              api(`/apps/${app.app_id}/invites`, {
                method: "POST",
                body: { to },
              }),
            );
            if (result) {
              setMessage(`Invitation created for ${to}.`);
              setTo("");
            }
          }}
        >
          <Input
            label="Invite an author"
            value={to}
            onChange={(e) => setTo(e.target.value)}
            placeholder="c:alice, si:assistant, or email address"
            required
          />
          <Button type="submit" loading={mutation.pending}>
            <UserPlus size={16} /> Send invitation
          </Button>
        </form>
        {message && (
          <p className="success" role="status">
            {message}
          </p>
        )}
        <ErrorNotice error={mutation.error} />
        <ErrorNotice error={authors.error} retry={authors.reload} />
        <div className="members-list">
          {list.map((author) => (
            <div className="member-row" key={author.uuid}>
              <div className="row">
                <div className="avatar">
                  {(author.display_name || author.id).slice(0, 1).toUpperCase()}
                </div>
                <div>
                  <span>
                    {author.display_name || author.id}
                    {author.uuid === account?.uuid && (
                      <span className="muted"> (you)</span>
                    )}
                  </span>
                  <p className="small muted">
                    {author.id} · Joined {date(author.joined_at)}
                  </p>
                </div>
              </div>
              {app.is_admin && author.uuid !== account?.uuid && (
                <div className="row">
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() =>
                      setConfirm({
                        title: "Transfer administration?",
                        description: `${author.id} will manage access changes and member removal. You will remain an author.`,
                        action: () =>
                          api(`/apps/${app.app_id}/admin`, {
                            method: "POST",
                            body: { uuid: author.uuid },
                          }),
                      })
                    }
                  >
                    Make admin
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    aria-label={`Remove ${author.id}`}
                    onClick={() =>
                      setConfirm({
                        title: `Remove ${author.id}?`,
                        description:
                          "This account will lose author access to the app. You can invite them again later.",
                        action: () =>
                          api(`/apps/${app.app_id}/authors/${author.uuid}`, {
                            method: "DELETE",
                            body: {},
                          }),
                      })
                    }
                  >
                    <UserMinus size={16} />
                  </Button>
                </div>
              )}
            </div>
          ))}
        </div>
      </Section>
      <Section title="Pending invitations">
        <ErrorNotice error={invites.error} retry={invites.reload} />
        {invites.data?.items.filter((i) => i.status === "pending").length ? (
          <div className="members-list">
            {invites.data.items
              .filter((i) => i.status === "pending")
              .map((invite) => (
                <div className="member-row" key={invite.id}>
                  <div>
                    <span>{invite.to}</span>
                    <p className="small muted">
                      Invited {date(invite.created_at)}
                    </p>
                  </div>
                  <Button
                    variant="secondary"
                    size="sm"
                    loading={mutation.pending}
                    onClick={async () => {
                      if (
                        await mutation.run(() =>
                          api(`/apps/${app.app_id}/invites/${invite.id}`, {
                            method: "DELETE",
                            body: {},
                          }),
                        )
                      )
                        setMessage("Invitation cancelled.");
                    }}
                  >
                    Cancel invitation
                  </Button>
                </div>
              ))}
          </div>
        ) : (
          <p className="muted">No invitations are waiting for a response.</p>
        )}
      </Section>
      <Section
        title="App credentials"
        description="All authors can rotate the app secret. The previous secret stops working immediately."
      >
        <Button
          variant="secondary"
          onClick={() =>
            setConfirm({
              title: "Rotate app secret?",
              description:
                "Any service using the current secret will need the new secret immediately. The new value will be shown only once.",
              action: async () => {
                const result = await api<{ app_secret: string }>(
                  `/apps/${app.app_id}/secret/rotate`,
                  { method: "POST", body: {} },
                );
                setSecret(result.app_secret);
                return result;
              },
            })
          }
        >
          <KeyRound size={16} /> Rotate app secret
        </Button>
      </Section>
      <Section
        title="Leave this app"
        description={
          list.length === 1
            ? "You are the last author. Invite another author and wait for them to join before leaving."
            : "You can leave at any time. If you administer this app, administration passes to the oldest remaining author."
        }
      >
        <Button
          variant="danger"
          disabled={list.length === 1}
          onClick={() =>
            setConfirm({
              title: `Leave ${app.name}?`,
              description:
                "You will lose access to this workspace. Another author will need to invite you if you want to rejoin.",
              action: async () => {
                const result = await api(`/apps/${app.app_id}/authors/leave`, {
                  method: "POST",
                  body: {},
                });
                navigate("/developer");
                return result;
              },
            })
          }
        >
          <LogOut size={16} /> Leave app
        </Button>
      </Section>
      <Modal
        open={!!confirm}
        onClose={() => setConfirm(null)}
        title={confirm?.title || "Confirm change"}
        description={confirm?.description}
      >
        <div className="stack">
          <ErrorNotice error={mutation.error} />
          <div className="row end">
            <Button variant="secondary" onClick={() => setConfirm(null)}>
              Cancel
            </Button>
            <Button
              variant="danger"
              loading={mutation.pending}
              onClick={async () => {
                if (confirm && (await mutation.run(confirm.action))) {
                  setConfirm(null);
                  setMessage("Your change has been saved.");
                }
              }}
            >
              Confirm
            </Button>
          </div>
        </div>
      </Modal>
      <SecretModal secret={secret} onClose={() => setSecret(null)} />
    </>
  );
}
export function Invitations() {
  const { account, loading } = useSession();
  const invitations = useResource<{ items: Invite[] }>(
    account ? "/invites" : null,
  );
  const mutation = useMutation(invitations.reload);
  const [message, setMessage] = useState("");
  if (loading) return <Loading />;
  return (
    <>
      <PageTitle
        title="Invitations"
        description="Good things are built together. Join an app as a co-author."
      />
      {!account ? (
        <SignIn />
      ) : invitations.loading ? (
        <Loading />
      ) : invitations.error ? (
        <ErrorNotice error={invitations.error} retry={invitations.reload} />
      ) : (
        <>
          {message && (
            <div className="notice success" role="status">
              <Check size={18} />
              {message}
            </div>
          )}
          <ErrorNotice error={mutation.error} />
          {invitations.data?.items.filter((i) => i.status === "pending")
            .length ? (
            <div className="invite-list">
              {invitations.data.items
                .filter((i) => i.status === "pending")
                .map((invite) => (
                  <article className="invitation" key={invite.id}>
                    <div className="row">
                      <Mail size={23} />
                      <div>
                        <h2>{invite.app_id}</h2>
                        <p>
                          Invited as {invite.to} · {date(invite.created_at)}
                        </p>
                      </div>
                    </div>
                    <div className="row">
                      <Button
                        variant="secondary"
                        loading={mutation.pending}
                        onClick={async () => {
                          if (
                            await mutation.run(() =>
                              api(`/invites/${invite.id}/decline`, {
                                method: "POST",
                                body: {},
                              }),
                            )
                          )
                            setMessage("Invitation declined.");
                        }}
                      >
                        Decline
                      </Button>
                      <Button
                        loading={mutation.pending}
                        onClick={async () => {
                          if (
                            await mutation.run(() =>
                              api(`/invites/${invite.id}/accept`, {
                                method: "POST",
                                body: {},
                              }),
                            )
                          )
                            setMessage(
                              `You are now an author of ${invite.app_id}. Find it in Your apps.`,
                            );
                        }}
                      >
                        Accept invitation
                      </Button>
                    </div>
                  </article>
                ))}
            </div>
          ) : (
            <Empty
              title="You’re all caught up"
              description="Invitations to become an app author will appear here."
              icon={<Mail />}
            />
          )}
        </>
      )}
    </>
  );
}
export function AppHistory({ app }: { app: App }) {
  const [page, setPage] = useState(0);
  const history = useResource<{
    items: {
      id: string;
      at: string;
      actor_uuid: string;
      kind: string;
      data: unknown;
    }[];
    total: number;
  }>(`/apps/${app.app_id}/history?limit=30&offset=${page * 30}`);
  return (
    <Section
      title="App history"
      description="A durable record of releases, promotions, packages, authors, access, and details."
    >
      <ErrorNotice error={history.error} retry={history.reload} />
      {history.loading ? (
        <Loading />
      ) : history.data?.items.length ? (
        <>
          <ol className="history-list">
            {history.data.items.map((event) => (
              <li key={event.id}>
                <span className="history-dot" />
                <div>
                  <div className="row between">
                    <h3>{event.kind.replace(/[._]/g, " ")}</h3>
                    <time className="small muted" dateTime={event.at}>
                      {new Date(event.at).toLocaleString()}
                    </time>
                  </div>
                  <p className="small muted">
                    {app.authors.find((a) => a.uuid === event.actor_uuid)?.id ||
                      event.actor_uuid ||
                      "System"}
                  </p>
                  <details>
                    <summary>View event details</summary>
                    <pre>{JSON.stringify(event.data, null, 2)}</pre>
                  </details>
                </div>
              </li>
            ))}
          </ol>
          <div className="pagination">
            <span className="small muted">{history.data.total} events</span>
            <div className="row">
              <Button
                variant="secondary"
                disabled={!page}
                onClick={() => setPage((p) => p - 1)}
              >
                Previous
              </Button>
              <Button
                variant="secondary"
                disabled={(page + 1) * 30 >= history.data.total}
                onClick={() => setPage((p) => p + 1)}
              >
                Next
              </Button>
            </div>
          </div>
        </>
      ) : (
        <Empty
          title="Your app’s story starts here"
          description="Changes to this app will appear in this history."
          icon={<Clock />}
        />
      )}
    </Section>
  );
}
