import { compile as coreCompile } from "./index.js";
import type { CompiledModel } from "./model.js";
import type {
  AxisRole,
  AxisSelector,
  AxisSlice,
  CellInput,
  CellInputValue,
  CellValueLiteral,
  ColumnOptions,
  ColumnSpecInput,
  CurrencyCode,
  DataColumnSpecInput,
  DataRowSpecInput,
  DetailedCellInput,
  FormatPreset,
  FreezePanesInput,
  ModelOptions,
  ModelSpec,
  PlacementSpecInput,
  ProjectManifestInput,
  RowOptions,
  RowSpecInput,
  SectionHeaderOptions,
  SectionHeaderRowSpecInput,
  SemanticStyle,
  Shade,
  SheetOptions,
  SheetSpecInput,
  TableOptions,
  TableSpecInput,
  ThemeInput,
} from "./types.js";

function normalizeCurrency(currency: CurrencyCode): CurrencyCode {
  return currency.toLowerCase() as CurrencyCode;
}

export class Cell {
  readonly value: CellValueLiteral | undefined;
  readonly expr: string | null;
  readonly style: SemanticStyle | null;
  readonly highlight: boolean | null;
  readonly shade: Shade | null;
  readonly format: FormatPreset | null;
  readonly decimals: number | null;
  readonly currency: CurrencyCode | null;
  readonly indent: number | null;
  readonly note: string | null;

  constructor(options: DetailedCellInput) {
    const hasValue = options.value !== undefined;
    const hasExpr = options.expr !== undefined;
    if (hasValue && hasExpr) {
      throw new Error("Cell cannot specify both 'value' and 'expr'");
    }
    if (!hasValue && !hasExpr) {
      throw new Error("Cell must specify either 'value' or 'expr'");
    }

    this.value = options.value;
    this.expr = options.expr ?? null;
    this.style = options.style ?? null;
    this.highlight = options.highlight ?? null;
    this.shade = options.shade ?? null;
    this.format = options.format ?? null;
    this.decimals = options.decimals ?? null;
    this.currency = options.currency ?? null;
    this.indent = options.indent ?? null;
    this.note = options.note ?? null;
  }
}

function serializeCellInput(cell: CellInputValue): CellInput {
  if (cell instanceof Cell) {
    const hasMeta =
      cell.style !== null ||
      cell.highlight !== null ||
      cell.shade !== null ||
      cell.format !== null ||
      cell.decimals !== null ||
      cell.currency !== null ||
      cell.indent !== null ||
      cell.note !== null;
    if (cell.expr !== null || hasMeta) {
      return {
        ...(cell.value !== undefined ? { value: cell.value } : {}),
        ...(cell.expr !== null ? { expr: cell.expr } : {}),
        ...(cell.style !== null ? { style: cell.style } : {}),
        ...(cell.highlight !== null ? { highlight: cell.highlight } : {}),
        ...(cell.shade !== null ? { shade: cell.shade } : {}),
        ...(cell.format !== null ? { format: cell.format } : {}),
        ...(cell.decimals !== null ? { decimals: cell.decimals } : {}),
        ...(cell.currency !== null ? { currency: normalizeCurrency(cell.currency) } : {}),
        ...(cell.indent !== null ? { indent: cell.indent } : {}),
        ...(cell.note !== null ? { note: cell.note } : {}),
      };
    }
    return cell.value ?? null;
  }

  if (typeof cell === "object" && cell !== null) {
    if (cell.currency !== undefined) {
      return {
        ...cell,
        currency: normalizeCurrency(cell.currency),
      };
    }
    return cell;
  }

  return cell;
}

interface DataColumnItem {
  readonly kind: "data";
  readonly id: string;
  readonly role: AxisRole;
  readonly group: string | null;
  readonly headers: readonly string[];
  readonly style: SemanticStyle | null;
  readonly highlight: boolean | null;
  readonly shade: Shade | null;
  readonly format: FormatPreset | null;
  readonly decimals: number | null;
  readonly currency: CurrencyCode | null;
  readonly expr: string | null;
}

interface GutterColumnItem {
  readonly kind: "gutter";
}

type ColumnItem = DataColumnItem | GutterColumnItem;

interface DataRowItem {
  readonly kind: "data";
  readonly id: string;
  readonly cells: Record<string, CellInputValue>;
  readonly expr: string | null;
  readonly style: SemanticStyle | null;
  readonly highlight: boolean | null;
  readonly shade: Shade | null;
  readonly format: FormatPreset | null;
  readonly decimals: number | null;
  readonly currency: CurrencyCode | null;
}

