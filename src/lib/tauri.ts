import { invoke } from "@tauri-apps/api/core";

export function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(command, args);
}
