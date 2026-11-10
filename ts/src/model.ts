import { TableAuditor } from "./audit.js";
import { EmissionError, F8Error, toF8Error } from "./errors.js";
import type {
  CellInspection,
  CellNeighborInspection,
  CellScalarValue,
  CellValueDict,
  ColumnInspection,
  CurrencyCode,
  DependencyNode,
  DependencyTraceOptions,
  DependencyTree,
  EmissionOptions,
  FormatPreset,
  FormattedOptions,
  ModelSummary,
  RowInspection,
  SemanticStyle,
  Shade,
  SheetCellInspection,
  SheetCellKind,
  SheetInspection,
  SheetRowInspection,
  SheetSummary,
  TableCellInfo,
  TableInspection,
  TableSummary,
} from "./types.js";

export interface WasmCompiledModelBinding {
  emitWorkbook(options?: unknown): Uint8Array;
  summary(): unknown;
  value(target: string, row?: string, col?: string): unknown;
  num(target: string, row?: string, col?: string): number;
  formatted(target: string, row?: string, col?: string, unit?: string): string;
  inspectCell(target: string, row?: string, col?: string): unknown;
  inspectTable(table: string): unknown;
  inspectSheet(sheet: string): unknown;
  precedentsOf(target: string, row?: string, col?: string, recursive?: boolean): unknown;
  dependentsOf(target: string, row?: string, col?: string, recursive?: boolean): unknown;
}

interface RawCoordinate {
  readonly table: string;
  readonly row: string;
  readonly col: string;
}

interface RawTableSummary {
  readonly id: string;
  readonly title: string;
  readonly row_count: number;
  readonly col_count: number;
  readonly cell_count: number;
  readonly driver_count: number;
  readonly formula_count: number;
}

interface RawSheetSummary {
  readonly name: string;
  readonly tables: readonly RawTableSummary[];
  readonly total_cells: number;
}

interface RawModelSummary {
  readonly model_name: string;
  readonly theme_name: string;
  readonly sheets: readonly RawSheetSummary[];
  readonly total_sheets: number;
  readonly total_tables: number;
  readonly total_cells: number;
  readonly total_drivers: number;
  readonly total_formulas: number;
}

interface RawCellNeighborInspection {
  readonly cell_id: RawCoordinate;
  readonly sheet_address: string;
  readonly value: CellValueDict | null;
  readonly formatted_value: string | null;
}

interface RawCellInspection {
  readonly coordinate: RawCoordinate;
  readonly sheet: string;
  readonly physical_address: string;
  readonly evaluated_value: CellValueDict;
  readonly formatted_value: string | null;
  readonly format: FormatPreset | null;
  readonly decimals: number | null;
  readonly currency: CurrencyCode | null;
  readonly is_highlight: boolean;
  readonly shade: Shade | null;
  readonly indent: number;
  readonly is_formula: boolean;
  readonly formula_expr: string | null;
  readonly excel_formula: string | null;
  readonly note: string | null;
  readonly precedents: readonly RawCellNeighborInspection[];
  readonly dependents: readonly RawCellNeighborInspection[];
}

interface RawColumnInspection {
  readonly id: string | null;
  readonly group: string | null;
  readonly headers: readonly string[];
  readonly is_gutter: boolean;
  readonly is_descriptor: boolean;
  readonly style: SemanticStyle | null;
  readonly shade: Shade | null;
}

interface RawRowInspection {
  readonly id: string | null;
  readonly section_title: string | null;
  readonly is_section_header: boolean;
  readonly is_gutter: boolean;
  readonly indent: number;
  readonly style: SemanticStyle | null;
  readonly shade: Shade | null;
}

interface RawTableCellInfo {
  readonly value: CellValueDict;
  readonly formatted_value: string | null;
  readonly is_formula: boolean;
  readonly formula_expr: string | null;
  readonly excel_formula: string | null;
  readonly physical_a1: string;
  readonly format: FormatPreset | null;
  readonly decimals: number | null;
  readonly currency: CurrencyCode | null;
}

interface RawTableInspection {
  readonly table_id: string;
  readonly title: string;
  readonly subtitle: string | null;
  readonly sheet: string;
  readonly columns: readonly RawColumnInspection[];
  readonly rows: readonly RawRowInspection[];
  readonly cells: Readonly<Record<string, RawTableCellInfo>>;
}

