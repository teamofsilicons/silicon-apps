"use client";

import { useState } from "react";
import styles from "./store.module.css";

/** Keep the initial underneath if an Accounts photo is no longer available. */
export function AvatarPhoto({ src, size }: { src: string; size: number }) {
  const [failed, setFailed] = useState<string>();
  if (failed === src) return null;
  // Account images are already hosted by Accounts or the account's image provider.
  // eslint-disable-next-line @next/next/no-img-element
  return <img className={styles.avatarPhoto} src={src} alt="" width={size} height={size} decoding="async" onError={() => setFailed(src)} />;
}
