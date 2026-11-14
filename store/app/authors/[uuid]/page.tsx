/**
 * /authors/{uuid}: an author's page. Empty for now (Anil, 2026-11-14), to be designed again inside the account site's
 * layout. Unknown authors still answer 404 before this renders (proxy.ts asks the API).
 */
import { Page } from "@/components/parts/page";

export default function AuthorPage() {
  return <Page />;
}
