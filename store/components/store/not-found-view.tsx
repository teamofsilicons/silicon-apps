/**
 * What a visitor sees for an address the store cannot show, server-rendered: an unknown page, an app they cannot see
 * (unknown, not published yet, or private and not shared with them; the API never says which), or an unknown author.
 * The original address comes from proxy.ts (x-store-path), which also sends missing apps and authors here so the page
 * renders in full on the server with a 404 status.
 */
import { headers } from "next/headers";
import { Lock, Search, UserRound } from "lucide-react";
import { Action } from "@/components/site/action";
import { getAccount, signInHref } from "@/lib/session";
import { EmptyState, styles } from "./parts";
import { SearchBox } from "./search-form";

const ICON = { size: 20, strokeWidth: 1.75 } as const;

export async function NotFoundView() {
  const [incoming, account] = await Promise.all([headers(), getAccount()]);
  const raw = incoming.get("x-store-path") || "/";
  const path = raw.startsWith("/") && !raw.startsWith("//") ? raw : "/";
  const pathname = path.split("?")[0];

  if (pathname.startsWith("/apps/")) {
    return (
      <div className={`${styles.page} ${styles.narrow}`}>
        <EmptyState
          icon={<Lock {...ICON} />}
          title="We could not find that app"
          headingLevel={1}
          actions={account ? <Action href="/search" variant="secondary">See all apps</Action> : <Action href={signInHref(path)} rel="nofollow">Sign in to see private apps</Action>}
        >
          <p>
            {account
              ? "It does not exist, it is not published yet, or it is private and not shared with your account."
              : "It does not exist, it is not published yet, or it is private. If it was shared with you, sign in to see it."}
          </p>
          <SearchBox id="not-found-q" />
        </EmptyState>
      </div>
    );
  }

  if (pathname.startsWith("/authors/")) {
    return (
      <div className={`${styles.page} ${styles.narrow}`}>
        <EmptyState icon={<UserRound {...ICON} />} title="We could not find that author" headingLevel={1} actions={<Action href="/search" variant="secondary">See all apps</Action>}>
          <p>Authors are listed by their permanent Silicon Accounts id. Find one of their apps instead:</p>
          <SearchBox id="not-found-q" />
        </EmptyState>
      </div>
    );
  }

  return (
    <div className={`${styles.page} ${styles.narrow}`}>
      <EmptyState icon={<Search {...ICON} />} title="This page is not here" headingLevel={1} actions={<Action href="/" variant="secondary">Back to the store</Action>}>
        <p>The address may have changed. Search for the app instead:</p>
        <SearchBox id="not-found-q" />
      </EmptyState>
    </div>
  );
}
