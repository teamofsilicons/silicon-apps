/**
 * Plain text with `code` in backticks and https:// links, as the developer site's home page formats its answers
 * (developer/components/home/home-page.tsx).
 */
import { Fragment } from "react";

export function Rich({ text }: { text: string }) {
  const parts = text.split(/(`[^`]+`|https:\/\/[^\s),]+)/g);
  return (
    <>
      {parts.map((part, index) => {
        if (part.startsWith("`") && part.endsWith("`")) return <code key={index} data-sq-native="">{part.slice(1, -1)}</code>;
        if (part.startsWith("https://")) {
          const url = part.replace(/\.$/, "");
          return (
            <Fragment key={index}>
              <a href={url} rel="noopener">{url.replace(/^https:\/\//, "")}</a>
              {part.endsWith(".") ? "." : ""}
            </Fragment>
          );
        }
        return <Fragment key={index}>{part}</Fragment>;
      })}
    </>
  );
}