interface RawSheetCellInspection {
  readonly physical_col: number;
  readonly physical_a1: string;
  readonly table_id: string | null;
  readonly row_id: string | null;
  readonly col_id: string | null;
  readonly kind: SheetCellKind;
  readonly is_descriptor: boolean;
  readonly value: CellValueDict;
  readonly formatted_value: string | null;
  readonly formula_expr: string | null;
  readonly excel_formula: string | null;
}

interface RawSheetRowInspection {
  readonly physical_row: number;
  readonly excel_row: number;
  readonly cells: readonly RawSheetCellInspection[];
}

interface RawSheetInspection {
  readonly sheet_id: string;
  readonly title: string;
  readonly tables: readonly string[];
  readonly min_col: number;
  readonly max_col: number;
  readonly min_row: number;
  readonly max_row: number;
  readonly col_widths: Readonly<Record<number, number>>;
  readonly rows: readonly RawSheetRowInspection[];
}

interface RawDependencyNode {
  readonly cell_id: RawCoordinate;
  readonly sheet_address: string;
  readonly value: CellValueDict | null;
  readonly formatted_value: string | null;
  readonly excel_formula: string | null;
  readonly is_pruned: boolean;
  readonly children: readonly RawDependencyNode[];
}

interface RawDependencyTree {
  readonly root: RawCoordinate;
  readonly direct_count: number;
  readonly transitive_count: number;
  readonly nodes: readonly RawDependencyNode[];
}

function mapTableSummary(raw: RawTableSummary): TableSummary {
  return {
    id: raw.id,
    title: raw.title,
    rowCount: raw.row_count,
    colCount: raw.col_count,
    cellCount: raw.cell_count,
    driverCount: raw.driver_count,
    formulaCount: raw.formula_count,
  };
}

function mapSheetSummary(raw: RawSheetSummary): SheetSummary {
  return {
    name: raw.name,
    tables: (raw.tables ?? []).map(mapTableSummary),
    totalCells: raw.total_cells,
  };
}

function mapModelSummary(raw: RawModelSummary): ModelSummary {
  return {
    modelName: raw.model_name,
    themeName: raw.theme_name,
    sheets: (raw.sheets ?? []).map(mapSheetSummary),
    totalSheets: raw.total_sheets,
    totalTables: raw.total_tables,
    totalCells: raw.total_cells,
    totalDrivers: raw.total_drivers,
    totalFormulas: raw.total_formulas,
  };
}

function mapCellNeighborInspection(raw: RawCellNeighborInspection): CellNeighborInspection {
  return {
    cellId: {
      table: raw.cell_id.table,
      row: raw.cell_id.row,
      col: raw.cell_id.col,
    },
    sheetAddress: raw.sheet_address,
    value: raw.value ?? null,
    formattedValue: raw.formatted_value ?? null,
  };
}

function mapCellInspection(raw: RawCellInspection): CellInspection {
  return {
    coordinate: {
      table: raw.coordinate.table,
      row: raw.coordinate.row,
      col: raw.coordinate.col,
    },
    sheet: raw.sheet,
    physicalAddress: raw.physical_address,
    evaluatedValue: raw.evaluated_value,
    formattedValue: raw.formatted_value ?? null,
    format: raw.format ?? null,
    decimals: raw.decimals ?? null,
    currency: raw.currency ?? null,
    isHighlight: Boolean(raw.is_highlight),
    shade: raw.shade ?? null,
    indent: raw.indent ?? 0,
    isFormula: Boolean(raw.is_formula),
    formulaExpr: raw.formula_expr ?? null,
    excelFormula: raw.excel_formula ?? null,
    note: raw.note ?? null,
    precedents: (raw.precedents ?? []).map(mapCellNeighborInspection),
    dependents: (raw.dependents ?? []).map(mapCellNeighborInspection),
  };
}

function mapColumnInspection(col: RawColumnInspection): ColumnInspection {
  return {
    id: col.id ?? null,
    group: col.group ?? null,
    headers: col.headers ?? [],
    isGutter: Boolean(col.is_gutter),
    isDescriptor: Boolean(col.is_descriptor),
    style: col.style ?? null,
    shade: col.shade ?? null,
  };
}

function mapRowInspection(row: RawRowInspection): RowInspection {
  return {
    id: row.id ?? null,
    sectionTitle: row.section_title ?? null,
    isSectionHeader: Boolean(row.is_section_header),
    isGutter: Boolean(row.is_gutter),
    indent: row.indent ?? 0,
    style: row.style ?? null,
    shade: row.shade ?? null,
  };
}