interface SectionHeaderRowItem {
  readonly kind: "section_header";
  readonly title: string;
  readonly indent: number | null;
  readonly note: string | null;
}

interface GutterRowItem {
  readonly kind: "gutter";
}

type RowItem = DataRowItem | SectionHeaderRowItem | GutterRowItem;

function isAxisSlice(selector: Iterable<string> | AxisSlice): selector is AxisSlice {
  return typeof selector === "object" && selector !== null && !(Symbol.iterator in selector);
}

export class Table {
  readonly id: string;
  readonly title: string;
  readonly subtitle: string | null;
  readonly currency: CurrencyCode | null;
  readonly collapsible: boolean;
  readonly rightOf: string | null;
  readonly below: string | null;
  readonly gap: number | null;

  readonly #columns: ColumnItem[] = [];
  readonly #colIds: Set<string> = new Set();
  readonly #rows: RowItem[] = [];
  readonly #dataRows: Map<string, DataRowItem> = new Map();

  constructor(id: string, title: string, options?: TableOptions) {
    if (options?.rightOf !== undefined && options?.below !== undefined) {
      throw new Error("Cannot specify both 'rightOf' and 'below'");
    }
    if (options?.gap !== undefined && (options.gap < 0 || !Number.isInteger(options.gap))) {
      throw new Error(`Table 'gap' must be non-negative integer, got ${options.gap}`);
    }

    this.id = id;
    this.title = title;
    this.subtitle = options?.subtitle ?? null;
    this.currency = options?.currency ?? null;
    this.collapsible = options?.collapsible ?? false;
    this.rightOf =
      options?.rightOf instanceof Table ? options.rightOf.id : (options?.rightOf ?? null);
    this.below = options?.below instanceof Table ? options.below.id : (options?.below ?? null);
    this.gap = options?.gap ?? null;
  }

  addColumn(id: string, options?: ColumnOptions): this {
    if (options?.header !== undefined && options?.headers !== undefined) {
      throw new Error("Cannot specify both 'header' and 'headers'");
    }
    if (options?.group !== undefined && options.group.trim().length === 0) {
      throw new Error("Column 'group' cannot be empty");
    }
    if (this.#colIds.has(id)) {
      throw new Error(`Column '${id}' already exists in table '${this.id}'`);
    }

    let normHeaders: readonly string[];
    if (options?.header !== undefined) {
      normHeaders = [options.header];
    } else if (options?.headers !== undefined) {
      normHeaders = [...options.headers];
    } else {
      normHeaders = [];
    }

    const col: DataColumnItem = {
      kind: "data",
      id,
      role: options?.role ?? "measure",
      group: options?.group ?? null,
      headers: normHeaders,
      style: options?.style ?? null,
      highlight: options?.highlight ?? null,
      shade: options?.shade ?? null,
      format: options?.format ?? null,
      decimals: options?.decimals ?? null,
      currency: options?.currency ?? null,
      expr: options?.expr ?? null,
    };
    this.#columns.push(col);
    this.#colIds.add(id);
    return this;
  }

  addColumnGutter(): this {
    this.#columns.push({ kind: "gutter" });
    return this;
  }

  addRow(id: string, options?: RowOptions): this {
    if (this.#dataRows.has(id)) {
      throw new Error(`Row '${id}' already exists in table '${this.id}'`);
    }

    const storedCells: Record<string, CellInputValue> = options?.cells ? { ...options.cells } : {};
    const row: DataRowItem = {
      kind: "data",
      id,
      cells: storedCells,
      expr: options?.expr ?? null,
      style: options?.style ?? null,
      highlight: options?.highlight ?? null,
      shade: options?.shade ?? null,
      format: options?.format ?? null,
      decimals: options?.decimals ?? null,
      currency: options?.currency ?? null,
    };
    this.#rows.push(row);
    this.#dataRows.set(id, row);
    return this;
  }

  addRowGutter(): this {
    this.#rows.push({ kind: "gutter" });
    return this;
  }

