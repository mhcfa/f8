import { normalizeDetailedCellObject } from "./builder.js"
import { F8Error, toF8Error } from "./errors.js"
import { CompiledModel } from "./model.js"
import type {
  ProjectManifestInput,
  SheetSpecInput,
  TableSpecInput,
} from "./types.js"
import initWasm, {
  compile as wasmCompile,
  defaultTheme as wasmDefaultTheme,
  initSync as initWasmSync,
  version as wasmVersion,
  type InitInput,
  type InitOutput,
} from "../wasm/f8_wasm.js"

export { TableAuditor, unboxCellValue } from "./audit.js"
export { Cell, Model, Sheet, Table } from "./builder.js"
export * from "./errors.js"
export * from "./types.js"
export { CompiledModel } from "./model.js"

let initialized = false

function getNodeFs(): typeof import("node:fs") | undefined {
  if (
    typeof process !== "undefined" &&
    typeof process.getBuiltinModule === "function"
  ) {
    return process.getBuiltinModule("node:fs")
  }
  return undefined
}

function findWasmPath(fs: typeof import("node:fs")): URL {
  const primary = new URL("../wasm/f8_wasm_bg.wasm", import.meta.url)
  const candidates = [
    primary,
    new URL("./wasm/f8_wasm_bg.wasm", import.meta.url),
    new URL("./f8_wasm_bg.wasm", import.meta.url),
  ]
  for (const url of candidates) {
    if (fs.existsSync(url)) {
      return url
    }
  }
  return primary
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
  )
}

let autoInitError: unknown = null

// Module-load auto-initialization only runs in server runtimes (Node.js, Bun, Deno)
if (isNodeLikeEnvironment()) {
  try {
    const fs = getNodeFs()
    if (!fs) {
      throw new Error("node:fs builtin module unavailable")
    }
    const wasmPath = findWasmPath(fs)
    const bytes = fs.readFileSync(wasmPath)
    initWasmSync({ module: bytes })
    initialized = true
  } catch (err: unknown) {
    // If auto-init cannot locate or load the wasm binary at import time,
    // record the error; explicit ensureInitSync() will report it with cause.
    autoInitError = err
  }
}

export function ensureInitSync(): void {
  if (initialized) {
    return
  }
  try {
    const fs = getNodeFs()
    if (!fs) {
      throw new Error(
        "Synchronous WASM initialization requires Node.js, Bun, or Deno; call await init() in browser environments"
      )
    }
    const wasmPath = findWasmPath(fs)
    const bytes = fs.readFileSync(wasmPath)
    initWasmSync({ module: bytes })
    initialized = true
    VERSION = wasmVersion()
  } catch (err: unknown) {
    const causeErr = autoInitError ?? err
    const msg = causeErr instanceof Error ? causeErr.message : String(causeErr)
    throw new F8Error(
      `Failed to synchronously initialize WASM core module: ${msg}`,
      {
        cause: causeErr,
      }
    )
  }
}

export async function init(
  moduleOrPath?: InitInput | Promise<InitInput>
): Promise<InitOutput> {
  const options =
    moduleOrPath !== undefined ? { module_or_path: moduleOrPath } : undefined
  const res = await initWasm(options)
  initialized = true
  VERSION = wasmVersion()
  return res
}

export let VERSION: string = initialized ? wasmVersion() : ""

export function defaultTheme(): Record<string, unknown> {
  ensureInitSync()
  try {
    return wasmDefaultTheme() as Record<string, unknown>
  } catch (err: unknown) {
    throw toF8Error(err)
  }
}

function renameKey(
  obj: Record<string, unknown>,
  from: string,
  to: string
): void {
  if (from in obj && !(to in obj)) {
    const val = obj[from]
    delete obj[from]
    if (val !== undefined) {
      obj[to] = val
    }
  }
}