function mapTableCellInfo(cell: RawTableCellInfo): TableCellInfo {
  return {
    value: cell.value,
    formattedValue: cell.formatted_value ?? null,
    isFormula: Boolean(cell.is_formula),
    formulaExpr: cell.formula_expr ?? null,
    excelFormula: cell.excel_formula ?? null,
    physicalA1: cell.physical_a1,
    format: cell.format ?? null,
    decimals: cell.decimals ?? null,
    currency: cell.currency ?? null,
  };
}

function mapTableInspection(raw: RawTableInspection): TableInspection {
  const cells: Record<string, TableCellInfo> = {};
  for (const [key, cell] of Object.entries(raw.cells ?? {})) {
    cells[key] = mapTableCellInfo(cell);
  }
  return {
    tableId: raw.table_id,
    title: raw.title,
    subtitle: raw.subtitle ?? null,
    sheet: raw.sheet,
    columns: (raw.columns ?? []).map(mapColumnInspection),
    rows: (raw.rows ?? []).map(mapRowInspection),
    cells,
  };
}

function mapSheetCellInspection(cell: RawSheetCellInspection): SheetCellInspection {
  return {
    physicalCol: cell.physical_col,
    physicalA1: cell.physical_a1,
    tableId: cell.table_id ?? null,
    rowId: cell.row_id ?? null,
    colId: cell.col_id ?? null,
    kind: cell.kind,
    isDescriptor: Boolean(cell.is_descriptor),
    value: cell.value,
    formattedValue: cell.formatted_value ?? null,
    formulaExpr: cell.formula_expr ?? null,
    excelFormula: cell.excel_formula ?? null,
  };
}

function mapSheetRowInspection(row: RawSheetRowInspection): SheetRowInspection {
  return {
    physicalRow: row.physical_row,
    excelRow: row.excel_row,
    cells: (row.cells ?? []).map(mapSheetCellInspection),
  };
}

function mapSheetInspection(raw: RawSheetInspection): SheetInspection {
  return {
    sheetId: raw.sheet_id,
    title: raw.title,
    tables: raw.tables ?? [],
    minCol: raw.min_col,
    maxCol: raw.max_col,
    minRow: raw.min_row,
    maxRow: raw.max_row,
    colWidths: raw.col_widths ?? {},
    rows: (raw.rows ?? []).map(mapSheetRowInspection),
  };
}

function mapDependencyNode(node: RawDependencyNode): DependencyNode {
  return {
    cellId: {
      table: node.cell_id.table,
      row: node.cell_id.row,
      col: node.cell_id.col,
    },
    sheetAddress: node.sheet_address,
    value: node.value ?? null,
    formattedValue: node.formatted_value ?? null,
    excelFormula: node.excel_formula ?? null,
    isPruned: Boolean(node.is_pruned),
    children: (node.children ?? []).map(mapDependencyNode),
  };
}

function mapDependencyTree(raw: RawDependencyTree): DependencyTree {
  return {
    root: {
      table: raw.root.table,
      row: raw.root.row,
      col: raw.root.col,
    },
    directCount: raw.direct_count,
    transitiveCount: raw.transitive_count,
    nodes: (raw.nodes ?? []).map(mapDependencyNode),
  };
}

export class CompiledModel {
  readonly #wasm: WasmCompiledModelBinding;

  constructor(wasm: WasmCompiledModelBinding) {
    this.#wasm = wasm;
  }

  emitWorkbook(options?: EmissionOptions): Uint8Array {
    try {
      const rawOptions = options
        ? {
            values_only: options.valuesOnly ?? false,
            omit_formula_cache: options.omitFormulaCache ?? false,
            title: options.title,
            author: options.author,
          }
        : undefined;
      return this.#wasm.emitWorkbook(rawOptions);
    } catch (err) {
      throw toF8Error(err);
    }
  }

  save(path: string, options?: EmissionOptions): void {
    try {
      const bytes = this.emitWorkbook(options);
      const fs =
        typeof process !== "undefined" && typeof process.getBuiltinModule === "function"
          ? process.getBuiltinModule("node:fs")
          : undefined;
      const pathMod =
        typeof process !== "undefined" && typeof process.getBuiltinModule === "function"
          ? process.getBuiltinModule("node:path")
          : undefined;
      if (!fs || !pathMod) {
        throw new Error("save() requires filesystem support (node:fs)");
      }
      const parent = pathMod.dirname(path);
      if (parent && !fs.existsSync(parent)) {
        fs.mkdirSync(parent, { recursive: true });
      }
      fs.writeFileSync(path, bytes);
    } catch (err) {
      if (err instanceof F8Error) {
        throw err;
      }
      const msg = err instanceof Error ? err.message : String(err);
      throw new EmissionError(`Failed to save workbook to '${path}': ${msg}`);
    }
  }

