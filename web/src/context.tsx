import { createContext, useContext } from "react";
import type { Account } from "./types";
export const SessionContext = createContext<{
  account: Account | null;
  loading: boolean;
  refresh: () => void;
}>({ account: null, loading: true, refresh: () => {} });
export const useSession = () => useContext(SessionContext);
