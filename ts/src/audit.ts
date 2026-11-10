import type {
  CellScalarValue,
  ColumnInspection,
  FormattedOptions,
  RowInspection,
  SplitDict,
  TableCellInfo,
  TableInspection,
  TableMarkdownOptions,
  TableOrient,
  TableViewMode,
} from "./types.js";

const EXCEL_ERROR_MAP: Readonly<Record<string, string>> = {
  Div0: "#DIV/0!",
  Value: "#VALUE!",
  Ref: "#REF!",
  Num: "#NUM!",
  Na: "#N/A",
  Name: "#NAME?",
};

export function unboxCellValue(val: unknown): CellScalarValue {
  if (val === null || val === undefined) {
    return null;
  }
  if (typeof val === "string") {
    if (val.toLowerCase() === "empty") {
      return null;
    }
    return val;
  }
  if (typeof val === "boolean") {
    return val;
  }
  if (typeof val === "number") {
    return val;
  }
  if (typeof val === "object") {
    const mapping = val as Record<string, unknown>;
    if ("Number" in mapping) {
      const num = mapping.Number;
      if (typeof num === "number") {
        return num;
      }
      return null;
    }
    if ("Text" in mapping) {
      const text = mapping.Text;
      if (typeof text === "string") {
        return text;
      }
      if (typeof text === "number" || typeof text === "boolean") {
        return String(text);
      }
      return "";
    }
    if ("Boolean" in mapping) {
      return Boolean(mapping.Boolean);
    }
    if ("Empty" in mapping) {
      return null;
    }
    if ("Error" in mapping) {
      const err = mapping.Error;
      if (typeof err === "string") {
        return EXCEL_ERROR_MAP[err] ?? `#${err}!`;
      }
      return "#ERROR!";
    }
    return null;
  }
  if (typeof val === "bigint") {
    return String(val);
  }
  return null;
}

export function formatWithUnit(formatted: string, unit = ""): string {
  if (!formatted || !unit) {
    return formatted;
  }
  if (formatted.endsWith(")")) {
    return `${formatted.slice(0, -1)}${unit})`;
  }
  return `${formatted}${unit}`;
}

interface RenderColumn {
  readonly id: string | null;
  readonly header: string;
  readonly isDescriptor: boolean;
}

export class TableAuditor {
  readonly #raw: TableInspection;
  readonly #id: string;
  readonly #title: string;
  readonly #subtitle: string | null;
  readonly #sheet: string;
  readonly #columns: readonly string[];
  readonly #measureColumns: readonly string[];
  readonly #rows: readonly string[];
  readonly #rowSet: ReadonlySet<string>;
  readonly #cellMap: Map<string, Map<string, TableCellInfo>>;

  constructor(raw: TableInspection) {
    this.#raw = raw;
    this.#id = raw.tableId;
    this.#title = raw.title;
    this.#subtitle = raw.subtitle ?? null;
    this.#sheet = raw.sheet;

    const cols: string[] = [];
    const measureCols: string[] = [];
    for (const c of raw.columns ?? []) {
      if (!c.isGutter && c.id !== null) {
        cols.push(c.id);
        if (!c.isDescriptor) {
          measureCols.push(c.id);
        }
      }
    }
    this.#columns = cols;
    this.#measureColumns = measureCols;

    const dataRows: string[] = [];
    for (const r of raw.rows ?? []) {
      if (!r.isGutter && !r.isSectionHeader && r.id !== null) {
        dataRows.push(r.id);
      }
    }
    this.#rows = dataRows;
    this.#rowSet = new Set(dataRows);

    this.#cellMap = new Map();
    const prefix = `${this.#id}["`;
    const suffix = '"]';
    for (const [k, cellInfo] of Object.entries(raw.cells ?? {})) {
      if (k.startsWith(prefix) && k.endsWith(suffix)) {
        const inner = k.slice(prefix.length, k.length - suffix.length);
        const sepIdx = inner.indexOf('", "');
        if (sepIdx !== -1) {
          const r = inner.slice(0, sepIdx);
          const c = inner.slice(sepIdx + 4);
          this.#setCell(r, c, cellInfo);
        }
      }
    }
  }

