/**
 * /playground/sitemap: the store's whole flow (Anil, 2026-11-19: "the complete flow of the apps website"), from the
 * links in the pages' code (flow.ts). Under `next dev` only (a 404 in production builds, never indexed).
 * - The sidebar, on every page, as a band across the top.
 * - A diagram: the pages (white), the steps that aren't pages (dashed: signing in and out) and the other sites
 *   (grey), with an arrow for every link from one to another. Pick one (?focus=, a plain link, so no script) to light
 *   up its arrows in and out and dim the rest. The diagram scrolls sideways on a phone; the page doesn't.
 * - Under it, every page: what it is, where it goes and what leads to it (just the picked one when one is picked),
 *   then the old addresses that redirect, and what the site serves for machines.
 */
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { Page } from "@/components/parts/page";
import { EDGES, MACHINES, NODE_H, NODE_W, NODES, REDIRECTS, SIDEBAR, VIEW_H, VIEW_W, type FlowEdge, type FlowNode } from "./flow";
import styles from "./sitemap.module.css";

export const metadata: Metadata = { title: "Sitemap flow", robots: { index: false, follow: false } };

const KINDS = [
  { kind: "page", label: "A page on this site" },
  { kind: "step", label: "A step, not a page" },
  { kind: "away", label: "Another site" },
] as const;

const byId = new Map(NODES.map(node => [node.id, node]));

/** An arrow from one box to another: side to side when they're in different columns, else bottom to top (or up). */
function arrowPath(from: FlowNode, to: FlowNode): string {
  const [fx, fy, tx, ty] = (() => {
    if (to.x > from.x) return [from.x + NODE_W, from.y + NODE_H / 2, to.x, to.y + NODE_H / 2];
    if (to.x < from.x) return [from.x, from.y + NODE_H / 2, to.x + NODE_W, to.y + NODE_H / 2];
    return to.y > from.y ? [from.x + NODE_W / 2, from.y + NODE_H, to.x + NODE_W / 2, to.y] : [from.x + NODE_W / 2, from.y, to.x + NODE_W / 2, to.y + NODE_H];
  })();
  if (fx === tx) return `M${fx},${fy} L${tx},${ty}`;
  const bend = Math.max(40, Math.abs(tx - fx) / 2) * Math.sign(tx - fx);
  return `M${fx},${fy} C${fx + bend},${fy} ${tx - bend},${ty} ${tx},${ty}`;
}

function Diagram({ focus }: { focus?: string }) {
  const between = EDGES.filter(edge => edge.from !== edge.to);
  const lit = (edge: FlowEdge) => !focus || edge.from === focus || edge.to === focus;
  const near = new Set(focus ? EDGES.filter(lit).flatMap(edge => [edge.from, edge.to]) : NODES.map(node => node.id));
  return (
    <div className={styles.scroller}>
      <svg viewBox={`-4 -4 ${VIEW_W + 8} ${VIEW_H + 8}`} className={styles.diagram} role="img" aria-labelledby="flow-title">
        <defs>
          <marker id="arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L8,4 L0,8 z" className={styles.head} /></marker>
          <marker id="arrow-lit" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L8,4 L0,8 z" className={styles.headLit} /></marker>
        </defs>
        {/* Dimmed arrows first, so the lit ones draw over them. */}
        {[...between.filter(edge => !lit(edge)), ...between.filter(lit)].map(edge => {
          const on = Boolean(focus) && lit(edge);
          return (
            <path key={`${edge.from}-${edge.to}`} d={arrowPath(byId.get(edge.from)!, byId.get(edge.to)!)} className={styles.edge} data-on={on ? "" : undefined} data-off={lit(edge) ? undefined : ""} markerEnd={on ? "url(#arrow-lit)" : "url(#arrow)"}>
              <title>{`${byId.get(edge.from)!.label} → ${byId.get(edge.to)!.label}: ${edge.label}`}</title>
            </path>
          );
        })}
        {NODES.map(node => (
          <a key={node.id} href={focus === node.id ? "?" : `?focus=${node.id}#flow`} className={styles.node} data-kind={node.kind} data-focus={focus === node.id ? "" : undefined} data-off={near.has(node.id) ? undefined : ""}>
            <rect x={node.x} y={node.y} width={NODE_W} height={NODE_H} rx="10" />
            <text x={node.x + 14} y={node.y + 25} className={styles.nodeLabel}>{node.label}</text>
            <text x={node.x + 14} y={node.y + 44} className={styles.nodePath}>{node.path}</text>
            {EDGES.some(edge => edge.from === node.id && edge.to === node.id) ? <text x={node.x + NODE_W - 14} y={node.y + 25} textAnchor="end" className={styles.nodePath}>↻</text> : null}
          </a>
        ))}
      </svg>
    </div>
  );
}

