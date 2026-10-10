// Cifrar los datos de AdminOps (1.2.9): clientes, contactos, notas, casos, agenda…
import { invoke } from "./core";

export interface DataCryptStatus {
  /** Hay bloqueo con PIN o contraseña (hace falta para cifrar). */
  available: boolean;
  enabled: boolean;
  /** La clave de los datos está abierta en esta sesión. */
  unlocked: boolean;
  files: number;
  sealed: number;
}

export const dataCryptApi = {
  status: () => invoke<DataCryptStatus>("data_crypt_status"),
  /** Cifra los datos. Devuelve la clave de rescate: se enseña una sola vez. */
  enable: (secret: string) => invoke<string>("data_crypt_enable", { secret }),
  /** Descifra todo y quita las claves. Devuelve cuántos archivos se descifraron. */
  disable: (secret: string) => invoke<number>("data_crypt_disable", { secret }),
  unlockRecovery: (key: string) => invoke<boolean>("data_crypt_unlock_recovery", { key }),
};