  #setCell(row: string, col: string, info: TableCellInfo): void {
    let rowMap = this.#cellMap.get(row);
    if (rowMap === undefined) {
      rowMap = new Map();
      this.#cellMap.set(row, rowMap);
    }
    rowMap.set(col, info);
  }

  #getCell(row: string, col: string): TableCellInfo | undefined {
    const cached = this.#cellMap.get(row)?.get(col);
    if (cached !== undefined) {
      return cached;
    }
    const canonicalKey = `${this.#id}["${row}", "${col}"]`;
    if (Object.prototype.hasOwnProperty.call(this.#raw.cells, canonicalKey)) {
      const info = this.#raw.cells[canonicalKey];
      if (info !== undefined) {
        this.#setCell(row, col, info);
        return info;
      }
    }
    return undefined;
  }

  get id(): string {
    return this.#id;
  }

  get title(): string {
    return this.#title;
  }

  get subtitle(): string | null {
    return this.#subtitle;
  }

  get sheet(): string {
    return this.#sheet;
  }

  get columns(): string[] {
    return [...this.#columns];
  }

  get measureColumns(): string[] {
    return [...this.#measureColumns];
  }

  get rows(): string[] {
    return [...this.#rows];
  }

  get shape(): [number, number] {
    return [this.#rows.length, this.#columns.length];
  }

  get length(): number {
    return this.#rows.length;
  }

  cell(row: string, col: string): TableCellInfo {
    const info = this.#getCell(row, col);
    if (info !== undefined) {
      return info;
    }
    throw new Error(`Coordinate ('${row}', '${col}') not found in table '${this.#id}'`);
  }

  value(row: string, col: string): CellScalarValue {
    return unboxCellValue(this.cell(row, col).value);
  }

  num(row: string, col: string): number {
    const val = this.value(row, col);
    if (typeof val !== "number") {
      throw new TypeError(
        `Cell ('${row}', '${col}') in table '${this.#id}' did not evaluate to a number (got ${JSON.stringify(val)})`,
      );
    }
    return val;
  }

  formatted(row: string, col: string, options?: FormattedOptions): string {
    const cellInfo = this.cell(row, col);
    let valStr = cellInfo.formattedValue;
    if (valStr === null || valStr === undefined) {
      const rawVal = unboxCellValue(cellInfo.value);
      valStr = rawVal !== null ? String(rawVal) : "";
    }
    return formatWithUnit(valStr, options?.unit);
  }

  rowValues(row: string, cols?: readonly string[]): CellScalarValue[] {
    if (!this.#rowSet.has(row)) {
      throw new Error(`Row '${row}' not found in table '${this.#id}'`);
    }
    const targetCols = cols ?? this.#measureColumns;
    return targetCols.map((col) => this.value(row, col));
  }

  rowNums(row: string, cols?: readonly string[]): number[] {
    if (!this.#rowSet.has(row)) {
      throw new Error(`Row '${row}' not found in table '${this.#id}'`);
    }
    const targetCols = cols ?? this.#measureColumns;
    return targetCols.map((col) => this.num(row, col));
  }

  formula(row: string, col: string): string | null {
    return this.cell(row, col).excelFormula ?? null;
  }

  expr(row: string, col: string): string | null {
    return this.cell(row, col).formulaExpr ?? null;
  }

  address(row: string, col: string): string {
    return this.cell(row, col).physicalA1 ?? "";
  }

  toDict(orient?: "records"): Record<string, CellScalarValue>[];
  toDict(orient: "dict"): Record<string, CellScalarValue[]>;
  toDict(orient: "index"): Record<string, Record<string, CellScalarValue>>;
  toDict(orient: "split"): SplitDict;
  toDict(
    orient?: TableOrient,
  ):
    | Record<string, CellScalarValue>[]
    | Record<string, CellScalarValue[]>
    | Record<string, Record<string, CellScalarValue>>
    | SplitDict;
  toDict(
    orient: TableOrient = "records",
  ):
    | Record<string, CellScalarValue>[]
    | Record<string, CellScalarValue[]>
    | Record<string, Record<string, CellScalarValue>>
    | SplitDict {
    switch (orient) {
      case "records": {
        const records: Record<string, CellScalarValue>[] = [];
        for (const r of this.#rows) {
          const rowDict: Record<string, CellScalarValue> = {};
          for (const c of this.#columns) {
            const cellInfo = this.#getCell(r, c);
            rowDict[c] = cellInfo !== undefined ? unboxCellValue(cellInfo.value) : null;
          }
          records.push(rowDict);
        }
        return records;
      }
      case "dict": {
        const colDict: Record<string, CellScalarValue[]> = {};
        for (const c of this.#columns) {
          colDict[c] = [];
        }
        for (const r of this.#rows) {
          for (const c of this.#columns) {
            const cellInfo = this.#getCell(r, c);
            const val = cellInfo !== undefined ? unboxCellValue(cellInfo.value) : null;
            colDict[c]?.push(val);
          }
        }
        return colDict;
      }
      case "index": {
        const indexDict: Record<string, Record<string, CellScalarValue>> = {};
        for (const r of this.#rows) {
          const rowDict: Record<string, CellScalarValue> = {};
          for (const c of this.#columns) {
            const cellInfo = this.#getCell(r, c);
            rowDict[c] = cellInfo !== undefined ? unboxCellValue(cellInfo.value) : null;
          }
          indexDict[r] = rowDict;
        }
        return indexDict;
      }
      case "split": {
        const data: CellScalarValue[][] = [];
        for (const r of this.#rows) {
          const rowData: CellScalarValue[] = [];
          for (const c of this.#columns) {
            const cellInfo = this.#getCell(r, c);
            rowData.push(cellInfo !== undefined ? unboxCellValue(cellInfo.value) : null);
          }
          data.push(rowData);
        }
        return {
          columns: [...this.#columns],
          index: [...this.#rows],
          data,
        };
      }
    }
    throw new Error(
      `Invalid orient '${String(orient)}'. Expected one of 'records', 'dict', 'index', 'split'.`,
    );
  }

  toMarkdown(options?: TableMarkdownOptions): string;
  toMarkdown(view: TableViewMode, options?: Omit<TableMarkdownOptions, "view">): string;
  toMarkdown(
    arg1?: TableViewMode | TableMarkdownOptions,
    arg2?: Omit<TableMarkdownOptions, "view">,
  ): string {
    let view: TableViewMode;
    let opts: Omit<TableMarkdownOptions, "view"> | undefined;

    if (typeof arg1 === "string") {
      view = arg1;
      opts = arg2;
    } else {
      opts = arg1;
      view = arg1?.view ?? "values";
    }

    if (view !== "values" && view !== "formulas" && view !== "expr" && view !== "addresses") {
      throw new Error(
        `Invalid view mode '${String(view)}'. Expected one of 'values', 'formulas', 'expr', 'addresses'.`,
      );
    }

    const rows = opts?.rows;
    const cols = opts?.cols;
    const includeSectionHeaders = opts?.includeSectionHeaders;
    const showIds = opts?.showIds ?? false;

    const nonGutterCols: ColumnInspection[] = (this.#raw.columns ?? []).filter((c) => !c.isGutter);
    if (nonGutterCols.length === 0) {
      return "";
    }

    const colSpecById = new Map<string, ColumnInspection>();
    for (const c of nonGutterCols) {
      if (c.id !== null) {
        colSpecById.set(c.id, c);
      }
    }

    let selectedCols: ColumnInspection[];
    if (cols === undefined) {
      selectedCols = nonGutterCols;
    } else {
      for (const cid of cols) {
        if (!colSpecById.has(cid)) {
          throw new Error(`Column '${cid}' not found in table '${this.#id}'`);
        }
      }
      const requestedColSet = new Set(cols);
      const retainedDescriptors = nonGutterCols.filter(
        (c) => c.isDescriptor && (c.id === null || !requestedColSet.has(c.id)),
      );
      const requestedSpecs: ColumnInspection[] = [];
      for (const cid of cols) {
        const spec = colSpecById.get(cid);
        if (spec !== undefined) {
          requestedSpecs.push(spec);
        }
      }
      selectedCols = [...retainedDescriptors, ...requestedSpecs];
    }

    if (selectedCols.length === 0) {
      return "";
    }

    const rawRows: RowInspection[] = (this.#raw.rows ?? []).filter((r) => !r.isGutter);
    const rowSpecById = new Map<string, RowInspection>();
    for (const r of rawRows) {
      if (!r.isSectionHeader && r.id !== null) {
        rowSpecById.set(r.id, r);
      }
    }

    let selectedRows: RowInspection[];
    if (rows === undefined) {
      const showSections = includeSectionHeaders ?? true;
      selectedRows = rawRows.filter((r) => showSections || !r.isSectionHeader);
    } else {
      for (const rid of rows) {
        if (!rowSpecById.has(rid)) {
          throw new Error(`Row '${rid}' not found in table '${this.#id}'`);
        }
      }
      const showSections = includeSectionHeaders ?? false;
      if (!showSections) {
        selectedRows = [];
        for (const rid of rows) {
          const spec = rowSpecById.get(rid);
          if (spec !== undefined) {
            selectedRows.push(spec);
          }
        }
      } else {
        const requestedRowSet = new Set(rows);
        selectedRows = [];
        let pendingHeader: RowInspection | null = null;
        for (const r of rawRows) {
          if (r.isSectionHeader) {
            pendingHeader = r;
          } else if (r.id !== null && requestedRowSet.has(r.id)) {
            if (pendingHeader !== null) {
              selectedRows.push(pendingHeader);
              pendingHeader = null;
            }
            selectedRows.push(r);
          }
        }
      }
    }

    const renderCols: RenderColumn[] = selectedCols.map((c) => {
      const colId = c.id;
      const isDescriptor = c.isDescriptor;
      const stackedHeader = (c.headers ?? []).filter((h) => h.length > 0).join(" ");
      const group = c.group;
      let displayHeader: string;
      if (group && stackedHeader) {
        displayHeader = `${group}: ${stackedHeader}`;
      } else if (group) {
        displayHeader = group;
      } else if (stackedHeader) {
        displayHeader = stackedHeader;
      } else {
        displayHeader = colId ?? "";
      }

      const headerLabel =
        showIds && colId && displayHeader.toLowerCase() !== colId.toLowerCase()
          ? `${displayHeader} [${colId}]`
          : displayHeader;

      return {
        id: colId,
        header: headerLabel,
        isDescriptor,
      };
    });

    const rowIndentMap = new Map<string, number>();
    const rowSummaryMap = new Map<string, boolean>();
    for (const [rid, r] of rowSpecById.entries()) {
      rowIndentMap.set(rid, r.indent ?? 0);
      rowSummaryMap.set(rid, r.style === "subtotal" || r.style === "total");
    }

    const getCellText = (rowId: string, col: RenderColumn): string => {
      if (col.id === null) {
        return "";
      }
      const cellInfo = this.#getCell(rowId, col.id);
      const isSummary = rowSummaryMap.get(rowId) ?? false;

      let valStr = cellInfo !== undefined ? (cellInfo.formattedValue ?? "") : "";
      if (!valStr && cellInfo !== undefined) {
        const rawVal = unboxCellValue(cellInfo.value);
        valStr = rawVal !== null ? String(rawVal) : "";
      }

      if (col.isDescriptor) {
        const indent = rowIndentMap.get(rowId) ?? 0;
        let label: string;
        if (!valStr) {
          label = rowId;
        } else if (showIds && valStr.toLowerCase() !== rowId.toLowerCase()) {
          label = `${valStr} [${rowId}]`;
        } else {
          label = valStr;
        }
        const pad = "  ".repeat(indent);
        if (isSummary && label) {
          return `${pad}**${label}**`;
        }
        return `${pad}${label}`;
      }

      let rawText: string;
      switch (view) {
        case "values":
          rawText = valStr ? valStr : "—";
          break;
        case "formulas":
          if (cellInfo !== undefined && cellInfo.isFormula) {
            rawText = cellInfo.excelFormula || valStr;
          } else {
            rawText = valStr;
          }
          break;
        case "expr":
          if (cellInfo !== undefined && cellInfo.isFormula) {
            rawText = cellInfo.formulaExpr || valStr;
          } else {
            rawText = valStr;
          }
          break;
        case "addresses":
          rawText = cellInfo !== undefined ? (cellInfo.physicalA1 ?? "") : "";
          break;
      }

      if (isSummary && rawText) {
        return `**${rawText}**`;
      }
      return rawText;
    };

    const colWidths: number[] = renderCols.map((col) => Math.max(col.header.length, 4));

    for (const r of selectedRows) {
      if (r.isSectionHeader) {
        continue;
      }
      const rowId = r.id;
      if (!rowId) {
        continue;
      }
      for (let colIdx = 0; colIdx < renderCols.length; colIdx++) {
        const col = renderCols[colIdx];
        if (col === undefined) {
          continue;
        }
        const text = getCellText(rowId, col);
        const currentWidth = colWidths[colIdx] ?? 4;
        if (text.length > currentWidth) {
          colWidths[colIdx] = text.length;
        }
      }
    }

    const outLines: string[] = [];

    const headerCells: string[] = [];
    for (let colIdx = 0; colIdx < renderCols.length; colIdx++) {
      const col = renderCols[colIdx];
      if (col === undefined) {
        continue;
      }
      const w = colWidths[colIdx] ?? 4;
      const padded = col.isDescriptor ? col.header.padEnd(w, " ") : col.header.padStart(w, " ");
      headerCells.push(` ${padded} `);
    }
    outLines.push(`|${headerCells.join("|")}|`);

    const alignCells: string[] = [];
    for (let colIdx = 0; colIdx < renderCols.length; colIdx++) {
      const col = renderCols[colIdx];
      if (col === undefined) {
        continue;
      }
      const w = colWidths[colIdx] ?? 4;
      const dashes = "-".repeat(w - 1);
      alignCells.push(col.isDescriptor ? ` :${dashes} ` : ` ${dashes}: `);
    }
    outLines.push(`|${alignCells.join("|")}|`);

    for (const r of selectedRows) {
      if (r.isSectionHeader) {
        const title = r.sectionTitle ?? "";
        const emptyCols = " |".repeat(renderCols.length - 1);
        outLines.push(`| **${title}** |${emptyCols}`);
        continue;
      }
      const rowId = r.id;
      if (!rowId) {
        continue;
      }
      const rowCells: string[] = [];
      for (let colIdx = 0; colIdx < renderCols.length; colIdx++) {
        const col = renderCols[colIdx];
        if (col === undefined) {
          continue;
        }
        const text = getCellText(rowId, col);
        const w = colWidths[colIdx] ?? 4;
        const padded = col.isDescriptor ? text.padEnd(w, " ") : text.padStart(w, " ");
        rowCells.push(` ${padded} `);
      }
      outLines.push(`|${rowCells.join("|")}|`);
    }

    return `${outLines.join("\n")}\n`;
  }
}
