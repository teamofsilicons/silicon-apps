import { useEffect, useState, type MouseEvent } from "react";
import {
  BrowserRouter,
  Link,
  NavLink,
  Navigate,
  Route,
  Routes,
  useLocation,
  useNavigate,
} from "react-router-dom";
import {
  ArrowUpRight,
  BookOpen,
  Code2,
  Grid2X2,
  Mail,
  Menu,
  Settings as SettingsIcon,
  Terminal,
  X,
} from "lucide-react";
import { api, flushPendingSaves, login, useResource } from "./api";
import { SessionContext } from "./context";
import type { Account } from "./types";
import { AppWorkspace, Developer } from "./Developer";
import { Invitations } from "./Management";
import { AppDetail, Store } from "./Store";
import { Settings, telemetry } from "./Settings";
import {
  Button,
  Command,
  Empty,
  ErrorNotice,
  IconLogo,
  PageTitle,
  Section,
} from "./ui";
function Shell() {
  const location = useLocation();
  const navigate = useNavigate();
  const session = useResource<{
    authenticated: boolean;
    account: Account | null;
  }>("/session");
  const [mobile, setMobile] = useState(false);
  const [navigationError, setNavigationError] = useState<Error>();
  const developer = location.pathname.startsWith("/developer");
  const account = session.data?.account || null;
  useEffect(() => {
    document.documentElement.dataset.theme =
      localStorage.getItem("apps.theme") || "light";
  }, []);
  useEffect(() => {
    setMobile(false);
    window.scrollTo(0, 0);
    document.title = developer ? "Silicon Apps · Developer" : "Silicon Apps";
    void telemetry("page_view", { path: location.pathname });
  }, [location.pathname, developer]);
  const handleNavigation = (event: MouseEvent<HTMLDivElement>) => {
    if (
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    )
      return;
    const anchor = (event.target as HTMLElement).closest(
      "a[href]",
    ) as HTMLAnchorElement | null;
    if (
      !anchor ||
      anchor.target === "_blank" ||
      anchor.hasAttribute("download")
    )
      return;
    const url = new URL(anchor.href);
    if (
      url.origin !== window.location.origin ||
      url.protocol !== window.location.protocol ||
      (url.pathname === location.pathname &&
        url.search === location.search &&
        url.hash)
    )
      return;
    event.preventDefault();
    void flushPendingSaves()
      .then(() => {
        setNavigationError(undefined);
        navigate(url.pathname + url.search + url.hash);
      })
      .catch((error) => setNavigationError(error as Error));
  };
  return (
    <SessionContext.Provider
      value={{ account, loading: session.loading, refresh: session.reload }}
    >
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <div className="app-shell" onClickCapture={handleNavigation}>
        <header className="site-header">
          <div className="header-inner">
            <Link to="/store" className="brand" aria-label="Silicon Apps home">
              <IconLogo />
              <span>
                Silicon <span className="brand-light">Apps</span>
              </span>
              {developer && <span className="developer-label">Developer</span>}
            </Link>
            <nav className="desktop-nav" aria-label="Main navigation">
              <NavLink to="/store">Discover</NavLink>
              <NavLink to="/developer">Developers</NavLink>
              <NavLink to="/docs">Docs</NavLink>
            </nav>
            <div className="header-account">
              {account ? (
                <Link
                  to="/settings"
                  className="account-link"
                  aria-label={`Account settings for ${account.id}`}
                >
                  <span className="avatar small-avatar">
                    {(account.display_name || account.id)
                      .slice(0, 1)
                      .toUpperCase()}
                  </span>
                  <span>{account.id}</span>
                </Link>
              ) : (
                <Button variant="secondary" size="sm" onClick={login}>
                  Sign in <ArrowUpRight size={15} />
                </Button>
              )}
              <Button
                className="menu-button"
                variant="ghost"
                aria-label={mobile ? "Close navigation" : "Open navigation"}
                aria-expanded={mobile}
                onClick={() => setMobile(!mobile)}
              >
                {mobile ? <X size={21} /> : <Menu size={21} />}
              </Button>
            </div>
          </div>
          {mobile && (
            <nav className="mobile-nav" aria-label="Mobile navigation">
              <Link to="/store">Discover apps</Link>
              <Link to="/developer">Your apps</Link>
              <Link to="/developer/invitations">Invitations</Link>
              <Link to="/docs">Docs</Link>
              <Link to="/settings">Settings</Link>
            </nav>
          )}
        </header>
        {developer && (
          <div className="developer-subnav">
            <div>
              <NavLink to="/developer" end>
                <Grid2X2 size={15} /> Your apps
              </NavLink>
              <NavLink to="/developer/invitations">
                <Mail size={15} /> Invitations
              </NavLink>
              <NavLink to="/settings">
                <SettingsIcon size={15} /> Settings
              </NavLink>
            </div>
          </div>
        )}
        <main
          id="main"
          className={`main-container ${developer ? "developer-container" : ""}`}
          tabIndex={-1}
        >
          <ErrorNotice error={navigationError} />
          {session.error && (
            <div className="session-notice">
              <ErrorNotice error={session.error} retry={session.reload} />
            </div>
          )}
          <Routes>
            <Route
              path="/"
              element={
                <Navigate
                  to={
                    window.location.hostname.startsWith("developer.")
                      ? "/developer"
                      : "/store"
                  }
                  replace
                />
              }
            />
            <Route path="/store" element={<Store />} />
            <Route path="/store/:appId" element={<AppDetail />} />
            <Route path="/developer" element={<Developer />} />
            <Route path="/developer/apps/:appId" element={<AppWorkspace />} />
            <Route path="/developer/invitations" element={<Invitations />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="/docs" element={<Docs />} />
            <Route
              path="*"
              element={
                <Empty
                  title="This page isn’t here"
                  description="Head back to the store to find your next app."
                  action={
                    <Link className="link-button" to="/store">
                      Discover apps
                    </Link>
                  }
                />
              }
            />
          </Routes>
        </main>
        <footer className="site-footer">
          <div className="row">
            <IconLogo size={19} />
            <span>For Carbons and Silicons.</span>
          </div>
          <nav aria-label="Footer">
            <Link to="/developer">Build an app</Link>
            <Link to="/docs">Documentation</Link>
            <Link to="/settings">Settings</Link>
          </nav>
          <span>Silicon Apps</span>
        </footer>
      </div>
    </SessionContext.Provider>
  );
}
function Docs() {
  return (
    <>
      <PageTitle
        title="A good place to start"
        description="Discover, install, and publish from your terminal."
      />
      <div className="docs-layout">
        <Section title="Find and install an app">
          <p>
            Search public apps without an account, then install the latest
            production release for your platform.
          </p>
          <Command value="apps search" />
          <Command value="apps install <app_id>" />
          <p>
            After installation, run the app’s help command. Silicon Apps checks
            for updates every minute.
          </p>
          <Command value="apps --help" />
        </Section>
        <Section title="Create and publish">
          <p>
            Sign in with Silicon Accounts, create an app, and open its
            publishing steps. You can do the same work in the developer
            platform.
          </p>
          <Command value="apps login" />
          <Command value="apps setup --help" />
          <Command value="apps pack --help" />
          <Command value="apps validate --help" />
          <Link to="/developer" className="link-button">
            Open developer platform <ArrowUpRight size={15} />
          </Link>
        </Section>
        <Section title="Every CLI speaks the same language">
          <p>
            Every package must include an apps.yaml manifest and support three
            commands. They are validated for each target before the package is
            accepted.
          </p>
          <Command value="<command> --help" />
          <Command value="<command> accounts --json" />
          <Command value="<command> login status --json" />
          <p>
            The accounts command returns the app_id. Login status returns
            authenticated and the signed-in Carbon or Silicon when present.
          </p>
        </Section>
        <Section title="Choose a release">
          <p>
            Development and production releases have independent versions. Quote
            the app reference so your shell treats it as one argument.
          </p>
          <Command value="apps install '<app_id>>dev'" />
          <Command value="apps install '<app_id>@1.2.3'" />
          <Command value="apps install '<app_id>>dev@1.2.3'" />
          <p>
            If you switch channels, the CLI asks before replacing your installed
            release.
          </p>
        </Section>
      </div>
    </>
  );
}
export default function App() {
  return (
    <BrowserRouter>
      <Shell />
    </BrowserRouter>
  );
}
