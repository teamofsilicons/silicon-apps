/**
 * /authors/{uuid}: a developer's page (Anil, 2026-11-18), where the app page's developer card and More by's "See all"
 * lead. Built as the app page is, so the two read as one store:
 * - a header like the app's: their picture (72px, 56 on a phone) on the left, beside it their name (display) and their
 *   id with what they are (a Carbon is a person, a Silicon an agent); then a hairline and the labelled facts across the
 *   page: Apps (how many they have published), Installs (all their apps' together), Systems (every system any of their
 *   apps runs on, as chips), Works with (everyone they publish with, most shared apps first, each a chip linked to
 *   their page) and Publishing since (when they first became an author of one of these apps);
 * - then their apps in the store's grid (components/gallery/gallery.tsx), most installed first, the same rows as All
 *   Apps and a category.
 * GET /v1/authors/{uuid}?limit=100 (published apps the visitor may see; drafts never); unknown authors and authors with
 * nothing visible answer 404 (proxy.ts asks the API first, and this page asks again). Server-rendered.
 */
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Avatar } from "@teamofsilicons/silicon-ui/avatar";
import { Badge } from "@teamofsilicons/silicon-ui/badge";
import { appsCount, nameOf } from "@/components/app/developer";
import { Gallery } from "@/components/gallery/gallery";
import { OsMark } from "@/components/home/os-icons";
import { Page } from "@/components/parts/page";
import { apiOrNull } from "@/lib/api";
import { formatCount, formatDate, formatNumber, operatingSystems, safeImage } from "@/lib/format";
import { authorPath, JsonLd, pageMetadata, profilePageLd } from "@/lib/seo";
import type { Author, AuthorProfile } from "@/lib/types";
import styles from "./author.module.css";

const LIMIT = 100;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

type Props = { params: Promise<{ uuid: string }> };

async function load(uuid: string): Promise<AuthorProfile | null> {
  if (!UUID.test(uuid)) return null;
  return apiOrNull<AuthorProfile>(`/v1/authors/${encodeURIComponent(uuid)}?limit=${LIMIT}`);
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { uuid } = await params;
  const author = await load(uuid).catch(() => null);
  if (!author) return { title: { absolute: "Developer not found · Silicon Apps" }, robots: { index: false, follow: true } };
  const name = author.display_name || author.id;
  return pageMetadata({
    title: name,
    description: `${name} (${author.id}) on Silicon Apps: ${appsCount(author.total)}, each installed with one command.`,
    path: authorPath(author.uuid),
  });
}

/** Everyone they publish with, most shared apps first (then by name), each once. */
function collaborators(author: AuthorProfile): Author[] {
  const shared = new Map<string, { person: Author; count: number }>();
  for (const app of author.items) {
    for (const person of app.authors) {
      if (person.uuid === author.uuid) continue;
      const seen = shared.get(person.uuid);
      shared.set(person.uuid, { person, count: (seen?.count ?? 0) + 1 });
    }
  }
  return [...shared.values()].sort((a, b) => b.count - a.count || nameOf(a.person).localeCompare(nameOf(b.person))).map(entry => entry.person);
}

export default async function AuthorPage({ params }: Props) {
  const { uuid } = await params;
  const author = await load(uuid);
  if (!author) notFound();
  const name = author.display_name || author.id;
  const apps = [...author.items].sort((a, b) => (b.installs ?? 0) - (a.installs ?? 0) || a.name.localeCompare(b.name));
  const installs = apps.reduce((sum, app) => sum + (app.installs ?? 0), 0);
  const systems = operatingSystems(apps.flatMap(app => app.targets));
  const team = collaborators(author);
  const first = apps.flatMap(app => app.authors.filter(person => person.uuid === author.uuid).map(person => person.joined_at)).sort()[0];
  const silicon = author.id.startsWith("si:");

  return (
    <article aria-labelledby="author-name">
      <Page wide>
        <JsonLd graph={[profilePageLd(author, authorPath(author.uuid))]} />
        <header className={styles.header}>
          <div className={styles.top}>
            <span className={styles.picture}><Avatar name={name} src={safeImage(author.pfp_url)} size="xl" aria-hidden="true" /></span>
            <div className={styles.words}>
              <h1 id="author-name" className={styles.name}>{name}</h1>
              <p className={styles.line}>
                <span className={styles.id}>{author.id}</span>
                <Badge size="sm" tone="neutral">{silicon ? "Silicon" : "Carbon"}</Badge>
              </p>
            </div>
          </div>
          <dl className={styles.facts}>
            <div><dt>Apps</dt><dd>{formatNumber(author.total)}</dd></div>
            <div><dt>Installs</dt><dd title={formatNumber(installs)}>{formatCount(installs)}</dd></div>
            {systems.length ? (
              <div><dt>Systems</dt><dd>
                <ul className={styles.chips}>
                  {systems.map(system => <li key={system} className={styles.chip}><OsMark system={system} className={styles.mark} />{system}</li>)}
                </ul>
              </dd></div>
            ) : null}
            {team.length ? (
              <div><dt>Works with</dt><dd>
                <ul className={styles.chips}>
                  {team.map(person => (
                    <li key={person.uuid}>
                      <a href={authorPath(person.uuid)} className={styles.person} title={person.id}>
                        <Avatar name={nameOf(person)} src={safeImage(person.pfp_url)} size="sm" aria-hidden="true" />
                        {nameOf(person)}
                      </a>
                    </li>
                  ))}
                </ul>
              </dd></div>
            ) : null}
            {first ? <div><dt>Publishing since</dt><dd><time dateTime={first}>{formatDate(first)}</time></dd></div> : null}
          </dl>
        </header>

        <section className={styles.apps} aria-labelledby="author-apps">
          <div className={styles.appsHead}>
            <h2 id="author-apps" className={styles.title}>Apps</h2>
            <p className={styles.count}>{author.total > apps.length ? `The ${apps.length} most installed of ${author.total}` : appsCount(author.total)}</p>
          </div>
          <Gallery apps={apps} label={`Apps by ${name}`} />
        </section>
      </Page>
    </article>
  );
}
