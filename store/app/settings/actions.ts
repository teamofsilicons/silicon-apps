"use server";
/** Report a problem: the same report as `silicon-apps report`, delivered to the maintainers by the API. */
import { redirect } from "next/navigation";
import { api, ApiError } from "@/lib/api";

export async function sendReport(formData: FormData): Promise<void> {
  const message = String(formData.get("message") ?? "").trim();
  const pr = String(formData.get("pr") ?? "").trim();
  const key = String(formData.get("idempotency_key") ?? "");
  if (message.length < 3) redirect("/settings?report=empty#report");
  if (message.length > 10_000) redirect("/settings?report=long#report");
  if (pr && !/^https:\/\/\S{3,500}$/.test(pr)) redirect("/settings?report=pr#report");
  let outcome = "sent";
  try {
    await api("/v1/reports", { method: "POST", body: pr ? { message, pr } : { message }, idempotencyKey: /^[\x21-\x7e]{8,200}$/.test(key) ? key : undefined });
  } catch (error) {
    outcome = error instanceof ApiError && error.status === 503 ? "unavailable" : "failed";
  }
  redirect(`/settings?report=${outcome}#report`);
}
