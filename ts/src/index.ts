import { F8Error, toF8Error } from "./errors.js";
import { CompiledModel } from "./model.js";
import type { ProjectManifestInput, SheetSpecInput, TableSpecInput } from "./types.js";
import initWasm, {
  compile as wasmCompile,
  defaultTheme as wasmDefaultTheme,
  initSync as initWasmSync,
  version as wasmVersion,
  type InitInput,
  type InitOutput,
} from "../wasm/f8_wasm.js";

export * from "./errors.js";
export * from "./types.js";
export { CompiledModel } from "./model.js";

let initialized = false;

function getNodeFs(): typeof import("node:fs") | undefined {
  if (typeof process !== "undefined" && typeof process.getBuiltinModule === "function") {
    return process.getBuiltinModule("node:fs");
  }
  return undefined;
}

function findWasmPath(fs: typeof import("node:fs")): URL {
  const primary = new URL("../wasm/f8_wasm_bg.wasm", import.meta.url);
  const candidates = [
    primary,
    new URL("./wasm/f8_wasm_bg.wasm", import.meta.url),
    new URL("./f8_wasm_bg.wasm", import.meta.url),
  ];
  for (const url of candidates) {
    if (fs.existsSync(url)) {
      return url;
    }
  }
  return primary;
}

function isNodeLikeEnvironment(): boolean {
  return (
    typeof process !== "undefined" &&
    process !== null &&
    typeof process.versions === "object" &&
    process.versions !== null &&
    (Boolean(process.versions.node) ||
      Boolean(process.versions.bun) ||
      Boolean((process.versions as Record<string, unknown>).deno))
  );
}

let autoInitError: unknown = null;

// Module-load auto-initialization only runs in server runtimes (Node.js, Bun, Deno)
if (isNodeLikeEnvironment()) {
  try {
    const fs = getNodeFs();
    if (!fs) {
      throw new Error("node:fs builtin module unavailable");
    }
    const wasmPath = findWasmPath(fs);
    const bytes = fs.readFileSync(wasmPath);
    initWasmSync({ module: bytes });
    initialized = true;
  } catch (err: unknown) {
    // If auto-init cannot locate or load the wasm binary at import time,
    // record the error; explicit ensureInitSync() will report it with cause.
    autoInitError = err;
  }
}

export function ensureInitSync(): void {
  if (initialized) {
    return;
  }
  try {
    const fs = getNodeFs();
    if (!fs) {
      throw new Error(
        "Synchronous WASM initialization requires Node.js, Bun, or Deno; call await init() in browser environments",
      );
    }
    const wasmPath = findWasmPath(fs);
    const bytes = fs.readFileSync(wasmPath);
    initWasmSync({ module: bytes });
    initialized = true;
    VERSION = wasmVersion();
  } catch (err: unknown) {
    const causeErr = autoInitError ?? err;
    const msg = causeErr instanceof Error ? causeErr.message : String(causeErr);
    throw new F8Error(`Failed to synchronously initialize WASM core module: ${msg}`, {
      cause: causeErr,
    });
  }
}

export async function init(moduleOrPath?: InitInput | Promise<InitInput>): Promise<InitOutput> {
  const options = moduleOrPath !== undefined ? { module_or_path: moduleOrPath } : undefined;
  const res = await initWasm(options);
  initialized = true;
  VERSION = wasmVersion();
  return res;
}

export let VERSION: string = initialized ? wasmVersion() : "";

export function defaultTheme(): Record<string, unknown> {
  ensureInitSync();
  try {
    return wasmDefaultTheme() as Record<string, unknown>;
  } catch (err: unknown) {
    throw toF8Error(err);
  }
}

function normalizeManifest(manifest: ProjectManifestInput): Record<string, unknown> {
  const sheets = manifest.sheets.map((sheet: SheetSpecInput) => {
    const normSheet: Record<string, unknown> = {
      name: sheet.name,
      tables: sheet.tables,
    };
    if (sheet.tabColor !== undefined) {
      normSheet.tab_color = sheet.tabColor;
    }
    if (sheet.freezePanes !== undefined) {
      normSheet.freeze_panes = sheet.freezePanes;
    }
    if (sheet.showGridlines !== undefined) {
      normSheet.show_gridlines = sheet.showGridlines;
    }
    return normSheet;
  });

  const res: Record<string, unknown> = {
    model_name: manifest.modelName,
    sheets,
  };
  if (manifest.theme !== undefined) {
    res.theme = manifest.theme;
  }
  return res;
}

function normalizeTables(tables: readonly TableSpecInput[]): readonly Record<string, unknown>[] {
  return tables.map((t) => {
    const normTable: Record<string, unknown> = {
      id: t.id,
      title: t.title,
    };
    if (t.subtitle !== undefined) {
      normTable.subtitle = t.subtitle;
    }
    if (t.currency !== undefined) {
      normTable.currency = t.currency.toLowerCase();
    }
    if (t.collapsible !== undefined) {
      normTable.collapsible = t.collapsible;
    }
    if (t.placement !== undefined) {
      const p = { ...t.placement } as Record<string, unknown>;
      if (p.kind === "rightOf") {
        p.kind = "right_of";
      }
      normTable.placement = p;
    }
    normTable.columns = t.columns.map((c) => {
      if (c.kind === "data" && c.currency !== undefined) {
        return {
          ...c,
          currency: c.currency.toLowerCase(),
        };
      }
      return c;
    });
    normTable.rows = t.rows.map((r) => {
      if (r.kind === "sectionHeader") {
        return {
          ...r,
          kind: "section_header",
        };
      }
      if (r.kind === "data") {
        let normRow: Record<string, unknown> | null = null;
        if (r.currency !== undefined) {
          normRow = {
            ...r,
            currency: r.currency.toLowerCase(),
          };
        }
        if (r.cells !== undefined) {
          const normCells: Record<string, unknown> = {};
          let cellsModified = false;
          for (const [colId, cell] of Object.entries(r.cells)) {
            if (
              typeof cell === "object" &&
              cell !== null &&
              "currency" in cell &&
              cell.currency !== undefined
            ) {
              normCells[colId] = {
                ...cell,
                currency: cell.currency.toLowerCase(),
              };
              cellsModified = true;
            } else {
              normCells[colId] = cell;
            }
          }
          if (cellsModified) {
            normRow = {
              ...(normRow ?? r),
              cells: normCells,
            };
          }
        }
        if (normRow !== null) {
          return normRow;
        }
      }
      return r;
    });
    return normTable;
  });
}

export function compile(
  manifest: ProjectManifestInput,
  tables: readonly TableSpecInput[],
): CompiledModel {
  ensureInitSync();
  try {
    const normalizedManifest = normalizeManifest(manifest);
    const normalizedTables = normalizeTables(tables);
    const model = wasmCompile(normalizedManifest, normalizedTables);
    return new CompiledModel(model);
  } catch (err: unknown) {
    throw toF8Error(err);
  }
}