function NodeCard({ node }: { node: FlowNode }) {
  const out = EDGES.filter(edge => edge.from === node.id);
  const into = EDGES.filter(edge => edge.to === node.id && edge.from !== node.id);
  const fromSidebar = SIDEBAR.filter(item => item.to === node.id);
  return (
    <article className={styles.card} data-kind={node.kind} aria-labelledby={`node-${node.id}`}>
      <div className={styles.cardHead}>
        <h3 id={`node-${node.id}`} className={styles.cardTitle}><a href={`?focus=${node.id}#flow`}>{node.label}</a></h3>
        {node.open ? <a href={node.open} className={styles.open}>Open {node.open.length > 28 ? "it" : node.open}</a> : null}
      </div>
      <p className={styles.path}>{node.path}</p>
      <p className={styles.note}>{node.note}</p>
      {out.length ? (
        <div className={styles.links}>
          <h4 className={styles.linksTitle}>Goes to</h4>
          <ul>{out.map(edge => <li key={edge.to}><strong>{byId.get(edge.to)!.label}</strong> <span>{edge.label}</span></li>)}</ul>
        </div>
      ) : null}
      {into.length || fromSidebar.length ? (
        <div className={styles.links}>
          <h4 className={styles.linksTitle}>Reached from</h4>
          <ul>
            {fromSidebar.map(item => <li key={item.label}><strong>The sidebar</strong> <span>{item.label}</span></li>)}
            {into.map(edge => <li key={edge.from}><strong>{byId.get(edge.from)!.label}</strong> <span>{edge.label}</span></li>)}
          </ul>
        </div>
      ) : null}
    </article>
  );
}

type Props = { searchParams: Promise<Record<string, string | string[] | undefined>> };

export default async function SitemapFlow({ searchParams }: Props) {
  if (process.env.NODE_ENV === "production") notFound();
  const params = await searchParams;
  const focus = typeof params.focus === "string" && byId.has(params.focus) ? params.focus : undefined;
  const picked = focus ? byId.get(focus)! : undefined;

  return (
    <Page wide>
      <div className={styles.lab}>
        <header className={styles.head}>
          <h1 className={styles.title}>Sitemap flow</h1>
          <p className={styles.lede}>Every page of the store, the steps between them and where they lead off-site: {NODES.filter(node => node.kind === "page").length} pages, {EDGES.length} links. Pick a box to follow its links.</p>
        </header>

        <section className={styles.sidebar} aria-labelledby="sidebar-title">
          <h2 id="sidebar-title" className={styles.sidebarTitle}>On every page, the sidebar</h2>
          <ul>{SIDEBAR.map(item => <li key={item.label}><a href={`?focus=${item.to}#flow`}>{item.label}</a></li>)}</ul>
        </section>

        <section id="flow" className={styles.flow} aria-labelledby="flow-title">
          <div className={styles.flowHead}>
            <h2 id="flow-title" className={styles.h2}>{picked ? `Links in and out of ${picked.label}` : "How the pages link"}</h2>
            {picked ? <a href="?#flow" className={styles.open}>Show all</a> : null}
          </div>
          <ul className={styles.legend} aria-label="Kinds">
            {KINDS.map(item => <li key={item.kind}><span className={styles.swatch} data-kind={item.kind} aria-hidden="true" />{item.label}</li>)}
            <li><span className={styles.loop} aria-hidden="true">↻</span>Also links to itself</li>
          </ul>
          <Diagram focus={focus} />
        </section>

        <section className={styles.section} aria-labelledby="pages-title">
          <h2 id="pages-title" className={styles.h2}>{picked ? picked.label : "Every page and step"}</h2>
          <div className={styles.cards}>{(picked ? [picked] : NODES).map(node => <NodeCard key={node.id} node={node} />)}</div>
        </section>

        <div className={styles.pair}>
          <section className={styles.section} aria-labelledby="redirects-title">
            <h2 id="redirects-title" className={styles.h2}>Old addresses</h2>
            <p className={styles.note}>Each a permanent redirect (proxy.ts), so old links still land.</p>
            <dl className={styles.table}>{REDIRECTS.map(item => <div key={item.from}><dt>{item.from}</dt><dd>{item.to}</dd></div>)}</dl>
          </section>
          <section className={styles.section} aria-labelledby="machines-title">
            <h2 id="machines-title" className={styles.h2}>For machines</h2>
            <p className={styles.note}>Served beside the pages, for search engines, agents and browsers.</p>
            <dl className={styles.table}>{MACHINES.map(item => <div key={item.path}><dt>{item.path}</dt><dd>{item.note}</dd></div>)}</dl>
          </section>
        </div>
      </div>
    </Page>
  );
}