function normalizeManifest(manifest: ProjectManifestInput): unknown {
  const rawManifest: unknown = manifest
  if (
    typeof rawManifest !== "object" ||
    rawManifest === null ||
    Array.isArray(rawManifest)
  ) {
    return rawManifest
  }
  const rawSheets: unknown = manifest.sheets
  const sheets = Array.isArray(rawSheets)
    ? manifest.sheets.map((sheet: SheetSpecInput) => {
        if (
          typeof sheet !== "object" ||
          sheet === null ||
          Array.isArray(sheet)
        ) {
          return sheet
        }
        const normSheet: Record<string, unknown> = { ...sheet }
        renameKey(normSheet, "tabColor", "tab_color")
        renameKey(normSheet, "freezePanes", "freeze_panes")
        renameKey(normSheet, "showGridlines", "show_gridlines")
        if (normSheet.tab_color === undefined) {
          delete normSheet.tab_color
        }
        if (normSheet.freeze_panes === undefined) {
          delete normSheet.freeze_panes
        }
        if (normSheet.show_gridlines === undefined) {
          delete normSheet.show_gridlines
        }
        if (normSheet.layout === undefined) {
          delete normSheet.layout
        } else if (
          typeof normSheet.layout === "object" &&
          normSheet.layout !== null &&
          !Array.isArray(normSheet.layout)
        ) {
          const layoutObj: Record<string, unknown> = {
            ...(normSheet.layout as Record<string, unknown>),
          }
          renameKey(layoutObj, "colWidth", "col_width")
          if (layoutObj.col_width === undefined) {
            delete layoutObj.col_width
          }
          normSheet.layout = layoutObj
        }
        if (Array.isArray(normSheet.bands)) {
          normSheet.bands = normSheet.bands.map((band: unknown) => {
            if (
              typeof band !== "object" ||
              band === null ||
              Array.isArray(band)
            ) {
              return band
            }
            const normBand: Record<string, unknown> = {
              ...(band as Record<string, unknown>),
            }
            renameKey(normBand, "laneGap", "lane_gap")
            if (normBand.gap === undefined) {
              delete normBand.gap
            }
            if (normBand.lane_gap === undefined) {
              delete normBand.lane_gap
            }
            if (Array.isArray(normBand.lanes)) {
              normBand.lanes = normBand.lanes.map((lane: unknown) => {
                if (
                  typeof lane !== "object" ||
                  lane === null ||
                  Array.isArray(lane)
                ) {
                  return lane
                }
                const normLane: Record<string, unknown> = {
                  ...(lane as Record<string, unknown>),
                }
                renameKey(normLane, "tableGap", "table_gap")
                if (normLane.table_gap === undefined) {
                  delete normLane.table_gap
                }
                return normLane
              })
            }
            return normBand
          })
        }
        return normSheet
      })
    : rawSheets

  const res: Record<string, unknown> = {
    model_name: manifest.modelName,
    sheets,
  }
  if (manifest.theme !== undefined) {
    res.theme = manifest.theme
  }
  return res
}

function normalizeTables(tables: readonly TableSpecInput[]): unknown {
  const rawTables: unknown = tables
  if (!Array.isArray(rawTables)) {
    return rawTables
  }
  return tables.map((t) => {
    if (typeof t !== "object" || t === null || Array.isArray(t)) {
      return t
    }
    const normTable: Record<string, unknown> = { ...t }
    const rawSubtitles: unknown = t.subtitles
    if (Array.isArray(rawSubtitles)) {
      if (t.subtitles !== undefined && t.subtitles.length > 0) {
        normTable.subtitles = [...t.subtitles]
      } else {
        delete normTable.subtitles
      }
    } else if (rawSubtitles === undefined) {
      delete normTable.subtitles
    }
    if (typeof t.currency === "string") {
      normTable.currency = t.currency.toLowerCase()
    } else if (t.currency === undefined) {
      delete normTable.currency
    }
    if (t.collapsible === undefined) {
      delete normTable.collapsible
    }
    const rawColumns: unknown = t.columns
    if (Array.isArray(rawColumns)) {
      normTable.columns = t.columns.map((c) => {
        if (
          typeof c === "object" &&
          c !== null &&
          !Array.isArray(c) &&
          c.kind === "data" &&
          typeof c.currency === "string"
        ) {
          return {
            ...c,
            currency: c.currency.toLowerCase(),
          }
        }
        return c
      })
    }
    const rawRows: unknown = t.rows
    if (Array.isArray(rawRows)) {
      normTable.rows = t.rows.map((r) => {
        if (typeof r !== "object" || r === null || Array.isArray(r)) {
          return r
        }
        if (r.kind === "sectionHeader") {
          return {
            ...r,
            kind: "section_header",
          }
        }
        if (r.kind === "data") {
          let normRow: Record<string, unknown> | null = null
          if (typeof r.currency === "string") {
            normRow = {
              ...r,
              currency: r.currency.toLowerCase(),
            }
          }
          if (
            typeof r.cells === "object" &&
            r.cells !== null &&
            !Array.isArray(r.cells)
          ) {
            const normCells: Record<string, unknown> = {}
            let cellsModified = false
            for (const [colId, cell] of Object.entries(r.cells)) {
              if (
                typeof cell === "object" &&
                cell !== null &&
                !Array.isArray(cell)
              ) {
                const normalized = normalizeDetailedCellObject(cell)
                normCells[colId] = normalized.cell
                if (normalized.modified) {
                  cellsModified = true
                }
              } else {
                normCells[colId] = cell
              }
            }
            if (cellsModified) {
              normRow = {
                ...(normRow ?? r),
                cells: normCells,
              }
            }
          }
          if (normRow !== null) {
            return normRow
          }
        }
        return r
      })
    }
    return normTable
  })
}

export function compile(
  manifest: ProjectManifestInput,
  tables: readonly TableSpecInput[]
): CompiledModel {
  ensureInitSync()
  try {
    const normalizedManifest = normalizeManifest(manifest)
    const normalizedTables = normalizeTables(tables)
    const model = wasmCompile(normalizedManifest, normalizedTables)
    return new CompiledModel(model)
  } catch (err: unknown) {
    throw toF8Error(err)
  }
}