  addSectionHeader(title: string, options?: SectionHeaderOptions): this {
    this.#rows.push({
      kind: "section_header",
      title,
      indent: options?.indent ?? null,
      note: options?.note ?? null,
    });
    return this;
  }

  #resolveRowSelector(selector: AxisSelector): string[] {
    if (typeof selector === "string") {
      if (!this.#dataRows.has(selector)) {
        throw new Error(`Row '${selector}' does not exist in table '${this.id}'`);
      }
      return [selector];
    }

    if (isAxisSlice(selector)) {
      if (typeof selector.start !== "string" || typeof selector.stop !== "string") {
        throw new Error("Row slice selector requires string 'start' and 'stop' bounds");
      }
      if (!this.#dataRows.has(selector.start)) {
        throw new Error(`Row '${selector.start}' does not exist in table '${this.id}'`);
      }
      if (!this.#dataRows.has(selector.stop)) {
        throw new Error(`Row '${selector.stop}' does not exist in table '${this.id}'`);
      }
      const dataRowIds = this.#rows
        .filter((r): r is DataRowItem => r.kind === "data")
        .map((r) => r.id);
      const startIdx = dataRowIds.indexOf(selector.start);
      const stopIdx = dataRowIds.indexOf(selector.stop);
      if (startIdx > stopIdx) {
        throw new Error(
          `Inverted row slice '${selector.start}'..'${selector.stop}' in table '${this.id}'`,
        );
      }
      return dataRowIds.slice(startIdx, stopIdx + 1);
    }

    const rowIds = Array.from(selector);
    if (rowIds.length === 0) {
      throw new Error("Row selector iterable cannot be empty");
    }
    for (const rowId of rowIds) {
      if (!this.#dataRows.has(rowId)) {
        throw new Error(`Row '${rowId}' does not exist in table '${this.id}'`);
      }
    }
    return rowIds;
  }

  #resolveColSelector(selector: AxisSelector): string[] {
    if (typeof selector === "string") {
      if (!this.#colIds.has(selector)) {
        throw new Error(`Column '${selector}' does not exist in table '${this.id}'`);
      }
      return [selector];
    }

    if (isAxisSlice(selector)) {
      if (typeof selector.start !== "string" || typeof selector.stop !== "string") {
        throw new Error("Column slice selector requires string 'start' and 'stop' bounds");
      }
      if (!this.#colIds.has(selector.start)) {
        throw new Error(`Column '${selector.start}' does not exist in table '${this.id}'`);
      }
      if (!this.#colIds.has(selector.stop)) {
        throw new Error(`Column '${selector.stop}' does not exist in table '${this.id}'`);
      }
      const dataColIds = this.#columns
        .filter((c): c is DataColumnItem => c.kind === "data")
        .map((c) => c.id);
      const startIdx = dataColIds.indexOf(selector.start);
      const stopIdx = dataColIds.indexOf(selector.stop);
      if (startIdx > stopIdx) {
        throw new Error(
          `Inverted column slice '${selector.start}'..'${selector.stop}' in table '${this.id}'`,
        );
      }
      return dataColIds.slice(startIdx, stopIdx + 1);
    }

    const colIds = Array.from(selector);
    if (colIds.length === 0) {
      throw new Error("Column selector iterable cannot be empty");
    }
    for (const colId of colIds) {
      if (!this.#colIds.has(colId)) {
        throw new Error(`Column '${colId}' does not exist in table '${this.id}'`);
      }
    }
    return colIds;
  }

  #resolveCellOverride(row: string, col: string, cell: CellInputValue): Cell {
    const targetRow = this.#dataRows.get(row);
    if (!targetRow) {
      throw new Error(`Row '${row}' does not exist in table '${this.id}'`);
    }
    const hasExisting = Object.prototype.hasOwnProperty.call(targetRow.cells, col);

    let existingVal: CellValueLiteral | undefined = undefined;
    let existingExpr: string | undefined = undefined;
    let existingStyle: SemanticStyle | undefined = undefined;
    let existingHighlight: boolean | undefined = undefined;
    let existingShade: Shade | undefined = undefined;
    let existingFormat: FormatPreset | undefined = undefined;
    let existingDecimals: number | undefined = undefined;
    let existingCurrency: CurrencyCode | undefined = undefined;
    let existingIndent: number | undefined = undefined;
    let existingNote: string | undefined = undefined;

    if (hasExisting) {
      const existingCell = targetRow.cells[col];
      if (existingCell instanceof Cell) {
        existingVal = existingCell.value;
        existingExpr = existingCell.expr ?? undefined;
        existingStyle = existingCell.style ?? undefined;
        existingHighlight = existingCell.highlight ?? undefined;
        existingShade = existingCell.shade ?? undefined;
        existingFormat = existingCell.format ?? undefined;
        existingDecimals = existingCell.decimals ?? undefined;
        existingCurrency = existingCell.currency ?? undefined;
        existingIndent = existingCell.indent ?? undefined;
        existingNote = existingCell.note ?? undefined;
      } else if (typeof existingCell === "object" && existingCell !== null) {
        existingVal = existingCell.value;
        existingExpr = existingCell.expr;
        existingStyle = existingCell.style;
        existingHighlight = existingCell.highlight;
        existingShade = existingCell.shade;
        existingFormat = existingCell.format;
        existingDecimals = existingCell.decimals;
        existingCurrency = existingCell.currency;
        existingIndent = existingCell.indent;
        existingNote = existingCell.note;
      } else {
        existingVal = existingCell;
      }
    }

    let baseVal: CellValueLiteral | undefined = undefined;
    let baseExpr: string | undefined = undefined;
    let baseStyle: SemanticStyle | undefined = undefined;
    let baseHighlight: boolean | undefined = undefined;
    let baseShade: Shade | undefined = undefined;
    let baseFormat: FormatPreset | undefined = undefined;
    let baseDecimals: number | undefined = undefined;
    let baseCurrency: CurrencyCode | undefined = undefined;
    let baseIndent: number | undefined = undefined;
    let baseNote: string | undefined = undefined;

    if (cell instanceof Cell) {
      baseVal = cell.value;
      baseExpr = cell.expr ?? undefined;
      baseStyle = cell.style ?? undefined;
      baseHighlight = cell.highlight ?? undefined;
      baseShade = cell.shade ?? undefined;
      baseFormat = cell.format ?? undefined;
      baseDecimals = cell.decimals ?? undefined;
      baseCurrency = cell.currency ?? undefined;
      baseIndent = cell.indent ?? undefined;
      baseNote = cell.note ?? undefined;
    } else if (typeof cell === "object" && cell !== null) {
      baseVal = cell.value;
      baseExpr = cell.expr;
      baseStyle = cell.style;
      baseHighlight = cell.highlight;
      baseShade = cell.shade;
      baseFormat = cell.format;
      baseDecimals = cell.decimals;
      baseCurrency = cell.currency;
      baseIndent = cell.indent;
      baseNote = cell.note;
    } else {
      baseVal = cell;
    }

    const hasInputPayload = baseVal !== undefined || baseExpr !== undefined;

    let finalVal: CellValueLiteral | undefined;
    let finalExpr: string | undefined;

    if (hasInputPayload) {
      if (baseVal !== undefined && baseExpr !== undefined) {
        throw new Error(
          `Cell override for row '${row}', col '${col}' cannot specify both 'value' and 'expr'`,
        );
      }
      finalVal = baseVal;
      finalExpr = baseExpr;
    } else if (hasExisting) {
      if (existingVal !== undefined && existingExpr !== undefined) {
        throw new Error(
          `Cell override for row '${row}', col '${col}' cannot specify both 'value' and 'expr'`,
        );
      }
      if (existingVal === undefined && existingExpr === undefined) {
        throw new Error(
          `Cell override for row '${row}', col '${col}' must specify either 'value' or 'expr'`,
        );
      }
      finalVal = existingVal;
      finalExpr = existingExpr;
    } else {
      throw new Error(
        `Cell override for row '${row}', col '${col}' must specify either 'value' or 'expr'`,
      );
    }

    return new Cell({
      value: finalVal,
      expr: finalExpr,
      style: baseStyle ?? existingStyle,
      highlight: baseHighlight ?? existingHighlight,
      shade: baseShade ?? existingShade,
      format: baseFormat ?? existingFormat,
      decimals: baseDecimals ?? existingDecimals,
      currency: baseCurrency ?? existingCurrency,
      indent: baseIndent ?? existingIndent,
      note: baseNote ?? existingNote,
    });
  }

  setCell(row: string, col: string, cell: CellInputValue): this {
    const targetRow = this.#dataRows.get(row);
    if (!targetRow) {
      throw new Error(`Row '${row}' does not exist in table '${this.id}'`);
    }
    if (!this.#colIds.has(col)) {
      throw new Error(`Column '${col}' does not exist in table '${this.id}'`);
    }

    targetRow.cells[col] = this.#resolveCellOverride(row, col, cell);
    return this;
  }

  setCells(rows: AxisSelector, cols: AxisSelector, cell: CellInputValue): this {
    const rowIds = this.#resolveRowSelector(rows);
    const colIds = this.#resolveColSelector(cols);

    const updates: Array<readonly [string, string, Cell]> = [];
    for (const rId of rowIds) {
      for (const cId of colIds) {
        const resolved = this.#resolveCellOverride(rId, cId, cell);
        updates.push([rId, cId, resolved]);
      }
    }

    for (const [rId, cId, resolvedCell] of updates) {
      const targetRow = this.#dataRows.get(rId);
      if (targetRow) {
        targetRow.cells[cId] = resolvedCell;
      }
    }

    return this;
  }

  toSpec(): TableSpecInput {
    const colsSpec: ColumnSpecInput[] = this.#columns.map((col): ColumnSpecInput => {
      switch (col.kind) {
        case "gutter":
          return { kind: "gutter" };
        case "data": {
          const dataCol: DataColumnSpecInput = {
            kind: "data",
            id: col.id,
            role: col.role,
            headers: [...col.headers],
            ...(col.group !== null ? { group: col.group } : {}),
            ...(col.style !== null ? { style: col.style } : {}),
            ...(col.highlight !== null ? { highlight: col.highlight } : {}),
            ...(col.shade !== null ? { shade: col.shade } : {}),
            ...(col.format !== null ? { format: col.format } : {}),
            ...(col.decimals !== null ? { decimals: col.decimals } : {}),
            ...(col.currency !== null ? { currency: normalizeCurrency(col.currency) } : {}),
            ...(col.expr !== null ? { expr: col.expr } : {}),
          };
          return dataCol;
        }
      }
    });

    const rowsSpec: RowSpecInput[] = this.#rows.map((row): RowSpecInput => {
      switch (row.kind) {
        case "gutter":
          return { kind: "gutter" };
        case "section_header": {
          const shRow: SectionHeaderRowSpecInput = {
            kind: "section_header",
            title: row.title,
            ...(row.indent !== null ? { indent: row.indent } : {}),
            ...(row.note !== null ? { note: row.note } : {}),
          };
          return shRow;
        }
        case "data": {
          const cellsSpec: Record<string, CellInput> = {};
          for (const [colId, cellIn] of Object.entries(row.cells)) {
            cellsSpec[colId] = serializeCellInput(cellIn);
          }
          const dataRow: DataRowSpecInput = {
            kind: "data",
            id: row.id,
            cells: cellsSpec,
            ...(row.style !== null ? { style: row.style } : {}),
            ...(row.highlight !== null ? { highlight: row.highlight } : {}),
            ...(row.shade !== null ? { shade: row.shade } : {}),
            ...(row.format !== null ? { format: row.format } : {}),
            ...(row.decimals !== null ? { decimals: row.decimals } : {}),
            ...(row.currency !== null ? { currency: normalizeCurrency(row.currency) } : {}),
            ...(row.expr !== null ? { expr: row.expr } : {}),
          };
          return dataRow;
        }
      }
    });

    let placement: PlacementSpecInput | undefined;
    if (this.rightOf !== null) {
      placement = {
        kind: "right_of",
        target: this.rightOf,
        ...(this.gap !== null ? { gap: this.gap } : {}),
      };
    } else if (this.below !== null) {
      placement = {
        kind: "below",
        target: this.below,
        ...(this.gap !== null ? { gap: this.gap } : {}),
      };
    } else if (this.gap !== null) {
      placement = {
        kind: "stack",
        gap: this.gap,
      };
    }

    return {
      id: this.id,
      title: this.title,
      collapsible: this.collapsible,
      columns: colsSpec,
      rows: rowsSpec,
      ...(this.subtitle !== null ? { subtitle: this.subtitle } : {}),
      ...(this.currency !== null ? { currency: normalizeCurrency(this.currency) } : {}),
      ...(placement !== undefined ? { placement } : {}),
    };
  }
}

