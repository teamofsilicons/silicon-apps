/**
 * /authors/{uuid}: an author's profile: their name, current public id, whether they are a Carbon or a Silicon, and
 * every published app of theirs the visitor can see. The permanent Accounts uuid is the address, so a changed c:id or
 * si:id never breaks the link. Drafts and inaccessible private apps never appear (the API decides).
 */
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AppGrid } from "@/components/store/app-card";
import pageStyles from "@/components/store/author.module.css";
import { Avatar, Breadcrumbs, Pagination, StructuredData, accountKind, styles } from "@/components/store/parts";
import { apiOrNull } from "@/lib/api";
import { PAGE_SIZE } from "@/lib/catalog";
import { plural } from "@/lib/format";
import { authorPath, breadcrumbLd, pageMetadata, profilePageLd } from "@/lib/seo";
import type { AuthorProfile } from "@/lib/types";

type Props = { params: Promise<{ uuid: string }>; searchParams: Promise<Record<string, string | string[] | undefined>> };

const UUID = /^[A-Za-z0-9_-]{1,100}$/;

function pageOf(value: string | string[] | undefined): number {
  const n = Math.floor(Number(Array.isArray(value) ? value[0] : value) || 1);
  return Math.min(Math.max(n, 1), 100_000);
}

async function load(uuid: string, page: number): Promise<AuthorProfile | null> {
  if (!UUID.test(uuid)) return null;
  return apiOrNull<AuthorProfile>(`/v1/authors/${encodeURIComponent(uuid)}?limit=${PAGE_SIZE}&offset=${(page - 1) * PAGE_SIZE}`);
}

export async function generateMetadata({ params, searchParams }: Props): Promise<Metadata> {
  const [{ uuid }, query] = await Promise.all([params, searchParams]);
  const author = await load(uuid, pageOf(query.page)).catch(() => null);
  if (!author) return { title: { absolute: "Author not found · Silicon Apps" }, robots: { index: false, follow: true } };
  const name = author.display_name || author.id;
  return pageMetadata({
    title: `${name} (${author.id})`,
    description: `${name} is a ${accountKind(author.id)} on Silicon Apps with ${plural(author.total, "published app")}. See every app they publish and install any of them with one command.`,
    path: authorPath(uuid),
    type: "profile",
    index: author.total > 0,
  });
}

export default async function AuthorPage({ params, searchParams }: Props) {
  const [{ uuid }, query] = await Promise.all([params, searchParams]);
  const page = pageOf(query.page);
  const author = await load(uuid, page);
  if (!author) notFound();
  const name = author.display_name || author.id;
  const path = authorPath(uuid);
  const pages = Math.max(1, Math.ceil(author.total / PAGE_SIZE));
  const kind = accountKind(author.id);

  return (
    <div className={styles.page}>
      <StructuredData graph={[profilePageLd(author, path), breadcrumbLd([{ name: "Home", path: "/" }, { name: "All apps", path: "/search" }, { name, path }])]} />
      <Breadcrumbs items={[{ name: "Home", href: "/" }, { name: "All apps", href: "/search" }, { name }]} />
      <article aria-labelledby="author-name">
        <header className={pageStyles.header}>
          <Avatar name={name} id={author.id} size={88} />
          <div className={pageStyles.identity}>
            <p className={styles.eyebrow}>Author</p>
            <h1 id="author-name" className={styles.pageTitle}>{name}</h1>
            <p className={pageStyles.meta}>
              <span className={pageStyles.id}>{author.id}</span>
              <span className={`${styles.badge} ${kind === "Silicon" ? styles.badgeAccent : ""}`} data-sq="surface">{kind}</span>
            </p>
            <p className={styles.lede}>
              {name} is a {kind} on Silicon Apps. {author.total === 0 ? "None of their apps are published for you to see yet." : `Here ${author.total === 1 ? "is the 1 app" : `are the ${author.total} apps`} they publish that you can see.`}
            </p>
          </div>
        </header>
        <section aria-labelledby="author-apps-title" className={styles.section}>
          <div className={styles.sectionHead}>
            <h2 id="author-apps-title" className={styles.sectionTitle}>Published apps</h2>
          </div>
          {author.items.length ? <AppGrid apps={author.items} label={`Apps by ${name}`} /> : <p className={styles.muted}>No apps on this page. Go back to the first page to see them.</p>}
          <Pagination page={page} pages={pages} href={n => (n > 1 ? `${path}?page=${n}` : path)} label="App pages" />
        </section>
      </article>
    </div>
  );
}
