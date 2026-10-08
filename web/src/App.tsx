import { useEffect, useState, type MouseEvent } from "react";
import {
  createBrowserRouter,
  RouterProvider,
  Link,
  NavLink,
  Navigate,
  Route,
  Routes,
  useLocation,
  useNavigate,
  useBlocker,
} from "react-router-dom";
import { ArrowUpRight, Menu, X } from "lucide-react";
import { flushPendingSaves, hasPendingSaves, login, useResource } from "./api";
import { SessionContext } from "./context";
import type { Account } from "./types";
import { developerUrl, legacyDeveloperUrl, docsUrl } from "./portal";
import { AppDetail, AuthorProfile, Store } from "./Store";
import { ThemeSwitch } from "./components/arc/theme-switch/theme-switch";
import { useTheme } from "./use-theme";
import { Settings, telemetry } from "./Settings";
import { Button, Empty, ErrorNotice, IconLogo } from "./ui";
function DeveloperRedirect() {
  const location = useLocation();
  const destination = legacyDeveloperUrl(location.pathname, location.search);
  useEffect(() => {
    window.location.replace(destination);
  }, [destination]);
  return (
    <p>
      Opening the developer portal… <a href={destination}>Continue</a>
    </p>
  );
}
function DocsRedirect() {
  const location = useLocation();
  const destination = docsUrl(
    location.pathname,
    location.search,
    location.hash,
  );
  useEffect(() => {
    window.location.replace(destination);
  }, [destination]);
  return (
    <p>
      Opening the shared documentation… <a href={destination}>Continue</a>
    </p>
  );
}
function Shell() {
  const { theme, change } = useTheme();
  const location = useLocation();
  const navigate = useNavigate();
  const session = useResource<{
    authenticated: boolean;
    account: Account | null;
  }>("/session");
  const [mobile, setMobile] = useState(false);
  const [navigationError, setNavigationError] = useState<Error>();
  const blocker = useBlocker(() => hasPendingSaves());
  useEffect(() => {
    if (blocker.state !== "blocked") return;
    let active = true;
    void flushPendingSaves()
      .then(() => {
        if (!active) return;
        setNavigationError(undefined);
        blocker.proceed();
      })
      .catch((error) => {
        if (!active) return;
        setNavigationError(error as Error);
        blocker.reset();
      });
    return () => {
      active = false;
    };
  }, [blocker]);
  const account = session.data?.account || null;

  useEffect(() => {
    setMobile(false);
    window.scrollTo(0, 0);
    document.title = "Silicon Apps";
    void telemetry("page_view", { path: location.pathname });
  }, [location.pathname]);
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
            <div className="header-start">
              <Link
                to="/store"
                className="brand"
                aria-label="Silicon Apps home"
              >
                <span className="brand-mark">
                  <IconLogo size={18} />
                </span>
                <span>
                  Silicon <span className="brand-light">Apps</span>
                </span>
              </Link>
              <nav className="desktop-nav" aria-label="Main navigation">
                <NavLink to="/store">Discover</NavLink>
                <a href={developerUrl()}>Developers</a>
                <a href={docsUrl()}>Docs</a>
              </nav>
            </div>
            <div className="header-account">
              <ThemeSwitch
                theme={theme}
                variant="eclipse"
                iconOnly
                onThemeChange={(next, _variant, trigger) =>
                  change(next, trigger)
                }
              />
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
              <a href={developerUrl()}>Developer portal</a>
              <a href={docsUrl()}>Docs</a>
              <Link to="/settings">Settings</Link>
            </nav>
          )}
        </header>
        <main id="main" className="main-container" tabIndex={-1}>
          <ErrorNotice error={navigationError} />
          {session.error && (
            <div className="session-notice">
              <ErrorNotice error={session.error} retry={session.reload} />
            </div>
          )}
          <Routes>
            <Route path="/" element={<Navigate to="/store" replace />} />
            <Route path="/store" element={<Store />} />
            <Route
              path="/store/apps"
              element={<Navigate to="/store/silicon-apps" replace />}
            />
            <Route path="/store/:appId" element={<AppDetail />} />
            <Route path="/authors/:authorUuid" element={<AuthorProfile />} />
            <Route path="/developer/*" element={<DeveloperRedirect />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="/docs/*" element={<DocsRedirect />} />
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
            <a href={developerUrl()}>Build an app</a>
            <a href={docsUrl()}>Documentation</a>
            <Link to="/settings">Settings</Link>
          </nav>
          <span>Silicon Apps</span>
        </footer>
      </div>
    </SessionContext.Provider>
  );
}
const router = createBrowserRouter([{ path: "*", element: <Shell /> }]);
export default function App() {
  return <RouterProvider router={router} />;
}
