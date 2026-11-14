"use client";
/**
 * The link Silicon UI renders inside the store's shell (NavList's rows): a plain <a>, as everywhere in the store, that
 * opens addresses on other sites in a new tab, as the old header and footer did. A client file only because a
 * component can't be handed from the server to SiliconProvider.
 */
import type { ReactNode } from "react";
import { SiliconProvider, type SiliconLinkProps } from "@teamofsilicons/silicon-ui/provider";

function StoreLink({ href, ...props }: SiliconLinkProps) {
  const external = /^https?:\/\//.test(href);
  return <a {...props} href={href} target={external ? "_blank" : props.target} rel={external ? "noopener noreferrer" : props.rel} />;
}

export function StoreLinks({ children }: { children: ReactNode }) {
  return <SiliconProvider link={StoreLink}>{children}</SiliconProvider>;
}
