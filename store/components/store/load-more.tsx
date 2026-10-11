"use client";
/**
 * "Show more apps" at the end of the list: the next batch joins the list in place, without moving the page. It is a
 * real link to the address with one more batch (?page=N), so without script it still works, as a normal page load.
 */
import { useRouter } from "next/navigation";
import { useTransition, type MouseEvent } from "react";
import { ChevronDown } from "lucide-react";
import buttonStyles from "@/components/silicon-ui/button/button.module.css";
import linkStyles from "@/components/foundation/button-link.module.css";

export function LoadMore({ href, remaining }: { href: string; remaining: number }) {
  const router = useRouter();
  const [pending, startTransition] = useTransition();
  const onClick = (event: MouseEvent<HTMLAnchorElement>) => {
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button !== 0) return;
    event.preventDefault();
    startTransition(() => router.push(href, { scroll: false }));
  };
  return (
    <a href={href} onClick={onClick} rel="next" aria-busy={pending || undefined} data-variant="secondary" className={`${buttonStyles.button} ${buttonStyles.secondary} ${buttonStyles.md} ${linkStyles.link}`}>
      <span className={linkStyles.content}>
        {pending ? "Loading more apps" : `Show more apps`}
        <span className="sr-only"> ({remaining} more)</span>
        <ChevronDown size={15} strokeWidth={1.75} aria-hidden="true" />
      </span>
    </a>
  );
}
