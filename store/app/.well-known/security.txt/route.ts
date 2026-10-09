/** GET /.well-known/security.txt (RFC 9116): where to report a security problem, as on the developer site. */
import { CANONICAL_ORIGIN, LINKS, ORGANIZATION } from "@/lib/site";
import { publicResponse } from "@/lib/server/public-response";

/** Renew before this date (RFC 9116 asks for less than a year ahead). */
const EXPIRES = "2027-09-30T00:00:00.000Z";

export function GET(request: Request) {
  const body = [
    `Contact: mailto:${ORGANIZATION.email}`,
    `Contact: ${LINKS.appsGithub}/security/advisories/new`,
    `Expires: ${EXPIRES}`,
    "Preferred-Languages: en",
    `Canonical: ${CANONICAL_ORIGIN}/.well-known/security.txt`,
    `Policy: ${LINKS.developers}/docs/accounts/learn/security`,
    "",
  ].join("\n");
  return publicResponse(request, body, { type: "text/plain; charset=utf-8", maxAge: 86400 });
}
