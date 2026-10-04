// Cuentas de Windows y credenciales.
import { invoke } from "./core";

// ---------- Cuentas (Microsoft, trabajo o escuela, Entra ID, Office, credenciales) ----------

export interface AccountsStatus {
  sessionUser: string;
  sessionKind: "local" | "microsoft" | "azuread" | "domain";
  device: {
    azureAdJoined: boolean;
    domainJoined: boolean;
    workplaceJoined: boolean;
    enterpriseJoined: boolean;
    tenantName: string;
    domainName: string;
    deviceId: string;
  };
  workAccounts: { id: string; email: string; tenant: string; scope: "device" | "user" }[];
  microsoftAccounts: string[];
  officeAccounts: { id: string; email: string; name: string; kind: string }[];
  credentials: { target: string; user: string; kind: string }[];
  hasLocalAdmin: boolean;
  otherUser: boolean;
}

export const accountsApi = {
  status: () => invoke<AccountsStatus>("accounts_status"),
  leaveAzureAd: () => invoke<string>("leave_azure_ad"),
  removeWorkAccount: (id: string) => invoke<string>("remove_work_account", { id }),
  officeSignOut: (id: string) => invoke<string>("office_sign_out", { id }),
  deleteCredential: (target: string) => invoke<void>("delete_credential", { target }),
  signOut: () => invoke<void>("sign_out_windows"),
};