  summary(): ModelSummary {
    try {
      const raw = this.#wasm.summary() as RawModelSummary;
      return mapModelSummary(raw);
    } catch (err) {
      throw toF8Error(err);
    }
  }

  value(coordinate: string): CellScalarValue;
  value(table: string, row: string, col: string): CellScalarValue;
  value(target: string, row?: string, col?: string): CellScalarValue {
    try {
      const val = this.#wasm.value(target, row, col);
      return val as CellScalarValue;
    } catch (err) {
      throw toF8Error(err);
    }
  }

  num(coordinate: string): number;
  num(table: string, row: string, col: string): number;
  num(target: string, row?: string, col?: string): number {
    try {
      return this.#wasm.num(target, row, col);
    } catch (err) {
      throw toF8Error(err);
    }
  }

  formatted(coordinate: string, options?: FormattedOptions): string;
  formatted(table: string, row: string, col: string, options?: FormattedOptions): string;
  formatted(
    target: string,
    arg2?: string | FormattedOptions,
    arg3?: string,
    arg4?: FormattedOptions,
  ): string {
    try {
      let row: string | undefined;
      let col: string | undefined;
      let options: FormattedOptions | undefined;

      if (typeof arg2 === "string") {
        row = arg2;
        col = arg3;
        options = arg4;
      } else {
        options = arg2;
      }

      const unit = options?.unit ?? "";
      return this.#wasm.formatted(target, row, col, unit);
    } catch (err) {
      throw toF8Error(err);
    }
  }

  inspectCell(coordinate: string): CellInspection;
  inspectCell(table: string, row: string, col: string): CellInspection;
  inspectCell(target: string, row?: string, col?: string): CellInspection {
    try {
      const raw = this.#wasm.inspectCell(target, row, col) as RawCellInspection;
      return mapCellInspection(raw);
    } catch (err) {
      throw toF8Error(err);
    }
  }

  inspectTable(table: string): TableInspection {
    try {
      const raw = this.#wasm.inspectTable(table) as RawTableInspection;
      return mapTableInspection(raw);
    } catch (err) {
      throw toF8Error(err);
    }
  }

  table(table: string): TableAuditor {
    return new TableAuditor(this.inspectTable(table));
  }

  inspectSheet(sheet: string): SheetInspection {
    try {
      const raw = this.#wasm.inspectSheet(sheet) as RawSheetInspection;
      return mapSheetInspection(raw);
    } catch (err) {
      throw toF8Error(err);
    }
  }

  precedentsOf(coordinate: string, options?: DependencyTraceOptions): DependencyTree;
  precedentsOf(
    table: string,
    row: string,
    col: string,
    options?: DependencyTraceOptions,
  ): DependencyTree;
  precedentsOf(
    target: string,
    arg2?: string | DependencyTraceOptions,
    arg3?: string,
    arg4?: DependencyTraceOptions,
  ): DependencyTree {
    try {
      let row: string | undefined;
      let col: string | undefined;
      let recursive = false;

      if (typeof arg2 === "string") {
        row = arg2;
        col = arg3;
        if (arg4 && typeof arg4 === "object") {
          recursive = arg4.recursive ?? false;
        }
      } else if (arg2 && typeof arg2 === "object") {
        recursive = arg2.recursive ?? false;
      }

      const raw = this.#wasm.precedentsOf(target, row, col, recursive) as RawDependencyTree;
      return mapDependencyTree(raw);
    } catch (err) {
      throw toF8Error(err);
    }
  }

  dependentsOf(coordinate: string, options?: DependencyTraceOptions): DependencyTree;
  dependentsOf(
    table: string,
    row: string,
    col: string,
    options?: DependencyTraceOptions,
  ): DependencyTree;
  dependentsOf(
    target: string,
    arg2?: string | DependencyTraceOptions,
    arg3?: string,
    arg4?: DependencyTraceOptions,
  ): DependencyTree {
    try {
      let row: string | undefined;
      let col: string | undefined;
      let recursive = false;

      if (typeof arg2 === "string") {
        row = arg2;
        col = arg3;
        if (arg4 && typeof arg4 === "object") {
          recursive = arg4.recursive ?? false;
        }
      } else if (arg2 && typeof arg2 === "object") {
        recursive = arg2.recursive ?? false;
      }

      const raw = this.#wasm.dependentsOf(target, row, col, recursive) as RawDependencyTree;
      return mapDependencyTree(raw);
    } catch (err) {
      throw toF8Error(err);
    }
  }
}