export class Sheet {
  readonly name: string;
  readonly tabColor: string | null;
  readonly showGridlines: boolean;
  readonly freezePanes: FreezePanesInput | null;

  readonly #model: Model | null;
  readonly #tables: Table[] = [];
  readonly #tableIds: Set<string> = new Set();

  constructor(name: string, options?: SheetOptions) {
    if (options?.freezePanes !== undefined) {
      const { row, col } = options.freezePanes;
      if (row < 0 || col < 0 || !Number.isInteger(row) || !Number.isInteger(col)) {
        throw new Error(
          `freezePanes must have non-negative integer row and col, got (${row}, ${col})`,
        );
      }
    }

    this.name = name;
    this.tabColor = options?.tabColor ?? null;
    this.showGridlines = options?.showGridlines ?? true;
    this.freezePanes =
      options?.freezePanes !== undefined
        ? { row: options.freezePanes.row, col: options.freezePanes.col }
        : null;
    this.#model = options?.model ?? null;
  }

  get tables(): readonly Table[] {
    return [...this.#tables];
  }

  addTable(id: string, title: string, options?: TableOptions): Table {
    if (this.#model !== null) {
      for (const s of this.#model.sheets) {
        for (const t of s.tables) {
          if (t.id === id) {
            throw new Error(`Table '${id}' already exists in model '${this.#model.name}'`);
          }
        }
      }
    } else if (this.#tableIds.has(id)) {
      throw new Error(`Table '${id}' already exists in sheet '${this.name}'`);
    }

    const table = new Table(id, title, options);
    this.#tables.push(table);
    this.#tableIds.add(id);
    return table;
  }

  getTable(id: string): Table {
    for (const table of this.#tables) {
      if (table.id === id) {
        return table;
      }
    }
    throw new Error(`Table '${id}' not found in sheet '${this.name}'`);
  }

  toSpec(): SheetSpecInput {
    return {
      name: this.name,
      showGridlines: this.showGridlines,
      tables: this.#tables.map((t) => t.id),
      ...(this.tabColor !== null ? { tabColor: this.tabColor } : {}),
      ...(this.freezePanes !== null
        ? { freezePanes: { row: this.freezePanes.row, col: this.freezePanes.col } }
        : {}),
    };
  }
}

function resolveThemeInput(theme: ThemeInput): string | Readonly<Record<string, unknown>> {
  if (theme instanceof URL) {
    const fs =
      typeof process !== "undefined" && typeof process.getBuiltinModule === "function"
        ? process.getBuiltinModule("node:fs")
        : undefined;
    if (!fs) {
      throw new Error("Loading a theme from a file URL requires filesystem support (node:fs)");
    }
    const rawText = fs.readFileSync(theme, "utf-8");
    const parsed: unknown = JSON.parse(rawText);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      throw new Error(`Theme JSON file '${theme.toString()}' must contain a JSON object at root`);
    }
    return parsed as Readonly<Record<string, unknown>>;
  }
  return theme;
}

