import { createContext, useContext, type ReactNode } from "react";
import type { RotameterApiClient } from "./api";
import type {
  AuthMeResponse,
  CapabilitiesResponse,
  DashboardQueryState,
  QueryMeta,
} from "./types";

export interface UpdateQueryOptions {
  mode?: "push" | "replace";
}

export interface DashboardContextValue {
  api: RotameterApiClient;
  capabilities: CapabilitiesResponse;
  auth: AuthMeResponse;
  query: DashboardQueryState;
  updateQuery: (patch: Partial<DashboardQueryState>, options?: UpdateQueryOptions) => void;
  telemetryMeta: QueryMeta | null;
  setTelemetryMeta: (meta: QueryMeta | null) => void;
}

const DashboardContext = createContext<DashboardContextValue | null>(null);

export interface DashboardProviderProps {
  value: DashboardContextValue;
  children: ReactNode;
}

export function DashboardProvider({ value, children }: DashboardProviderProps) {
  return <DashboardContext.Provider value={value}>{children}</DashboardContext.Provider>;
}

function useRequiredContext(): DashboardContextValue {
  const ctx = useContext(DashboardContext);
  if (!ctx) {
    throw new Error("useDashboard* hooks must be used inside <DashboardProvider>");
  }
  return ctx;
}

export function useCapabilities(): CapabilitiesResponse {
  return useRequiredContext().capabilities;
}

export function useAuth(): AuthMeResponse {
  return useRequiredContext().auth;
}

export function useApiClient(): RotameterApiClient {
  return useRequiredContext().api;
}

export function useDashboardQuery(): DashboardQueryState {
  return useRequiredContext().query;
}

export function useUpdateQuery(): (
  patch: Partial<DashboardQueryState>,
  options?: UpdateQueryOptions,
) => void {
  return useRequiredContext().updateQuery;
}

export function useTelemetryMeta(): QueryMeta | null {
  return useRequiredContext().telemetryMeta;
}

export function useSetTelemetryMeta(): (meta: QueryMeta | null) => void {
  return useRequiredContext().setTelemetryMeta;
}
