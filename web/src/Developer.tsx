import { useEffect, useState } from "react";
import {
  Link,
  useNavigate,
  useParams,
  useSearchParams,
} from "react-router-dom";
import {
  ArrowLeft,
  ArrowRight,
  Check,
  ChevronRight,
  ExternalLink,
  FileText,
  History,
  Package as PackageIcon,
  Plus,
  Users,
} from "lucide-react";
import { api, flushPendingSaves, useMutation, useResource } from "./api";
import { useSession } from "./context";
import type { App } from "./types";
import { STEPS } from "./types";
import {
  AppCard,
  AppLogo,
  Badge,
  Button,
  Empty,
  ErrorNotice,
  Input,
  Loading,
  Modal,
  PageTitle,
  SecretModal,
  SignIn,
  Textarea,
} from "./ui";
import { Setup } from "./Setup";
import { Authors, AppHistory, Releases } from "./Management";
export function Developer() {
  const { account, loading } = useSession();
  const apps = useResource<{ items: App[]; total: number }>(
    account ? "/apps?mine=true&limit=100" : null,
  );
  const [create, setCreate] = useState(false);
  if (loading) return <Loading />;
  return (
    <>
      <PageTitle
        title="Your apps"
        description="An idea, a package, a place in the ecosystem."
        action={
          account ? (
            <Button onClick={() => setCreate(true)}>
              <Plus size={16} /> Create app
            </Button>
          ) : undefined
        }
      />
      {!account ? (
        <SignIn />
      ) : apps.loading ? (
        <Loading label="Loading your apps…" />
      ) : apps.error ? (
        <ErrorNotice error={apps.error} retry={apps.reload} />
      ) : apps.data?.items.length ? (
        <div className="app-grid">
          {apps.data.items.map((app) => (
            <AppCard app={app} developer key={app.app_id} />
          ))}
        </div>
      ) : (
        <Empty
          title="From an idea to an app"
          description="Create an app in a few seconds. Add your packages, bring in your co-authors, and publish when you’re ready."
          action={
            <Button onClick={() => setCreate(true)}>
              <Plus size={16} /> Create your first app
            </Button>
          }
        />
      )}
      <div className="developer-intro">
        <div>
          <FileText size={22} />
          <h3>Start small</h3>
          <p>
            A name and an app ID are all you need. Come back to finish the
            details.
          </p>
        </div>
        <div>
          <PackageIcon size={22} />
          <h3>Built around your CLI</h3>
          <p>
            Upload packages for the platforms you support. One manifest connects
            it all.
          </p>
        </div>
        <div>
          <Users size={22} />
          <h3>Make it together</h3>
          <p>
            Invite Carbons and Silicons to author your app. Every author can
            contribute.
          </p>
        </div>
      </div>
      <CreateApp open={create} close={() => setCreate(false)} />
    </>
  );
}
function CreateApp({ open, close }: { open: boolean; close: () => void }) {
  const navigate = useNavigate();
  const [name, setName] = useState("");
  const [id, setId] = useState("");
  const [description, setDescription] = useState("");
  const [logo, setLogo] = useState("");
  const [checked, setChecked] = useState("");
  const [secret, setSecret] = useState<string | null>(null);
  const [created, setCreated] = useState("");
  const mutation = useMutation();
  useEffect(() => {
    const timer = setTimeout(() => setChecked(id), 300);
    return () => clearTimeout(timer);
  }, [id]);
  const availability = useResource<{ available: boolean }>(
    /^[a-z0-9_-]{3,30}$/.test(checked) ? `/apps/availability/${checked}` : null,
  );
  const valid = /^[a-z0-9_-]{3,30}$/.test(id);
  return (
    <>
      <Modal
        open={open && !secret}
        onClose={close}
        title="Create an app"
        description="Start with the basics. Your app exists immediately, and you can finish setting it up whenever you like."
      >
        <form
          className="stack"
          onSubmit={async (e) => {
            e.preventDefault();
            const result = await mutation.run(() =>
              api<{ app: App; app_secret: string }>("/apps", {
                method: "POST",
                body: { name, app_id: id, description, logo },
              }),
            );
            if (result) {
              setCreated(result.app.app_id);
              setSecret(result.app_secret);
            }
          }}
        >
          <Input
            label="App name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
            maxLength={120}
            placeholder="My useful app"
          />
          <Input
            label="App ID"
            value={id}
            onChange={(e) => setId(e.target.value.toLowerCase())}
            required
            minLength={3}
            maxLength={30}
            pattern="[a-z0-9_-]{3,30}"
            placeholder="my-useful-app"
            description="3–30 lowercase letters, numbers, hyphens, or underscores. This cannot be changed."
            error={
              id && !valid
                ? "Choose a valid app ID."
                : id === checked &&
                    availability.data &&
                    !availability.data.available
                  ? "This app ID is already in use."
                  : undefined
            }
          />
          {id === checked && availability.data?.available && (
            <p className="small success row">
              <Check size={14} /> This app ID is available
            </p>
          )}
          <Textarea
            label="Description (optional)"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            rows={3}
            maxLength={600}
          />
          <Input
            label="Logo URL (optional)"
            value={logo}
            onChange={(e) => setLogo(e.target.value)}
            type="url"
            placeholder="https://…"
          />
          <ErrorNotice error={mutation.error} />
          <Button
            type="submit"
            disabled={
              !name.trim() ||
              !valid ||
              !availability.data?.available ||
              id !== checked
            }
            loading={mutation.pending}
          >
            Create app <ArrowRight size={16} />
          </Button>
        </form>
      </Modal>
      <SecretModal
        secret={secret}
        onClose={() => {
          setSecret(null);
          close();
          navigate(`/developer/apps/${created}`);
        }}
      />
    </>
  );
}
export function AppWorkspace() {
  const { appId = "" } = useParams();
  const [params, setParams] = useSearchParams();
  const resource = useResource<App>(`/apps/${appId}`);
  const [tab, setTab] = useState("setup");
  const [navigationError, setNavigationError] = useState<Error>();
  const app = resource.data;
  const step = Math.max(
    1,
    Math.min(7, Number(params.get("step") || app?.setup_step || 1)),
  );
  const navigateStep = async (next: number) => {
    try {
      await flushPendingSaves();
      await api(`/apps/${appId}`, {
        method: "PATCH",
        body: { setup_step: next },
      });
      setParams({ step: String(next) });
      setNavigationError(undefined);
    } catch (error) {
      setNavigationError(error as Error);
    }
  };
  if (resource.loading && !app) return <Loading />;
  if (resource.error)
    return <ErrorNotice error={resource.error} retry={resource.reload} />;
  if (!app) return null;
  if (!app.is_author)
    return (
      <Empty
        title="This workspace is for the app’s authors"
        description="You can find the app and its public details in the store."
        action={
          <Link className="link-button" to={`/store/${app.app_id}`}>
            View app
          </Link>
        }
      />
    );
  return (
    <>
      <Link to="/developer" className="back-link">
        <ArrowLeft size={15} /> Your apps
      </Link>
      <div className="workspace-title">
        <div className="row">
          <AppLogo app={app} />
          <div>
            <h1>{app.name}</h1>
            <p className="small muted">{app.app_id}</p>
          </div>
          <Badge tone={app.published ? "success" : "warning"}>
            {app.published ? "Published" : "Continue setup"}
          </Badge>
        </div>
        {app.published && (
          <Link className="link-button secondary" to={`/store/${app.app_id}`}>
            View in store <ExternalLink size={15} />
          </Link>
        )}
      </div>
      <nav className="workspace-tabs" aria-label="App management">
        {[
          {
            id: "setup",
            label: app.published ? "App settings" : "Continue setup",
            icon: FileText,
          },
          { id: "releases", label: "Releases", icon: PackageIcon },
          { id: "authors", label: "Authors", icon: Users },
          { id: "history", label: "History", icon: History },
        ].map((item) => (
          <button
            key={item.id}
            className={tab === item.id ? "active" : ""}
            aria-current={tab === item.id ? "page" : undefined}
            onClick={async () => {
              try {
                await flushPendingSaves();
                setTab(item.id);
                setNavigationError(undefined);
              } catch (error) {
                setNavigationError(error as Error);
              }
            }}
          >
            <item.icon size={16} />
            {item.label}
          </button>
        ))}
      </nav>
      <ErrorNotice error={navigationError} />
      {tab === "setup" ? (
        <div className="setup-layout">
          <aside className="setup-sidebar">
            <nav aria-label="Publishing steps">
              {STEPS.map((label, index) => (
                <button
                  key={label}
                  className={`step-link ${step === index + 1 ? "active" : ""}`}
                  aria-current={step === index + 1 ? "step" : undefined}
                  onClick={() => void navigateStep(index + 1)}
                >
                  <span className="step-number">{index + 1}</span>
                  <span>
                    {label}
                    {index < 3 && <small>Required</small>}
                  </span>
                  {step === index + 1 && <ChevronRight size={15} />}
                </button>
              ))}
            </nav>
            <p className="small muted">
              Move freely between steps. Your progress is saved as you go.
            </p>
          </aside>
          <div className="setup-main">
            <Setup
              key={`${appId}-${step}`}
              app={app}
              step={step}
              refresh={resource.reload}
              go={navigateStep}
            />
            {step < 7 && (
              <div className="setup-footer">
                <Button
                  variant="secondary"
                  disabled={step === 1}
                  onClick={() => void navigateStep(step - 1)}
                >
                  Back
                </Button>
                <Button onClick={() => void navigateStep(step + 1)}>
                  Continue <ArrowRight size={16} />
                </Button>
              </div>
            )}
          </div>
        </div>
      ) : tab === "releases" ? (
        <Releases app={app} refresh={resource.reload} />
      ) : tab === "authors" ? (
        <Authors app={app} refresh={resource.reload} />
      ) : (
        <AppHistory app={app} />
      )}
    </>
  );
}
