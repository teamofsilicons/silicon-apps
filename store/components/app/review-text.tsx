"use client";
/**
 * The review form's words (Anil, 2026-11-18): Silicon UI's Textarea, at most 600 characters, with how many are left
 * under it (a line of its own: Textarea's description animates its words, which reads twice to a test or a copy). The count is server-rendered from what the field starts with and kept current as you type where script
 * runs; without script the field still stops at 600 (and the server action checks again).
 */
import { useState } from "react";
import { Textarea } from "@teamofsilicons/silicon-ui/textarea";
import styles from "./reviews.module.css";

const MAX = 600;
const left = (text: string) => `${MAX - [...text].length} characters left`;

export function ReviewText({ initial }: { initial: string }) {
  const [text, setText] = useState(initial);
  return (
    <div className={styles.words}>
      <Textarea
        label="Your review (optional)"
        name="text"
        rows={4}
        maxLength={MAX}
        defaultValue={initial}
        onChange={event => setText(event.currentTarget.value)}
        placeholder="What does it do well? What could be better?"
        aria-describedby="review-left"
      />
      <p id="review-left" className={styles.left}>{left(text)}</p>
    </div>
  );
}