export class Model {
  readonly name: string;
  readonly theme: string | Readonly<Record<string, unknown>>;

  readonly #sheets: Sheet[] = [];
  readonly #sheetNames: Set<string> = new Set();

  constructor(options?: ModelOptions) {
    this.name = options?.name ?? "Model";
    this.theme = resolveThemeInput(options?.theme ?? "corporate_navy");
  }

  get sheets(): readonly Sheet[] {
    return [...this.#sheets];
  }

  get tables(): readonly Table[] {
    return this.#sheets.flatMap((s) => s.tables);
  }

  addSheet(name: string, options?: Omit<SheetOptions, "model">): Sheet {
    if (this.#sheetNames.has(name)) {
      throw new Error(`Sheet '${name}' already exists in model '${this.name}'`);
    }

    const sheet = new Sheet(name, { ...options, model: this });
    this.#sheets.push(sheet);
    this.#sheetNames.add(name);
    return sheet;
  }

  getSheet(name: string): Sheet {
    for (const sheet of this.#sheets) {
      if (sheet.name === name) {
        return sheet;
      }
    }
    throw new Error(`Sheet '${name}' not found in model '${this.name}'`);
  }

  getTable(id: string): Table {
    for (const sheet of this.#sheets) {
      for (const table of sheet.tables) {
        if (table.id === id) {
          return table;
        }
      }
    }
    throw new Error(`Table '${id}' not found in model '${this.name}'`);
  }

  toSpec(): ModelSpec {
    const manifest: ProjectManifestInput = {
      modelName: this.name,
      theme: this.theme,
      sheets: this.#sheets.map((s) => s.toSpec()),
    };
    const tables: TableSpecInput[] = this.tables.map((t) => t.toSpec());
    return { manifest, tables };
  }

  compile(): CompiledModel {
    const { manifest, tables } = this.toSpec();
    return coreCompile(manifest, tables);
  }
}
