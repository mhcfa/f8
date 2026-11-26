import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { pathToFileURL } from "node:url"
import { inflateRawSync } from "node:zlib"
import { describe, expect, it } from "vitest"
import {
  type AxisSelector,
  type AxisSlice,
  type BandInput,
  type BandInputItem,
  type BandSpecInput,
  Cell,
  type CellInputValue,
  compile,
  CompiledModel,
  type HeaderFooterInput,
  type LaneInput,
  type LaneInputItem,
  type LaneSpecInput,
  Model,
  type PageOrientation,
  type PaperSize,
  type PrintFitMode,
  type Shade,
  Sheet,
  type SheetLayoutMode,
  type SheetPrintOverrideInput,
  type SheetPrintSpecInput,
  SpecificationError,
  Table,
  type TableRef,
} from "../src/index.js"

function readZipEntryUtf8(bytes: Uint8Array, entryName: string): string {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const decoder = new TextDecoder()
  let eocdOffset = -1
  for (let i = bytes.length - 22; i >= Math.max(0, bytes.length - 65557); i--) {
    if (view.getUint32(i, true) === 0x06054b50) {
      eocdOffset = i
      break
    }
  }
  expect(eocdOffset).toBeGreaterThanOrEqual(0)
  const totalEntries = view.getUint16(eocdOffset + 10, true)
  let cdOffset = view.getUint32(eocdOffset + 16, true)
  for (let i = 0; i < totalEntries; i++) {
    const compressionMethod = view.getUint16(cdOffset + 10, true)
    const compressedSize = view.getUint32(cdOffset + 20, true)
    const fileNameLen = view.getUint16(cdOffset + 28, true)
    const extraLen = view.getUint16(cdOffset + 30, true)
    const commentLen = view.getUint16(cdOffset + 32, true)
    const localHeaderOffset = view.getUint32(cdOffset + 42, true)
    const fileName = decoder.decode(
      bytes.subarray(cdOffset + 46, cdOffset + 46 + fileNameLen)
    )
    if (fileName === entryName) {
      const localNameLen = view.getUint16(localHeaderOffset + 26, true)
      const localExtraLen = view.getUint16(localHeaderOffset + 28, true)
      const dataStart = localHeaderOffset + 30 + localNameLen + localExtraLen
      const rawSlice = bytes.subarray(dataStart, dataStart + compressedSize)
      const inflated =
        compressionMethod === 8 ? inflateRawSync(rawSlice) : rawSlice
      return decoder.decode(inflated)
    }
    cdOffset += 46 + fileNameLen + extraLen + commentLen
  }
  throw new Error(`Missing zip entry ${entryName}`)
}

describe("TypeScript Fluent Builder DSL", () => {
  it("Scenario 1: end-to-end model construction, compilation, and .xlsx save", () => {
    const model = new Model({ name: "ThreeStatement", theme: "corporate_navy" })
    const sheet = model.addSheet("Income Statement", { showGridlines: true })
    const table = sheet.addTable("is_table", "Income Statement", {
      subtitle: "Audited Historicals & Projections",
      currency: "USD",
    })
    table
      .addColumn("line_item", { role: "descriptor", header: "Line Item" })
      .addColumn("2023A", {
        role: "measure",
        header: "2023A",
        format: "currency",
        decimals: 0,
      })
      .addColumn("2024E", {
        role: "measure",
        header: "2024E",
        format: "currency",
        decimals: 0,
      })

    table.addSectionHeader("Revenue & Gross Profit", { note: "Audited" })
    table.addRow("rev", {
      cells: { line_item: "Total Revenue", "2023A": 1000.0 },
      expr: 'is_table["rev", col.prev] * 1.10',
      format: "currency",
    })
    table.addRow("cogs", {
      cells: {
        line_item: new Cell({ value: "Cost of Goods Sold", indent: 1 }),
        "2023A": 400.0,
      },
      expr: 'is_table["cogs", col.prev] * 1.05',
      format: "currency",
    })
    table.addRow("gp", {
      cells: {
        line_item: new Cell({ value: "Gross Profit", style: "subtotal" }),
      },
      expr: 'is_table["rev", @col] - is_table["cogs", @col]',
      style: "subtotal",
      format: "currency",
    })

    const compiled = model.compile()
    expect(compiled).toBeInstanceOf(CompiledModel)
    const summary = compiled.summary()
    expect(summary.modelName).toBe("ThreeStatement")
    expect(summary.totalTables).toBe(1)
    expect(summary.totalCells).toBe(9)

    const rev2024 = compiled.inspectCell("is_table", "rev", "2024E")
    expect(rev2024.evaluatedValue).toEqual({ Number: 1100.0 })
    const cogs2024 = compiled.inspectCell("is_table", "cogs", "2024E")
    expect(cogs2024.evaluatedValue).toEqual({ Number: 420.0 })
    const gp2023 = compiled.inspectCell("is_table", "gp", "2023A")
    expect(gp2023.evaluatedValue).toEqual({ Number: 600.0 })
    const gp2024 = compiled.inspectCell("is_table", "gp", "2024E")
    expect(gp2024.evaluatedValue).toEqual({ Number: 680.0 })

    const tempDir = mkdtempSync(join(tmpdir(), "f8-builder-test-"))
    const target = join(tempDir, "from_model", "out.xlsx")
    try {
      compiled.save(target)
      expect(existsSync(target)).toBe(true)
      const bytes = readFileSync(target)
      expect(bytes[0]).toBe(0x50)
      expect(bytes[1]).toBe(0x4b)
      expect(bytes[2]).toBe(0x03)
      expect(bytes[3]).toBe(0x04)
    } finally {
      rmSync(tempDir, { recursive: true, force: true })
    }
  })

  it("explicit 0D cell indentation via Cell and DetailedCellInput", () => {
    const model = new Model()
    const sheet = model.addSheet("Sheet1")
    const table = sheet.addTable("T1", "Table 1")
    table.addColumn("line_item", { role: "descriptor", header: "Line Item" })
    table.addColumn("v", { role: "measure", header: "Value" })

    table.addRow("r1", {
      cells: {
        line_item: new Cell({ value: "Cost of Goods Sold", indent: 1 }),
        v: 100.0,
      },
    })
    table.addRow("r2", {
      cells: {
        line_item: { value: "Operating Expenses", indent: 2 },
        v: 50.0,
      },
    })

    const { tables } = model.toSpec()
    expect(tables[0]).toEqual({
      id: "T1",
      title: "Table 1",
      collapsible: false,
      columns: [
        {
          kind: "data",
          id: "line_item",
          role: "descriptor",
          headers: ["Line Item"],
        },
        {
          kind: "data",
          id: "v",
          role: "measure",
          headers: ["Value"],
        },
      ],
      rows: [
        {
          kind: "data",
          id: "r1",
          cells: {
            line_item: { value: "Cost of Goods Sold", indent: 1 },
            v: 100.0,
          },
        },
        {
          kind: "data",
          id: "r2",
          cells: {
            line_item: { value: "Operating Expenses", indent: 2 },
            v: 50.0,
          },
        },
      ],
    })

    const compiled = model.compile()
    expect(compiled.summary().totalCells).toBe(4)
  })

  it("symmetrical column and row spacing gutters", () => {
    const model = new Model()
    const sheet = model.addSheet("Sheet1")
    const table = sheet.addTable("T1", "Table 1")

    table.addColumn("c1", { header: "Col 1" })
    table.addColumnGutter()
    table.addColumn("c2", { header: "Col 2" })

    table.addRow("r1", { cells: { c1: 10.0, c2: 20.0 } })
    table.addRowGutter()
    table.addRow("r2", { cells: { c1: 30.0, c2: 40.0 } })

    const { tables } = model.toSpec()
    expect(tables[0]).toEqual({
      id: "T1",
      title: "Table 1",
      collapsible: false,
      columns: [
        { kind: "data", id: "c1", role: "measure", headers: ["Col 1"] },
        { kind: "gutter" },
        { kind: "data", id: "c2", role: "measure", headers: ["Col 2"] },
      ],
      rows: [
        { kind: "data", id: "r1", cells: { c1: 10.0, c2: 20.0 } },
        { kind: "gutter" },
        { kind: "data", id: "r2", cells: { c1: 30.0, c2: 40.0 } },
      ],
    })
  })

  it("point cell override via setCell", () => {
    const model = new Model()
    const sheet = model.addSheet("Sheet1")
    const table = sheet.addTable("T1", "Table 1")
    table.addColumn("c1", { header: "Col 1" })
    table.addColumn("c2", { header: "Col 2" })
    table.addColumn("c3", { header: "Col 3" })

    table.addRow("rev", { cells: { c1: 100.0 }, expr: "PREV * 1.05" })
    table.addRow("existing", {
      cells: { c1: new Cell({ value: 50.0, format: "currency" }) },
    })

    // Point override on row with vector formula
    table.setCell("rev", "c1", { value: 150.0, note: "Audited", indent: 0 })
    // Updating an existing cell preserving previous value and format
    table.setCell("existing", "c1", { note: "Updated note" })
    // Passing pre-constructed Cell
    table.setCell("rev", "c2", new Cell({ expr: "PREV * 1.10" }))
    // Positional null value override
    table.setCell("rev", "c3", null)

    const { tables } = model.toSpec()
    expect(tables[0]).toEqual({
      id: "T1",
      title: "Table 1",
      collapsible: false,
      columns: [
        { kind: "data", id: "c1", role: "measure", headers: ["Col 1"] },
        { kind: "data", id: "c2", role: "measure", headers: ["Col 2"] },
        { kind: "data", id: "c3", role: "measure", headers: ["Col 3"] },
      ],
      rows: [
        {
          kind: "data",
          id: "rev",
          expr: "PREV * 1.05",
          cells: {
            c1: { value: 150.0, indent: 0, note: "Audited" },
            c2: { expr: "PREV * 1.10" },
            c3: null,
          },
        },
        {
          kind: "data",
          id: "existing",
          cells: {
            c1: { value: 50.0, format: "currency", note: "Updated note" },
          },
        },
      ],
    })

    // Non-existent row or column throws Error
    expect(() => table.setCell("missing", "c1", 10.0)).toThrow(
      "Row 'missing' does not exist"
    )
    expect(() => table.setCell("rev", "missing", 10.0)).toThrow(
      "Column 'missing' does not exist"
    )

    // Creating a new cell without value or expr throws Error
    expect(() => table.setCell("existing", "c2", { note: "Invalid" })).toThrow(
      "either 'value' or 'expr'"
    )

    // Specifying both value and expr in DetailedCellInput throws Error
    expect(() =>
      table.setCell("existing", "c2", { value: 10.0, expr: "10.0" })
    ).toThrow("cannot specify both 'value' and 'expr'")
  })

  it("header specification validation", () => {
    const table = new Table("T1", "Table 1")
    expect(() =>
      table.addColumn("rev", {
        header: "Revenue",
        headers: ["Financials", "Revenue"],
      })
    ).toThrow("both 'header' and 'headers'")

    for (const invalidGroup of ["", "   "]) {
      expect(() =>
        table.addColumn("rev", { group: invalidGroup, header: "Revenue" })
      ).toThrow("Column 'group' cannot be empty")
    }

    table.addColumn("mrq", {
      group: "Historical",
      headers: ["MRQ", "2012-03-31"],
    })
    table.addColumn("standalone", {
      headers: ["Standalone", "at Close", "2012-12-31"],
    })
    const spec = table.toSpec()
    expect(spec.columns).toEqual([
      {
        kind: "data",
        id: "mrq",
        role: "measure",
        group: "Historical",
        headers: ["MRQ", "2012-03-31"],
      },
      {
        kind: "data",
        id: "standalone",
        role: "measure",
        headers: ["Standalone", "at Close", "2012-12-31"],
      },
    ])
  })

  it("duplicate identifier validation", () => {
    const model = new Model()
    const sheet = model.addSheet("Sheet1")
    expect(() => model.addSheet("Sheet1")).toThrow(
      "Sheet 'Sheet1' already exists"
    )

    const table = sheet.addTable("T1", "Table 1")
    expect(() => sheet.addTable("T1", "Duplicate Table")).toThrow(
      "Table 'T1' already exists"
    )

    const sheet2 = model.addSheet("Sheet2")
    expect(() => sheet2.addTable("T1", "Cross-sheet duplicate")).toThrow(
      "Table 'T1' already exists in model"
    )

    const standaloneSheet = new Sheet("Standalone")
    standaloneSheet.addTable("ST1", "Standalone 1")
    expect(() =>
      standaloneSheet.addTable("ST1", "Duplicate Standalone")
    ).toThrow("Table 'ST1' already exists in sheet 'Standalone'")

    table.addColumn("c1", { header: "Col 1" })
    expect(() => table.addColumn("c1", { header: "Col 1 duplicate" })).toThrow(
      "Column 'c1' already exists"
    )

    table.addRow("r1", { cells: { c1: 1.0 } })
    expect(() => table.addRow("r1", { cells: { c1: 2.0 } })).toThrow(
      "Row 'r1' already exists"
    )
  })

  it("global and sheet-scoped lookups", () => {
    const model = new Model()
    const s1 = model.addSheet("Financials")
    const s2 = model.addSheet("Operations")
    const t1 = s1.addTable("is_table", "Income Statement")
    const t2 = s2.addTable("headcount", "Headcount")

    expect(model.getSheet("Financials")).toBe(s1)
    expect(model.getSheet("Operations")).toBe(s2)
    expect(model.getTable("is_table")).toBe(t1)
    expect(model.getTable("headcount")).toBe(t2)
    expect(s1.getTable("is_table")).toBe(t1)
    expect(s2.getTable("headcount")).toBe(t2)
    expect(model.sheets).toEqual([s1, s2])
    expect(model.tables).toEqual([t1, t2])
    expect(s1.tables).toEqual([t1])

    expect(() => model.getSheet("unknown")).toThrow("Sheet 'unknown' not found")
    expect(() => model.getTable("unknown")).toThrow("Table 'unknown' not found")
    expect(() => s1.getTable("headcount")).toThrow(
      "Table 'headcount' not found in sheet 'Financials'"
    )
  })

  it("cell payload validation and explicit nulls", () => {
    const c1 = new Cell({ value: 100.0 })
    expect(c1.value).toBe(100.0)
    expect(c1.expr).toBeNull()

    const c2 = new Cell({ expr: "PREV * 1.05" })
    expect(c2.expr).toBe("PREV * 1.05")
    expect(c2.value).toBeUndefined()

    const c3 = new Cell({ value: null })
    expect(c3.value).toBeNull()
    expect(c3.expr).toBeNull()

    const c4 = new Cell({
      value: 42.0,
      style: "total",
      highlight: true,
      shade: "muted",
      format: "currency",
      decimals: 2,
      currency: "USD",
      indent: 1,
      note: "Source: audit",
    })
    expect(c4.value).toBe(42.0)
    expect(c4.style).toBe("total")
    expect(c4.highlight).toBe(true)
    expect(c4.shade).toBe("muted")
    expect(c4.format).toBe("currency")
    expect(c4.decimals).toBe(2)
    expect(c4.currency).toBe("USD")
    expect(c4.indent).toBe(1)
    expect(c4.note).toBe("Source: audit")

    expect(() => new Cell({ value: 100.0, expr: "A + B" })).toThrow("both")
    expect(() => new Cell({})).toThrow("either")
  })

  it("worksheet freezePanes split coordinates and validation", () => {
    const model = new Model()
    const sheet = model.addSheet("Sheet1", {
      freezePanes: { row: 4, col: 2 },
      tabColor: "#1B365D",
    })
    const table = sheet.addTable("T1", "Table 1")
    table.addColumn("c1", { header: "Col 1" })
    table.addRow("r1", { cells: { c1: 10.0 } })

    const { manifest } = model.toSpec()
    expect(manifest).toEqual({
      modelName: "Model",
      theme: "corporate_navy",
      sheets: [
        {
          name: "Sheet1",
          showGridlines: false,
          tabColor: "#1B365D",
          freezePanes: { row: 4, col: 2 },
          bands: [{ lanes: [{ tables: ["T1"] }] }],
        },
      ],
    })

    expect(() =>
      model.addSheet("InvalidSheet", { freezePanes: { row: -1, col: 2 } })
    ).toThrow("freezePanes")
    expect(
      () => new Sheet("DirectInvalid", { freezePanes: { row: 2, col: -1 } })
    ).toThrow("freezePanes")
  })

  it("normalizes currency codes to lowercase and unwraps pure Cell values in toSpec()", () => {
    const model = new Model()
    const sheet = model.addSheet("Sheet1")
    const table = sheet.addTable("T1", "Table 1", { currency: "USD" })
    table.addColumn("c1", { header: "Col 1", currency: "EUR" })
    table.addColumn("c2", { header: "Col 2" })
    table.addColumn("c3", { header: "Col 3" })
    table.addRow("r1", {
      cells: {
        c1: new Cell({ value: 100.0, currency: "GBP" }),
        c2: { value: 50.0, currency: "CAD" },
        c3: new Cell({ value: 25.0 }),
      },
      currency: "JPY",
    })

    const { tables } = model.toSpec()
    expect(tables[0]).toEqual({
      id: "T1",
      title: "Table 1",
      currency: "usd",
      collapsible: false,
      columns: [
        {
          kind: "data",
          id: "c1",
          role: "measure",
          headers: ["Col 1"],
          currency: "eur",
        },
        {
          kind: "data",
          id: "c2",
          role: "measure",
          headers: ["Col 2"],
        },
        {
          kind: "data",
          id: "c3",
          role: "measure",
          headers: ["Col 3"],
        },
      ],
      rows: [
        {
          kind: "data",
          id: "r1",
          currency: "jpy",
          cells: {
            c1: { value: 100.0, currency: "gbp" },
            c2: { value: 50.0, currency: "cad" },
            c3: 25.0,
          },
        },
      ],
    })
  })

  it("custom theme dictionary and URL resolution on Model initialization", () => {
    const customTheme: Readonly<Record<string, unknown>> = {
      name: "firm_custom",
      typography: {
        font_family: "Calibri",
        title_size: 16.0,
        subtitle_size: 10.0,
        super_header_size: 11.0,
        col_header_size: 11.0,
        section_header_size: 11.0,
        data_size: 11.0,
      },
      metrics: {
        gutter_col_width: 4.0,
        char_width_multiplier: 1.2,
        col_padding: 2.5,
        min_col_width: 14.0,
      },
      palette: {
        table_title_fill: "#2B4C7E",
        table_title_text: "#FFFFFF",
        table_subtitle_text: "#555555",
        super_header_fill: "#3B5C8E",
        super_header_text: "#FFFFFF",
        col_header_fill: "#EBF1F5",
        col_header_text: "#2B4C7E",
        section_header_fill: "#F5F8FA",
        section_header_text: "#2B4C7E",
        data_fill: "#FFFFFF",
        data_text: "#222222",
        subtotal_fill: "#F5F8FA",
        subtotal_text: "#2B4C7E",
        total_fill: "#EBF1F5",
        total_text: "#2B4C7E",
        input_text: "#002060",
        formula_text: "#000000",
        shade_muted_fill: "#F8FAFC",
        shade_alt_fill: "#F1F5F9",
        highlight_fill: "#FEF3C7",
        highlight_text: "#92400E",
        gridline_color: "#E5E7EB",
        border_color: "#D1D5DB",
        total_border_color: "#2B4C7E",
      },
      borders: {
        table_title_border_bottom: "none",
        super_header_border_bottom: "thin",
        header_border_bottom: "thin",
        descriptor_border_right: "none",
        subtotal_border_top: "thin",
        total_border_top: "thin",
        total_border_bottom: "double",
        col_subtotal_border_left: "thin",
        col_total_border_left: "thin",
        col_total_border_right: "double",
      },
      number_formats: {
        currency: '{sym}#,##0{dec}_);({sym}#,##0{dec});"-"_)',
        number: '#,##0{dec}_);(#,##0{dec});"-"_)',
        percentage: "0{dec}%",
        multiple: '0{dec}"x"',
        basis_points: '#,##0{dec}" bps"',
        date: "yyyy-mm-dd",
        text: "@",
        defaults: {
          currency_decimals: 2,
          number_decimals: 2,
          percentage_decimals: 2,
          multiple_decimals: 2,
          basis_points_decimals: 0,
        },
      },
    }

    const model = new Model({ name: "CustomThemeModel", theme: customTheme })
    const sheet = model.addSheet("Sheet1")
    const table = sheet.addTable("T1", "Theme Table")
    table.addColumn("c1", { header: "C1", format: "currency" })
    table.addRow("r1", { cells: { c1: 100.0 } })

    const { manifest } = model.toSpec()
    expect(manifest.theme).toEqual(customTheme)

    const compiled = model.compile()
    expect(compiled.summary().themeName).toBe("firm_custom")
    expect(compiled.formatted("T1", "r1", "c1")).toBe("$100.00")

    // Theme from file URL
    const tempDir = mkdtempSync(join(tmpdir(), "f8-theme-url-"))
    try {
      const themePath = join(tempDir, "theme.json")
      writeFileSync(themePath, JSON.stringify(customTheme), "utf-8")

      const urlModel = new Model({
        name: "UrlThemeModel",
        theme: pathToFileURL(themePath),
      })
      expect(urlModel.theme).toEqual(customTheme)

      const invalidThemePath = join(tempDir, "invalid_theme.json")
      writeFileSync(invalidThemePath, "[1, 2, 3]", "utf-8")
      expect(
        () =>
          new Model({
            name: "InvalidTheme",
            theme: pathToFileURL(invalidThemePath),
          })
      ).toThrow("must contain a JSON object at root")
    } finally {
      rmSync(tempDir, { recursive: true, force: true })
    }
  })

  it("semantic background shade DSL and conflict detection", () => {
    const model = new Model()
    const sheet = model.addSheet("Sheet1")
    const table = sheet.addTable("T1", "Shaded Table")
    table.addColumn("label", { role: "descriptor", header: "Item" })
    table.addColumn("2023a", { header: "2023A", shade: "muted" })
    table.addColumn("stub", { header: "Stub", shade: "alt" })
    table.addColumn("2024e", { header: "2024E" })

    table.addRow("rev", {
      cells: { label: "Revenue", "2023a": 100.0, stub: 25.0, "2024e": 120.0 },
    })
    table.addRow("kpi", {
      shade: "alt",
      cells: {
        label: "KPI Band",
        "2023a": new Cell({ value: 10.0, shade: "none" }),
        stub: { value: 12.0, shade: "muted" },
        "2024e": 15.0,
      },
    })
    table.setCell("rev", "2024e", { shade: "muted" })

    const { tables } = model.toSpec()
    expect(tables[0]).toEqual({
      id: "T1",
      title: "Shaded Table",
      collapsible: false,
      columns: [
        { kind: "data", id: "label", role: "descriptor", headers: ["Item"] },
        {
          kind: "data",
          id: "2023a",
          role: "measure",
          headers: ["2023A"],
          shade: "muted",
        },
        {
          kind: "data",
          id: "stub",
          role: "measure",
          headers: ["Stub"],
          shade: "alt",
        },
        { kind: "data", id: "2024e", role: "measure", headers: ["2024E"] },
      ],
      rows: [
        {
          kind: "data",
          id: "rev",
          cells: {
            label: "Revenue",
            "2023a": 100.0,
            stub: 25.0,
            "2024e": { value: 120.0, shade: "muted" },
          },
        },
        {
          kind: "data",
          id: "kpi",
          shade: "alt",
          cells: {
            label: "KPI Band",
            "2023a": { value: 10.0, shade: "none" },
            stub: { value: 12.0, shade: "muted" },
            "2024e": 15.0,
          },
        },
      ],
    })

    const compiled = model.compile()
    const tableInsp = compiled.inspectTable("T1")
    expect(tableInsp.columns[0]?.shade).toBeNull()
    expect(tableInsp.columns[1]?.shade).toBe("muted")
    expect(tableInsp.columns[2]?.shade).toBe("alt")
    expect(tableInsp.rows[0]?.shade).toBeNull()
    expect(tableInsp.rows[1]?.shade).toBe("alt")

    expect(compiled.inspectCell("T1", "rev", "2023a").shade).toBe("muted")
    expect(compiled.inspectCell("T1", "rev", "stub").shade).toBe("alt")
    expect(compiled.inspectCell("T1", "rev", "2024e").shade).toBe("muted")
    expect(compiled.inspectCell("T1", "kpi", "label").shade).toBe("alt")
    expect(compiled.inspectCell("T1", "kpi", "2023a").shade).toBeNull()
    expect(compiled.inspectCell("T1", "kpi", "stub").shade).toBe("muted")
    expect(compiled.inspectCell("T1", "kpi", "2024e").shade).toBe("alt")

    // Symmetrical vector shade conflict raises SpecificationError
    const conflictModel = new Model()
    const cs = conflictModel.addSheet("S")
    const ct = cs.addTable("Conflict", "Conflict")
    const shadeVal: Shade = "alt"
    ct.addColumn("c1", { header: "C1", shade: shadeVal })
    ct.addRow("r1", { shade: "muted", cells: { c1: 1.0 } })
    try {
      conflictModel.compile()
      expect.unreachable(
        "Should have thrown SpecificationError on shade conflict"
      )
    } catch (err) {
      expect(err).toBeInstanceOf(SpecificationError)
      expect((err as SpecificationError).table).toBe("Conflict")
    }
  })

  it("Scenario 2: Band/Lane DSL, validation, and layout scenarios (Scenarios 1-6 & 8)", () => {
    const makeCard = (
      sheet: Sheet,
      tid: string,
      title: string,
      cols: number,
      rows: number
    ): Table => {
      const t = sheet.addTable(tid, title)
      for (let c = 1; c <= cols; c++) {
        t.addColumn(`c${c}`, { header: `C${c}` })
      }
      for (let r = 1; r <= rows; r++) {
        const cells: Record<string, number> = {}
        for (let c = 1; c <= cols; c++) {
          cells[`c${c}`] = r * 10 + c
        }
        t.addRow(`r${r}`, { cells })
      }
      return t
    }

    // Scenario 1: Default vertical stacking without setBands
    const mDefault = new Model({ name: "DefaultStack" })
    const sDefault = mDefault.addSheet("Main")
    makeCard(sDefault, "T1", "First", 2, 2)
    makeCard(sDefault, "T2", "Second", 2, 1)
    const defaultBands: readonly BandSpecInput[] = sDefault.bands
    expect(defaultBands).toEqual([
      { lanes: [{ tables: ["T1"] }] },
      { lanes: [{ tables: ["T2"] }] },
    ])
    const { manifest: defManifest, tables: defTables } = mDefault.toSpec()
    expect(defManifest.sheets[0]?.bands).toEqual(defaultBands)
    expect("placement" in (defTables[0] ?? {})).toBe(false)
    const cDefault = mDefault.compile()
    expect(cDefault.table("T1").address("r1", "c1")).toBe("B5")
    expect(cDefault.table("T2").address("r1", "c1")).toBe("B12")
    cDefault.dispose()

    // Scenario 2: Wider lower-left card in multi-lane band (auto mode)
    const mWide = new Model({ name: "WiderLowerLeft" })
    const sWide = mWide.addSheet("Dashboard")
    const tl = makeCard(sWide, "TL", "Top Left", 3, 2)
    const bl = makeCard(sWide, "BL", "Bottom Left", 5, 3)
    const tr = makeCard(sWide, "TR", "Top Right", 2, 1)
    const br = makeCard(sWide, "BR", "Bottom Right", 2, 4)
    const bot = makeCard(sWide, "Bottom", "Band 2", 4, 1)
    const leftLane: LaneInput = { tables: [tl, bl] }
    const rightLane: LaneInput = { tables: [tr, br] }
    const topBand: BandInput = { lanes: [leftLane, rightLane] }
    const botItem: BandInputItem = bot
    expect(sWide.setBands([topBand, botItem])).toBe(sWide)
    const cWide = mWide.compile()
    expect(cWide.table("TL").address("r1", "c1")).toBe("B5")
    expect(cWide.table("BL").address("r1", "c5")).toBe("F12")
    expect(cWide.table("TR").address("r1", "c1")).toBe("H5")
    expect(cWide.table("BR").address("r1", "c1")).toBe("H11")
    expect(cWide.table("Bottom").address("r1", "c1")).toBe("B20")
    cWide.dispose()

    // Scenario 3: Custom container gaps (including 0), single-lane multi-table bands, bare TableRef shorthand
    const mGaps = new Model({ name: "CustomGaps" })
    const sGaps = mGaps.addSheet("Gaps")
    const gA = makeCard(sGaps, "A", "A", 2, 1)
    const gB = makeCard(sGaps, "B", "B", 1, 1)
    const gC = makeCard(sGaps, "C", "C", 1, 1)
    const gD = makeCard(sGaps, "D", "D", 1, 1)
    const gE = makeCard(sGaps, "E", "E", 1, 1)
    const gF = makeCard(sGaps, "F", "F", 1, 1)
    const gG = makeCard(sGaps, "G", "G", 2, 1)
    const gRef: TableRef = gG.id
    sGaps.setBands([
      {
        laneGap: 0,
        lanes: [
          { tables: [gA, gB], tableGap: 1 },
          { tables: [gC, gD], tableGap: 4 },
        ],
      },
      { gap: 0, lanes: [{ tables: [gE, gF], tableGap: 1 }] },
      gRef,
    ])
    const { manifest: gapManifest } = mGaps.toSpec()
    expect(gapManifest.sheets[0]?.bands).toEqual([
      {
        laneGap: 0,
        lanes: [
          { tables: ["A", "B"], tableGap: 1 },
          { tables: ["C", "D"], tableGap: 4 },
        ],
      },
      { gap: 0, lanes: [{ tables: ["E", "F"], tableGap: 1 }] },
      { lanes: [{ tables: ["G"] }] },
    ])
    const cGaps = mGaps.compile()
    expect(cGaps.table("C").address("r1", "c1")).toBe("D5")
    expect(cGaps.table("B").address("r1", "c1")).toBe("B10")
    expect(cGaps.table("D").address("r1", "c1")).toBe("D13")
    expect(cGaps.table("E").address("r1", "c1")).toBe("B17")
    expect(cGaps.table("F").address("r1", "c1")).toBe("B22")
    expect(cGaps.table("G").address("r1", "c1")).toBe("B28")
    cGaps.dispose()

    // Scenario 4: layout="grid" descriptor-span growth and per-lane descriptor alignment
    const mGrid = new Model({ name: "GridScenario4" })
    const sGrid = mGrid.addSheet("Summary", { layout: "grid", colWidth: 12 })
    const gtl = sGrid.addTable("TL", "Top Left")
    gtl.addColumn("item", { role: "descriptor", header: "Item" })
    gtl.addColumn("v1", { header: "V1" })
    gtl.addRow("r1", { cells: { item: "Short", v1: 10.0 } })

    const gbl = sGrid.addTable("BL", "Bottom Left")
    gbl.addColumn("item", { role: "descriptor", header: "Item" })
    gbl.addColumn("v1", { header: "V1" })
    gbl.addColumn("v2", { header: "V2" })
    gbl.addRow("r1", {
      cells: {
        item: "Consolidated Adjusted EBITDA ($)",
        v1: 100.0,
        v2: 120.0,
      },
    })

    const gtr = sGrid.addTable("TR", "Top Right")
    gtr.addColumn("k", { role: "descriptor", header: "Key" })
    gtr.addColumn("v", { header: "Val" })
    gtr.addRow("r1", { cells: { k: "IRR", v: 0.25 } })

    const gbr = sGrid.addTable("BR", "Bottom Right")
    gbr.addColumn("k", { role: "descriptor", header: "Key" })
    gbr.addColumn("v", { header: "Val" })
    gbr.addRow("r1", { cells: { k: "Sponsor Gross Multiple", v: 2.8 } })

    const gtime = sGrid.addTable("Timeline", "Timeline")
    gtime.addColumn("item", { role: "descriptor", header: "Item" })
    gtime.addColumn("y1", { header: "Y1" })
    gtime.addRow("r1", { cells: { item: "Revenue", y1: 50.0 } })

    sGrid.setBands([
      { lanes: [{ tables: [gtl, gbl] }, { tables: [gtr, gbr] }], laneGap: 2 },
      gtime,
    ])
    const cGrid = mGrid.compile()
    expect(cGrid.table("TL").address("r1", "v1")).toBe("E5")
    expect(cGrid.table("BL").address("r1", "v1")).toBe("E11")
    expect(cGrid.table("Timeline").address("r1", "y1")).toBe("E17")
    expect(cGrid.table("TR").address("r1", "k")).toBe("I5")
    expect(cGrid.table("TR").address("r1", "v")).toBe("K5")
    expect(cGrid.table("BR").address("r1", "k")).toBe("I11")
    expect(cGrid.table("BR").address("r1", "v")).toBe("K11")
    cGrid.dispose()

    // Scenario 5: Band/Lane reordering decoupled from declaration order
    const mOrder = new Model({ name: "ReorderModel" })
    const sOrder = mOrder.addSheet("Dash")
    const ot1 = sOrder.addTable("T1", "T1")
    const ot2 = sOrder.addTable("T2", "T2")
    const ot3 = sOrder.addTable("T3", "T3")
    for (const ot of [ot1, ot2, ot3]) {
      ot.addColumn("c1", { header: "C1" })
      ot.addRow("r1", { expr: "1 / 0" })
    }
    sOrder.setBands([{ lanes: [{ tables: [ot3, ot1] }, { tables: [ot2] }] }])
    expect(sOrder.tables.map((t) => t.id)).toEqual(["T1", "T2", "T3"])
    const cOrder = mOrder.compile()
    expect(cOrder.summary().sheets[0]?.tables.map((t) => t.id)).toEqual([
      "T3",
      "T1",
      "T2",
    ])
    expect(cOrder.errors().map((e) => e.coordinate.table)).toEqual([
      "T3",
      "T1",
      "T2",
    ])
    expect(cOrder.inspectSheet("Dash").tables).toEqual(["T3", "T1", "T2"])
    expect(cOrder.table("T3").address("r1", "c1")).toBe("B5")
    expect(cOrder.table("T1").address("r1", "c1")).toBe("B11")
    expect(cOrder.table("T2").address("r1", "c1")).toBe("D5")
    const mdOrder = cOrder.toMarkdown("layout")
    expect(mdOrder.indexOf("`T3`")).toBeLessThan(mdOrder.indexOf("`T1`"))
    expect(mdOrder.indexOf("`T1`")).toBeLessThan(mdOrder.indexOf("`T2`"))
    cOrder.dispose()

    // Scenario 6: Band/lane-aware title banner alignment
    const mBanner = new Model({ name: "BannerScenario6" })
    const sB1 = mBanner.addSheet("Dash")
    const btl = makeCard(sB1, "TL", "Top Left", 3, 4)
    const bbl = makeCard(sB1, "BL", "Bottom Left", 4, 7)
    const btr = makeCard(sB1, "TR", "Top Right", 3, 3)
    const bwb = makeCard(sB1, "WideBottom", "Wide Bottom", 10, 2)
    sB1.setBands([{ lanes: [{ tables: [btl, bbl] }, { tables: [btr] }] }, bwb])
    const cBanner = mBanner.compile()
    const inspB1 = cBanner.inspectSheet("Dash")
    const row2 = inspB1.rows.find((r) => r.excelRow === 2)
    expect(row2?.cells.map((c) => [c.physicalA1, c.kind, c.tableId])).toEqual([
      ["B2", "table_title", "TL"],
      ["C2", "table_title", "TL"],
      ["D2", "table_title", "TL"],
      ["E2", "table_title", "TL"],
      ["F2", "empty", null],
      ["G2", "table_title", "TR"],
      ["H2", "table_title", "TR"],
      ["I2", "table_title", "TR"],
      ["J2", "table_title", "TR"],
      ["K2", "table_title", "TR"],
    ])
    const row11 = inspB1.rows.find((r) => r.excelRow === 11)
    expect(
      row11?.cells
        .filter((c) => c.kind === "table_title")
        .map((c) => [c.physicalA1, c.kind, c.tableId])
    ).toEqual([
      ["B11", "table_title", "BL"],
      ["C11", "table_title", "BL"],
      ["D11", "table_title", "BL"],
      ["E11", "table_title", "BL"],
    ])
    cBanner.dispose()

    // Scenario 8: Outer SDK strict validation, empty sheets, idempotency, and sheet.bands parity
    const sVal = new Sheet("Val")
    const vt1 = sVal.addTable("T1", "T1")
    const vt2 = sVal.addTable("T2", "T2")

    // Bare non-array arguments
    expect(() =>
      sVal.setBands(vt1 as unknown as readonly BandInputItem[])
    ).toThrow("array")
    expect(() =>
      sVal.setBands("T1" as unknown as readonly BandInputItem[])
    ).toThrow("array")

    // Invalid Band / Lane shapes and gaps
    expect(() => sVal.setBands([{ lanes: [] }])).toThrow("cannot be empty")
    expect(() =>
      sVal.setBands([{ lanes: [[vt1, vt2] as unknown as LaneInputItem] }])
    ).toThrow("LaneInput")
    expect(() =>
      sVal.setBands([{ lanes: [123 as unknown as LaneInputItem] }])
    ).toThrow("LaneInput")
    expect(() =>
      sVal.setBands([{ lanes: [{} as unknown as LaneInputItem] }])
    ).toThrow("LaneInput")
    expect(() => sVal.setBands([{ lanes: [{ tables: [] }] }])).toThrow(
      "cannot be empty"
    )
    expect(() =>
      sVal.setBands([
        { lanes: [{ tables: "T1" as unknown as readonly TableRef[] }] },
      ])
    ).toThrow("tables")
    expect(() =>
      sVal.setBands([{ lanes: [{ tables: [vt1], tableGap: 1 }] }, vt2])
    ).toThrow("single table cannot specify 'tableGap'")
    expect(() =>
      sVal.setBands([{ lanes: [{ tables: [vt1], tableGap: 0 }] }, vt2])
    ).toThrow("single table cannot specify 'tableGap'")
    expect(() =>
      sVal.setBands([{ lanes: [{ tables: [vt1] }], laneGap: 1 }, vt2])
    ).toThrow("single lane and cannot specify 'laneGap'")
    expect(() => sVal.setBands([{ lanes: [vt1], laneGap: 1 }, vt2])).toThrow(
      "single lane and cannot specify 'laneGap'"
    )
    expect(() =>
      sVal.setBands([{ lanes: [{ tables: [vt1] }], laneGap: 0 }, vt2])
    ).toThrow("single lane and cannot specify 'laneGap'")
    expect(() => sVal.setBands([{ lanes: [vt1], laneGap: 0 }, vt2])).toThrow(
      "single lane and cannot specify 'laneGap'"
    )
    expect(() =>
      sVal.setBands([{ lanes: [{ tables: [vt1, vt2], tableGap: -1 }] }])
    ).toThrow("non-negative integer")
    expect(() =>
      sVal.setBands([{ lanes: [{ tables: [vt1, vt2], tableGap: 1.5 }] }])
    ).toThrow("non-negative integer")
    expect(() =>
      sVal.setBands([
        {
          lanes: [{ tables: [vt1, vt2], tableGap: true as unknown as number }],
        },
      ])
    ).toThrow("non-negative integer")
    expect(() =>
      sVal.setBands([
        { lanes: [{ tables: [vt1] }, { tables: [vt2] }], laneGap: -1 },
      ])
    ).toThrow("non-negative integer")
    expect(() =>
      sVal.setBands([vt1, { lanes: [{ tables: [vt2] }], gap: -1 }])
    ).toThrow("non-negative integer")
    expect(() =>
      sVal.setBands([
        vt1,
        { lanes: [{ tables: [vt2] }], gap: false as unknown as number },
      ])
    ).toThrow("non-negative integer")

    // Band 0 with gap
    expect(() =>
      sVal.setBands([{ lanes: [{ tables: [vt1] }], gap: 0 }, vt2])
    ).toThrow("band 0 cannot specify 'gap'")
    expect(() => sVal.setBands([{ lanes: [vt1], gap: 0 }, vt2])).toThrow(
      "band 0 cannot specify 'gap'"
    )
    expect(() =>
      sVal.setBands([{ lanes: [{ tables: [vt1] }], gap: 2 }, vt2])
    ).toThrow("band 0 cannot specify 'gap'")

    // Partition errors: missing, empty on non-empty, duplicates, unknown, foreign-sheet
    expect(() => sVal.setBands([vt1])).toThrow("missing table")
    expect(() => sVal.setBands([])).toThrow("missing table")
    expect(() =>
      sVal.setBands([{ lanes: [{ tables: [vt1, vt1] }, { tables: [vt2] }] }])
    ).toThrow("Duplicate table")
    expect(() => sVal.setBands([{ lanes: [vt1, vt1] }])).toThrow(
      "Duplicate table"
    )
    expect(() => sVal.setBands([vt1, vt2, vt1])).toThrow("Duplicate table")
    expect(() => sVal.setBands([vt1, vt2, "Ghost"])).toThrow(
      "does not belong to sheet"
    )
    expect(() => sVal.setBands([{ lanes: [vt1, "Ghost"] }])).toThrow(
      "does not belong to sheet"
    )

    const foreignSheet = new Sheet("Other")
    const foreignDiff = foreignSheet.addTable("Foreign", "Foreign")
    const foreignSameId = foreignSheet.addTable("T1", "Foreign T1")
    expect(() => sVal.setBands([vt1, foreignDiff])).toThrow(
      "does not belong to sheet"
    )
    expect(() => sVal.setBands([foreignSameId, vt2])).toThrow(
      "does not belong to sheet"
    )
    expect(() => sVal.setBands([{ lanes: [foreignSameId, vt2] }])).toThrow(
      "does not belong to sheet"
    )

    // Normalized sheet.bands parity (Table vs string vs band/lane shorthand vs omitted) and idempotency
    const expectedStacked: readonly BandSpecInput[] = [
      { lanes: [{ tables: ["T1"] }] },
      { lanes: [{ tables: ["T2"] }] },
    ]
    const expectedLane: LaneSpecInput = { tables: ["T2", "T1"] }
    expect(sVal.bands).toEqual(expectedStacked)
    sVal.setBands([vt1, vt2])
    expect(sVal.bands).toEqual(expectedStacked)
    sVal.setBands([{ lanes: [{ tables: ["T1"] }, { tables: ["T2"] }] }])
    expect(sVal.bands).toEqual([
      { lanes: [{ tables: ["T1"] }, { tables: ["T2"] }] },
    ])
    const lanePairItem: LaneInputItem = vt1
    sVal.setBands([{ lanes: [lanePairItem, "T2"] }])
    expect(sVal.bands).toEqual([
      { lanes: [{ tables: ["T1"] }, { tables: ["T2"] }] },
    ])
    // Idempotent replacement
    sVal.setBands([{ lanes: [{ tables: [vt2, vt1] }] }])
    expect(sVal.bands).toEqual([{ lanes: [expectedLane] }])

    // Bare TableRef lane shorthand end-to-end compilation (side-by-side pair, mixed lane, custom gap)
    const mShort = new Model({ name: "LaneShorthand" })
    const sShort = mShort.addSheet("Sensitivities")
    const sL1 = makeCard(sShort, "L1", "Left 1", 2, 1)
    const sL2 = makeCard(sShort, "L2", "Left 2", 2, 1)
    const sR1 = makeCard(sShort, "R1", "Right 1", 2, 1)
    const sP1 = makeCard(sShort, "P1", "Pair Left", 2, 1)
    makeCard(sShort, "P2", "Pair Right", 2, 1)
    const sFoot = makeCard(sShort, "Foot", "Footer", 3, 1)
    sShort.setBands([
      { lanes: [{ tables: [sL1, sL2] }, sR1], laneGap: 1 },
      { lanes: [sP1, "P2"], gap: 1, laneGap: 2 },
      { lanes: [sFoot], gap: 0 },
    ])
    expect(sShort.bands).toEqual([
      { lanes: [{ tables: ["L1", "L2"] }, { tables: ["R1"] }], laneGap: 1 },
      {
        lanes: [{ tables: ["P1"] }, { tables: ["P2"] }],
        gap: 1,
        laneGap: 2,
      },
      { lanes: [{ tables: ["Foot"] }], gap: 0 },
    ])
    const cShort = mShort.compile()
    expect(cShort.table("L1").address("r1", "c1")).toBe("B5")
    expect(cShort.table("L2").address("r1", "c1")).toBe("B11")
    expect(cShort.table("R1").address("r1", "c1")).toBe("E5")
    expect(cShort.table("P1").address("r1", "c1")).toBe("B16")
    expect(cShort.table("P2").address("r1", "c1")).toBe("F16")
    expect(cShort.table("Foot").address("r1", "c1")).toBe("B20")
    cShort.dispose()

    // Adding a table after setBands invalidates partition on toSpec() / compile()
    const mLate = new Model({ name: "LateTable" })
    const sLate = mLate.addSheet("S1")
    const lt1 = makeCard(sLate, "T1", "T1", 1, 1)
    sLate.setBands([lt1])
    makeCard(sLate, "T2", "T2", 1, 1)
    expect(() => sLate.toSpec()).toThrow(/missing from bands.*T2/)
    expect(() => mLate.compile()).toThrow(/missing from bands.*T2/)

    // Empty sheet setBands([]) succeeds
    const sEmpty = new Sheet("Empty")
    expect(sEmpty.bands).toEqual([])
    expect(sEmpty.setBands([])).toBe(sEmpty)
    expect(sEmpty.bands).toEqual([])
    expect(sEmpty.toSpec().bands).toEqual([])
  })

  it("Scenario 3a: setCells multi-regime timeline row authoring", () => {
    const model = new Model()
    const sheet = model.addSheet("Schedule")
    const table = sheet.addTable("T", "Multi-Regime Schedule")
    table
      .addColumn("line_item", { role: "descriptor", header: "Line Item" })
      .addColumn("standalone", { role: "measure", header: "Standalone" })
      .addColumn("adj", { role: "measure", header: "Adj." })
      .addColumn("pf", { role: "measure", header: "PF" })
      .addColumnGutter()
      .addColumn("q1", { role: "measure", header: "Q1" })
      .addColumnGutter()
      .addColumn("q2", { role: "measure", header: "Q2" })
      .addColumn("fy1", { role: "measure", header: "FY1" })
      .addColumn("fy2", { role: "measure", header: "FY2" })

    table.addRow("amort", {
      cells: { line_item: "Amortization" },
      expr: 'T["amort", col.prev] * 2',
    })
    const nullOverride: CellInputValue = null
    const sliceSelector: AxisSlice = { start: "q1", stop: "q2" }
    table.setCells("amort", ["standalone", "adj", "pf"], nullOverride)
    table.setCells("amort", sliceSelector, { expr: "10.0" })

    const compiled = model.compile()
    expect(compiled.value("T", "amort", "standalone")).toBeNull()
    expect(compiled.value("T", "amort", "adj")).toBeNull()
    expect(compiled.value("T", "amort", "pf")).toBeNull()
    expect(
      compiled.inspectCell("T", "amort", "standalone").evaluatedValue
    ).toBe("Empty")
    expect(compiled.inspectCell("T", "amort", "adj").evaluatedValue).toBe(
      "Empty"
    )
    expect(compiled.inspectCell("T", "amort", "pf").evaluatedValue).toBe(
      "Empty"
    )
    expect(compiled.num("T", "amort", "q1")).toBeCloseTo(10.0)
    expect(compiled.num("T", "amort", "q2")).toBeCloseTo(10.0)
    expect(compiled.num("T", "amort", "fy1")).toBeCloseTo(20.0)
    expect(compiled.num("T", "amort", "fy2")).toBeCloseTo(40.0)
  })

  it("Scenario 3b: setCells 2D rectangular block and metadata patching", () => {
    const model = new Model()
    const sheet = model.addSheet("Matrix")
    const table = sheet.addTable("T1", "Block Matrix")
    table
      .addColumn("c1", { header: "C1" })
      .addColumnGutter()
      .addColumn("c2", { header: "C2" })
      .addColumn("c3", { header: "C3" })

    table
      .addRow("r1", { cells: { c3: 1.0 } })
      .addSectionHeader("Section Divider")
      .addRowGutter()
      .addRow("r2", { cells: { c3: 2.0 } })
      .addRow("r3", { cells: { c1: 3.0, c2: 3.0, c3: 3.0 } })

    const rowSelector: AxisSelector = new Set(["r1", "r2"])
    table.setCells(
      { start: "r1", stop: "r2" },
      { start: "c1", stop: "c2" },
      5.0
    )
    table.setCells(rowSelector, "c2", { shade: "alt", note: "patched" })

    const { tables } = model.toSpec()
    const rowsSpec = tables[0]?.rows.filter((r) => r.kind === "data")
    expect(rowsSpec?.[0]).toEqual({
      kind: "data",
      id: "r1",
      cells: {
        c1: 5.0,
        c2: { value: 5.0, shade: "alt", note: "patched" },
        c3: 1.0,
      },
    })
    expect(rowsSpec?.[1]).toEqual({
      kind: "data",
      id: "r2",
      cells: {
        c1: 5.0,
        c2: { value: 5.0, shade: "alt", note: "patched" },
        c3: 2.0,
      },
    })
    expect(rowsSpec?.[2]).toEqual({
      kind: "data",
      id: "r3",
      cells: { c1: 3.0, c2: 3.0, c3: 3.0 },
    })

    const compiled = model.compile()
    expect(compiled.num("T1", "r1", "c1")).toBeCloseTo(5.0)
    expect(compiled.num("T1", "r2", "c1")).toBeCloseTo(5.0)
    expect(compiled.inspectCell("T1", "r1", "c2").shade).toBe("alt")
    expect(compiled.inspectCell("T1", "r1", "c2").note).toBe("patched")
    expect(compiled.inspectCell("T1", "r2", "c2").shade).toBe("alt")
    expect(compiled.inspectCell("T1", "r2", "c2").note).toBe("patched")
  })

  it("Scenario 3c & 4f: setCells selector validation and atomic failure", () => {
    const table = new Table("T1", "Validation Table")
    table.addColumn("c1", { header: "C1" })
    table.addColumn("c2", { header: "C2" })
    table.addRow("r1", { cells: { c1: 10.0 } })
    table.addRow("r2", { cells: { c1: 20.0, c2: 30.0 } })

    // Unknown row or column ID (via string, Iterable, or AxisSlice endpoint) throws Error
    expect(() => table.setCells("missing_row", "c1", 1.0)).toThrow(
      "Row 'missing_row' does not exist"
    )
    expect(() => table.setCells(["r1", "missing_row"], "c1", 1.0)).toThrow(
      "Row 'missing_row' does not exist"
    )
    expect(() =>
      table.setCells({ start: "missing_row", stop: "r2" }, "c1", 1.0)
    ).toThrow("Row 'missing_row' does not exist")
    expect(() =>
      table.setCells({ start: "r1", stop: "missing_row" }, "c1", 1.0)
    ).toThrow("Row 'missing_row' does not exist")

    expect(() => table.setCells("r1", "missing_col", 1.0)).toThrow(
      "Column 'missing_col' does not exist"
    )
    expect(() => table.setCells("r1", ["c1", "missing_col"], 1.0)).toThrow(
      "Column 'missing_col' does not exist"
    )
    expect(() =>
      table.setCells("r1", { start: "missing_col", stop: "c2" }, 1.0)
    ).toThrow("Column 'missing_col' does not exist")
    expect(() =>
      table.setCells("r1", { start: "c1", stop: "missing_col" }, 1.0)
    ).toThrow("Column 'missing_col' does not exist")

    // Inverted slice throws Error
    expect(() =>
      table.setCells("r1", { start: "c2", stop: "c1" }, 1.0)
    ).toThrow("Inverted column slice")
    expect(() =>
      table.setCells({ start: "r2", stop: "r1" }, "c1", 1.0)
    ).toThrow("Inverted row slice")

    // Empty iterable throws Error
    expect(() => table.setCells([], "c1", 1.0)).toThrow(
      "Row selector iterable cannot be empty"
    )
    expect(() => table.setCells("r1", [], 1.0)).toThrow(
      "Column selector iterable cannot be empty"
    )

    // Atomic failure: ("r1", "c1") has 10.0 and ("r1", "c2") is unpopulated
    expect(() =>
      table.setCells("r1", ["c1", "c2"], { note: "missing payload" })
    ).toThrow("must specify either 'value' or 'expr'")

    // Verify ("r1", "c1") was left completely unmutated without the note
    const spec = table.toSpec()
    const rowsSpec = spec.rows.filter((r) => r.kind === "data")
    expect(rowsSpec[0]).toEqual({
      kind: "data",
      id: "r1",
      cells: { c1: 10.0 },
    })
  })

  it("active table selector DSL (@table)", () => {
    const model = new Model({ name: "ActiveTableDSL" })
    const sheet = model.addSheet("Model")
    const table = sheet.addTable("IncomeStatement", "Income Statement")
    table.addColumn("y1", { role: "measure", header: "Y1" })
    table.addColumn("y2", { role: "measure", header: "Y2" })

    table.addRow("rev", { cells: { y1: 100.0, y2: 200.0 } })
    table.addRow("cogs", { cells: { y1: 40.0, y2: 70.0 } })
    table.addRow("gp", { expr: '@table["rev", @col] - @table["cogs", @col]' })

    const compiled = model.compile()
    const inspY1 = compiled.inspectCell("IncomeStatement", "gp", "y1")
    expect(inspY1.formulaExpr).toBe(
      '@table["rev", @col] - @table["cogs", @col]'
    )
    expect(inspY1.evaluatedValue).toEqual({ Number: 60.0 })

    const inspY2 = compiled.inspectCell("IncomeStatement", "gp", "y2")
    expect(inspY2.formulaExpr).toBe(
      '@table["rev", @col] - @table["cogs", @col]'
    )
    expect(inspY2.evaluatedValue).toEqual({ Number: 130.0 })
  })

  it("table subtitle normalization and validation", () => {
    const model = new Model({ name: "SubtitlesModel" })
    const sheet = model.addSheet("Summary")

    const t1 = sheet.addTable("T1", "T1", { subtitle: "Single" })
    const t2 = sheet.addTable("T2", "T2", {
      subtitle: ["Line 1", "Line 2"],
    })
    const t3 = sheet.addTable("T3", "T3", { subtitle: [] })
    const t4 = sheet.addTable("T4", "T4")

    for (const t of [t1, t2, t3, t4]) {
      t.addColumn("c1", { header: "Col 1" })
      t.addRow("r1", { cells: { c1: 1.0 } })
    }

    expect(t1.subtitles).toEqual(["Single"])
    expect(t2.subtitles).toEqual(["Line 1", "Line 2"])
    expect(t3.subtitles).toEqual([])
    expect(t4.subtitles).toEqual([])

    expect(t1.toSpec().subtitles).toEqual(["Single"])
    expect(t2.toSpec().subtitles).toEqual(["Line 1", "Line 2"])
    expect("subtitles" in t3.toSpec()).toBe(false)
    expect("subtitles" in t4.toSpec()).toBe(false)

    const compiled = model.compile()
    expect(compiled.table("T1").subtitles).toEqual(["Single"])
    expect(compiled.table("T2").subtitles).toEqual(["Line 1", "Line 2"])
    expect(compiled.table("T3").subtitles).toEqual([])
    expect(compiled.table("T4").subtitles).toEqual([])

    const sheetInsp = compiled.inspectSheet("Summary")
    const subtitleCells = sheetInsp.rows
      .flatMap((row) => row.cells)
      .filter((c) => c.tableId === "T2" && c.kind === "table_subtitle")
    expect(subtitleCells).toHaveLength(2)
    expect(subtitleCells.map((c) => c.formattedValue)).toEqual([
      "Line 1",
      "Line 2",
    ])

    for (const badSub of ["", ["Valid", "   "]]) {
      const badModel = new Model({ name: "BadSubtitle" })
      const badSheet = badModel.addSheet("S1")
      const badT = badSheet.addTable("Bad", "Bad", { subtitle: badSub })
      badT.addColumn("c1", { header: "Col 1" })
      badT.addRow("r1", { cells: { c1: 1.0 } })
      expect(() => badModel.compile()).toThrow(SpecificationError)
    }
  })

  it("explicit empty cells with metadata, setCell merge, and undefined vs null guards", () => {
    const model = new Model({ name: "PeerComps" })
    const sheet = model.addSheet("Comps")
    const table = sheet.addTable("PeerComps", "Peer Comps")
    table.addColumn("company", { role: "descriptor", header: "Company" })
    table.addColumn("ev_fcf", {
      header: "EV / FCF",
      format: "multiple",
      decimals: 1,
    })
    table.addColumn("ev_ebitda", {
      header: "EV / EBITDA",
      format: "multiple",
      decimals: 1,
    })

    table.addRow("crm", {
      cells: { company: "Salesforce", ev_fcf: 18.5, ev_ebitda: 14.0 },
    })
    table.addRow("orcl", {
      shade: "muted",
      highlight: true,
      cells: {
        company: "Oracle",
        ev_fcf: new Cell({
          value: null,
          note: "Excluded: negative FCF",
          shade: "none",
          highlight: false,
        }),
        ev_ebitda: null,
      },
    })
    // Merge metadata onto an existing bare null cell via setCell
    table.setCell("orcl", "ev_ebitda", { note: "Merged note on empty cell" })

    table.addRow("raw_obj", {
      cells: {
        company: "Raw Obj",
        ev_fcf: { value: null, note: "Raw object null note" },
        ev_ebitda: { value: null },
      },
    })
    table.addRow("batch", {
      cells: { company: "Batch Null" },
    })
    table.setCells("batch", ["ev_fcf", "ev_ebitda"], {
      value: null,
      note: "Batch excluded",
    })
    // Plain object with explicit JS undefined for value alongside expr must treat value as absent
    table.addRow("undef_expr", {
      cells: {
        company: "Undef Value + Expr",
        ev_fcf: { value: undefined, expr: "18.5", note: "Formula cell" },
        ev_ebitda: { value: undefined, expr: "14.0" },
      },
    })
    table.addRow("avg", {
      cells: { company: "Average" },
      expr: 'AVERAGE(PeerComps["crm".."undef_expr", @col])',
    })

    // toSpec() strips explicit JS undefined keys on raw DetailedCellInput objects (matching new Cell(...))
    const spec = model.toSpec()
    const undefRow = spec.tables[0].rows.find(
      (r) => r.kind === "data" && r.id === "undef_expr"
    )
    expect(undefRow?.kind).toBe("data")
    if (undefRow?.kind === "data") {
      const undefCell = undefRow.cells?.ev_fcf
      expect(typeof undefCell).toBe("object")
      expect(undefCell).not.toBeNull()
      if (typeof undefCell === "object" && undefCell !== null) {
        expect("value" in undefCell).toBe(false)
      }
    }

    const compiled = model.compile()

    const orclFcf = compiled.inspectCell("PeerComps", "orcl", "ev_fcf")
    expect(orclFcf.evaluatedValue).toBe("Empty")
    expect(orclFcf.note).toBe("Excluded: negative FCF")
    expect(orclFcf.shade).toBeNull()
    expect(orclFcf.isHighlight).toBe(false)

    const orclEbitda = compiled.inspectCell("PeerComps", "orcl", "ev_ebitda")
    expect(orclEbitda.evaluatedValue).toBe("Empty")
    expect(orclEbitda.note).toBe("Merged note on empty cell")

    const rawFcf = compiled.inspectCell("PeerComps", "raw_obj", "ev_fcf")
    expect(rawFcf.evaluatedValue).toBe("Empty")
    expect(rawFcf.note).toBe("Raw object null note")

    const rawEbitda = compiled.inspectCell("PeerComps", "raw_obj", "ev_ebitda")
    expect(rawEbitda.evaluatedValue).toBe("Empty")
    expect(rawEbitda.note).toBeNull()

    const batchFcf = compiled.inspectCell("PeerComps", "batch", "ev_fcf")
    expect(batchFcf.evaluatedValue).toBe("Empty")
    expect(batchFcf.note).toBe("Batch excluded")

    const undefExprFcf = compiled.inspectCell(
      "PeerComps",
      "undef_expr",
      "ev_fcf"
    )
    expect(undefExprFcf.evaluatedValue).toEqual({ Number: 18.5 })
    expect(undefExprFcf.note).toBe("Formula cell")

    expect(compiled.value("PeerComps", "orcl", "ev_fcf")).toBeNull()
    expect(compiled.num("PeerComps", "avg", "ev_fcf")).toBe(18.5)
    expect(compiled.num("PeerComps", "avg", "ev_ebitda")).toBe(14.0)
    const xlsx = compiled.emitWorkbook()
    expect(xlsx[0]).toBe(0x50)
    expect(xlsx[1]).toBe(0x4b)

    // 0D explicit null with note overrides 1D column and row vector formulas
    const overrideModel = new Model({ name: "VectorOverride" })
    const overrideSheet = overrideModel.addSheet("Comps")
    const colT = overrideSheet.addTable(
      "ColFormulaComps",
      "Column Formula Comps"
    )
    colT.addColumn("company", { role: "descriptor", header: "Company" })
    colT.addColumn("ev", { header: "EV", format: "currency", decimals: 1 })
    colT.addColumn("fcf", { header: "FCF", format: "currency", decimals: 1 })
    colT.addColumn("ev_fcf", {
      header: "EV / FCF",
      format: "multiple",
      decimals: 1,
      expr: 'ColFormulaComps[@row, "ev"] / ColFormulaComps[@row, "fcf"]',
    })
    colT.addRow("crm", {
      cells: { company: "Salesforce", ev: 185.0, fcf: 10.0 },
    })
    colT.addRow("orcl", {
      cells: {
        company: "Oracle",
        ev: 400.0,
        fcf: -5.0,
        ev_fcf: new Cell({ value: null, note: "Excluded: negative FCF" }),
      },
    })
    colT.addRow("avg", {
      cells: {
        company: "Average",
        ev: null,
        fcf: null,
        ev_fcf: new Cell({
          expr: 'AVERAGE(ColFormulaComps["crm".."orcl", @col])',
        }),
      },
    })

    const rowT = overrideSheet.addTable(
      "RowFormulaOverride",
      "Row Formula Override"
    )
    rowT.addColumn("c1", { header: "C1" })
    rowT.addColumn("c2", { header: "C2" })
    rowT.addRow("r1", {
      expr: "10 + 5",
      cells: { c1: { value: null, note: "Excluded from row formula" } },
    })

    const overrideCompiled = overrideModel.compile()
    const crmCalc = overrideCompiled.inspectCell(
      "ColFormulaComps",
      "crm",
      "ev_fcf"
    )
    expect(crmCalc.evaluatedValue).toEqual({ Number: 18.5 })
    expect(crmCalc.isFormula).toBe(true)

    const orclOverride = overrideCompiled.inspectCell(
      "ColFormulaComps",
      "orcl",
      "ev_fcf"
    )
    expect(orclOverride.evaluatedValue).toBe("Empty")
    expect(orclOverride.isFormula).toBe(false)
    expect(orclOverride.formulaExpr).toBeNull()
    expect(orclOverride.note).toBe("Excluded: negative FCF")
    expect(overrideCompiled.num("ColFormulaComps", "avg", "ev_fcf")).toBe(18.5)

    const rowC1 = overrideCompiled.inspectCell("RowFormulaOverride", "r1", "c1")
    expect(rowC1.evaluatedValue).toBe("Empty")
    expect(rowC1.isFormula).toBe(false)
    expect(rowC1.note).toBe("Excluded from row formula")
    expect(overrideCompiled.num("RowFormulaOverride", "r1", "c2")).toBe(15.0)

    // Direct compile() also strips explicit undefined keys and preserves explicit null without builder pre-normalization
    const directCompiled = compile(
      {
        modelName: "DirectCompileNullUndef",
        sheets: [{ name: "S1", bands: [{ lanes: [{ tables: ["T1"] }] }] }],
      },
      [
        {
          id: "T1",
          title: "T1",
          columns: [
            { kind: "data", id: "c1", headers: ["C1"], format: "currency" },
            { kind: "data", id: "c2", headers: ["C2"] },
          ],
          rows: [
            {
              kind: "data",
              id: "r1",
              cells: {
                c1: {
                  value: undefined,
                  expr: "18.5",
                  currency: "USD",
                  note: "Direct formula",
                },
                c2: { value: null, note: "Direct empty" },
              },
            },
          ],
        },
      ]
    )
    const directC1 = directCompiled.inspectCell("T1", "r1", "c1")
    expect(directC1.evaluatedValue).toEqual({ Number: 18.5 })
    expect(directC1.currency).toBe("usd")
    expect(directC1.note).toBe("Direct formula")
    const directC2 = directCompiled.inspectCell("T1", "r1", "c2")
    expect(directC2.evaluatedValue).toBe("Empty")
    expect(directC2.note).toBe("Direct empty")

    // Raw object with both explicit value: null and expr must fail with mutual exclusivity error
    const conflictModel = new Model({ name: "ConflictNullExpr" })
    const conflictSheet = conflictModel.addSheet("S1")
    const conflictT = conflictSheet.addTable("T1", "T1")
    conflictT.addColumn("c1", { header: "C1" })
    conflictT.addRow("r1", { cells: { c1: { value: null, expr: "1 + 1" } } })
    expect(() => conflictModel.compile()).toThrow(SpecificationError)
    expect(() => conflictModel.compile()).toThrow("both 'value' and 'expr'")

    // Raw object omitting both value and expr (including explicit undefined) must fail with EmptyCellPayload
    for (const badCell of [
      { note: "Missing payload" },
      { value: undefined, note: "Undefined value payload" },
    ]) {
      const emptyModel = new Model({ name: "MissingPayload" })
      const emptySheet = emptyModel.addSheet("S1")
      const emptyT = emptySheet.addTable("T1", "T1")
      emptyT.addColumn("c1", { header: "C1" })
      emptyT.addRow("r1", { cells: { c1: badCell } })
      expect(() => emptyModel.compile()).toThrow(SpecificationError)
      expect(() => emptyModel.compile()).toThrow("neither 'value' nor 'expr'")
    }
  })

  it("Scenario 9: aligned table title banners parity across inspectSheet and emitWorkbook", () => {
    const model = new Model({ name: "AlignedBannerParity" })
    const sheet = model.addSheet("Summary")

    const top = sheet.addTable("TopTable", "Top Table Summary Banner")
    top.addColumn("item", { role: "descriptor", header: "Item" })
    top.addColumn("v1", { header: "Val 1" })
    top.addRow("r1", { cells: { item: "Metric A", v1: 100.0 } })

    const bottom = sheet.addTable("WideTable", "Wide Forecast Table")
    bottom.addColumn("item", { role: "descriptor", header: "Item" })
    bottom.addColumn("y1", { header: "2024E" })
    bottom.addColumn("y2", { header: "2025E" })
    bottom.addColumn("y3", { header: "2026E" })
    bottom.addRow("r1", {
      cells: { item: "Revenue", y1: 10.0, y2: 20.0, y3: 30.0 },
    })

    const compiled = model.compile()
    const sheetInsp = compiled.inspectSheet("Summary")

    expect(sheetInsp.minCol).toBe(1)
    expect(sheetInsp.maxCol).toBe(4)
    expect(sheetInsp.minRow).toBe(1)

    // Row 0 (physicalRow 1, excelRow 2): TopTable title spans all 4 columns (B2..E2)
    const topTitleRow = sheetInsp.rows[0]
    expect(topTitleRow?.physicalRow).toBe(1)
    expect(topTitleRow?.excelRow).toBe(2)
    expect(
      topTitleRow?.cells.map((c) => [
        c.physicalCol,
        c.physicalA1,
        c.kind,
        c.tableId,
      ])
    ).toEqual([
      [1, "B2", "table_title", "TopTable"],
      [2, "C2", "table_title", "TopTable"],
      [3, "D2", "table_title", "TopTable"],
      [4, "E2", "table_title", "TopTable"],
    ])
    expect(topTitleRow?.cells.map((c) => c.formattedValue)).toEqual([
      "Top Table Summary Banner",
      null,
      null,
      null,
    ])

    // TopTable header row (B4..C4) and data row (B5..C5) remain strictly on 2 columns
    const topHeaderRow = sheetInsp.rows[2]
    expect(
      topHeaderRow?.cells.map((c) => [c.physicalA1, c.kind, c.tableId])
    ).toEqual([
      ["B4", "column_header", "TopTable"],
      ["C4", "column_header", "TopTable"],
      ["D4", "empty", null],
      ["E4", "empty", null],
    ])
    const topDataRow = sheetInsp.rows[3]
    expect(
      topDataRow?.cells.map((c) => [c.physicalA1, c.kind, c.tableId])
    ).toEqual([
      ["B5", "data", "TopTable"],
      ["C5", "data", "TopTable"],
      ["D5", "empty", null],
      ["E5", "empty", null],
    ])

    // Cell A1 addresses and colWidths remain unchanged
    expect(compiled.inspectCell("TopTable", "r1", "item").physicalAddress).toBe(
      "'Summary'!B5"
    )
    expect(compiled.inspectCell("TopTable", "r1", "v1").physicalAddress).toBe(
      "'Summary'!C5"
    )
    expect(compiled.inspectCell("WideTable", "r1", "y3").physicalAddress).toBe(
      "'Summary'!E11"
    )
    expect(sheetInsp.colWidths[1]).toBeCloseTo(12.0)
    expect(sheetInsp.colWidths[2]).toBeCloseTo(12.0)
    expect(sheetInsp.colWidths[3]).toBeCloseTo(12.0)
    expect(sheetInsp.colWidths[4]).toBeCloseTo(12.0)

    // OpenXML workbook emits styled title cells across B2..E2 while keeping TopTable headers/data on B..C
    const xlsxBytes = compiled.emitWorkbook()
    expect(xlsxBytes[0]).toBe(0x50)
    expect(xlsxBytes[1]).toBe(0x4b)
    const sheetXml = readZipEntryUtf8(xlsxBytes, "xl/worksheets/sheet1.xml")

    const titleStyles = ["B2", "C2", "D2", "E2"].map((ref) => {
      const match = new RegExp(`<c r="${ref}"[^>]*\\bs="(\\d+)"`).exec(sheetXml)
      expect(match).not.toBeNull()
      return match?.[1]
    })
    expect(new Set(titleStyles).size).toBe(1)
    expect(sheetXml).not.toContain('<c r="D4"')
    expect(sheetXml).not.toContain('<c r="E4"')
    expect(sheetXml).not.toContain('<c r="D5"')
    expect(sheetXml).not.toContain('<c r="E5"')
  })

  it("sheet showGridlines defaults to false across builder, raw compile, and .xlsx emission", () => {
    expect(new Sheet("Direct").showGridlines).toBe(false)
    expect(
      new Sheet("DirectVisible", { showGridlines: true }).showGridlines
    ).toBe(true)

    const model = new Model({ name: "GridlineDefaults" })
    const hiddenSheet = model.addSheet("DefaultHidden")
    const visibleSheet = model.addSheet("ExplicitVisible", {
      showGridlines: true,
    })

    expect(hiddenSheet.showGridlines).toBe(false)
    expect(visibleSheet.showGridlines).toBe(true)

    for (const [sheet, id] of [
      [hiddenSheet, "T1"],
      [visibleSheet, "T2"],
    ] as const) {
      const tbl = sheet.addTable(id, id)
      tbl.addColumn("c1", { header: "C1" })
      tbl.addRow("r1", { cells: { c1: 1.0 } })
    }

    const { manifest } = model.toSpec()
    expect(manifest.sheets[0]?.showGridlines).toBe(false)
    expect(manifest.sheets[1]?.showGridlines).toBe(true)

    const xlsxBytes = model.compile().emitWorkbook()
    const hiddenXml = readZipEntryUtf8(xlsxBytes, "xl/worksheets/sheet1.xml")
    const visibleXml = readZipEntryUtf8(xlsxBytes, "xl/worksheets/sheet2.xml")
    expect(hiddenXml).toContain('showGridLines="0"')
    expect(visibleXml).not.toContain('showGridLines="0"')

    // Raw compile() with omitted showGridlines also defaults to false
    const rawCompiled = compile(
      {
        modelName: "RawDefaults",
        theme: "corporate_navy",
        sheets: [
          { name: "RawSheet", bands: [{ lanes: [{ tables: ["T1"] }] }] },
        ],
      },
      [
        {
          id: "T1",
          title: "T1",
          columns: [{ kind: "data", id: "c1", headers: ["C1"] }],
          rows: [{ kind: "data", id: "r1", cells: { c1: 1.0 } }],
        },
      ]
    )
    const rawXml = readZipEntryUtf8(
      rawCompiled.emitWorkbook(),
      "xl/worksheets/sheet1.xml"
    )
    expect(rawXml).toContain('showGridLines="0"')
  })

  it("sheet grid layout validation, diagnostics, and unknown-field rejection (Scenarios 5, 6, 7)", () => {
    const mode: SheetLayoutMode = "grid"
    expect(mode).toBe("grid")

    // Default Sheet layout is "auto" and omits layout from toSpec()
    const defaultSheet = new Sheet("AutoDefault")
    expect(defaultSheet.layout).toBe("auto")
    expect(defaultSheet.colWidth).toBeNull()
    expect("layout" in defaultSheet.toSpec()).toBe(false)

    // Valid grid sheets with and without colWidth
    const gridDefault = new Sheet("GridDefault", { layout: "grid" })
    expect(gridDefault.toSpec().layout).toEqual({ kind: "grid" })
    const gridSized = new Sheet("GridSized", { layout: "grid", colWidth: 14 })
    expect(gridSized.colWidth).toBe(14)
    expect(gridSized.toSpec().layout).toEqual({ kind: "grid", colWidth: 14 })

    // Scenario 7: SDK constructor validation
    expect(
      () =>
        new Sheet("BadLayout", {
          layout: "fixed" as unknown as SheetLayoutMode,
        })
    ).toThrow("Invalid sheet layout")
    expect(
      () => new Sheet("AutoWithWidth", { layout: "auto", colWidth: 12 })
    ).toThrow("colWidth can only be specified when layout is 'grid'")
    for (const badWidth of [
      0,
      0.5,
      -5,
      Number.NaN,
      Number.POSITIVE_INFINITY,
      Number.NEGATIVE_INFINITY,
      256,
    ]) {
      expect(
        () => new Sheet("BadWidth", { layout: "grid", colWidth: badWidth })
      ).toThrow("colWidth must be a finite number in [1, 255]")
    }

    // Scenario 7: Raw compile() preserves unknown keys on layout so Rust deny_unknown_fields rejects them
    const minimalTable = {
      id: "T1",
      title: "T1",
      columns: [{ kind: "data" as const, id: "c1", headers: ["C1"] }],
      rows: [{ kind: "data" as const, id: "r1", cells: { c1: 1.0 } }],
    }
    const badLayouts = [
      { kind: "grid", colWidth: 14, bogus: 1 },
      { kind: "auto", colWidth: 12 },
      { kind: "auto", extra: true },
      { kind: "grid", colWidth: 0.5 },
      { kind: "grid", colWidth: 256 },
    ]
    for (const badLayout of badLayouts) {
      expect(() =>
        compile(
          {
            modelName: "BadRawLayout",
            theme: "corporate_navy",
            sheets: [
              {
                name: "S1",
                bands: [{ lanes: [{ tables: ["T1"] }] }],
                layout: badLayout as unknown as { readonly kind: "grid" },
              },
            ],
          },
          [minimalTable]
        )
      ).toThrow(SpecificationError)
    }

    // Scenario 5: Static base_w measure header rejection raises SpecificationError
    const hdrModel = new Model({ name: "WideHeader" })
    const hdrSheet = hdrModel.addSheet("Dash", {
      layout: "grid",
      colWidth: 12,
    })
    const hdrT = hdrSheet.addTable("Val", "Valuation")
    hdrT.addColumn("item", { role: "descriptor", header: "Item" })
    hdrT.addColumn("ev", { header: "Enterprise Value" })
    hdrT.addRow("r1", { cells: { item: "Base", ev: 100.0 } })
    try {
      hdrModel.compile()
      expect.unreachable("Should have thrown SpecificationError")
    } catch (err) {
      expect(err).toBeInstanceOf(SpecificationError)
      expect((err as SpecificationError).table).toBe("Val")
      expect((err as SpecificationError).message).toContain("Enterprise Value")
    }

    // Scenario 6: Multiline cell, column header, and group banner raise SpecificationError
    const nlCellModel = new Model({ name: "NewlineCell" })
    const nlT = nlCellModel
      .addSheet("Dash", { layout: "grid" })
      .addTable("T1", "T1")
    nlT.addColumn("item", { role: "descriptor", header: "Item" })
    nlT.addRow("r1", { cells: { item: "Line 1\nLine 2" } })
    try {
      nlCellModel.compile()
      expect.unreachable("Should have thrown SpecificationError")
    } catch (err) {
      expect(err).toBeInstanceOf(SpecificationError)
      expect((err as SpecificationError).table).toBe("T1")
      expect((err as SpecificationError).message).toContain("multiline cell")
    }

    const nlHdrModel = new Model({ name: "NewlineHdr" })
    const nlHt = nlHdrModel
      .addSheet("Dash", { layout: "grid" })
      .addTable("T2", "T2")
    nlHt.addColumn("m", { header: "Line 1\nLine 2" })
    nlHt.addRow("r1", { cells: { m: 1.0 } })
    try {
      nlHdrModel.compile()
      expect.unreachable("Should have thrown SpecificationError")
    } catch (err) {
      expect(err).toBeInstanceOf(SpecificationError)
      expect((err as SpecificationError).table).toBe("T2")
      expect((err as SpecificationError).message).toContain(
        "multiline column header"
      )
    }

    const nlGrpModel = new Model({ name: "NewlineGrp" })
    const nlGt = nlGrpModel
      .addSheet("Dash", { layout: "grid" })
      .addTable("T3", "T3")
    nlGt.addColumn("m", { group: "Forecast\rYears", header: "Y1" })
    nlGt.addRow("r1", { cells: { m: 1.0 } })
    try {
      nlGrpModel.compile()
      expect.unreachable("Should have thrown SpecificationError")
    } catch (err) {
      expect(err).toBeInstanceOf(SpecificationError)
      expect((err as SpecificationError).table).toBe("T3")
      expect((err as SpecificationError).message).toContain(
        "multiline column header"
      )
    }
  })

  it("sheet grid layout e2e parity with Python SDK (Scenarios 10 & 12)", () => {
    const model = new Model({ name: "CrossSdkGridParity" })
    const dash = model.addSheet("Dashboard", { layout: "grid", colWidth: 14 })

    const summary = dash.addTable("Summary", "Executive Summary")
    summary.addColumn("item", { role: "descriptor", header: "Line Item" })
    summary.addColumn("y1", {
      header: "2024E",
      format: "currency",
      decimals: 1,
    })
    summary.addColumn("y2", {
      header: "2025E",
      format: "currency",
      decimals: 1,
    })
    summary.addRow("rev", {
      cells: {
        item: "Total Consolidated Operating Revenue",
        y1: 100.0,
        y2: 120.0,
      },
    })
    summary.addRow("ebitda", {
      style: "total",
      cells: { item: "Adjusted EBITDA" },
      expr: 'Summary["rev", @col] * 0.25',
    })

    const kpis = dash.addTable("KPIs", "Key Metrics")
    kpis.addColumn("metric", { role: "descriptor", header: "Metric" })
    kpis.addColumn("val", {
      header: "Value",
      format: "percentage",
      decimals: 1,
    })
    kpis.addRow("margin", {
      cells: { metric: "2025E Consolidated Margin" },
      expr: 'Summary["ebitda", "y2"] / Summary["rev", "y2"]',
    })
    dash.setBands([{ lanes: [{ tables: [summary] }, { tables: [kpis] }] }])

    const notesSheet = model.addSheet("Notes")
    const notes = notesSheet.addTable("Commentary", "Notes")
    notes.addColumn("topic", { role: "descriptor", header: "Topic" })
    notes.addColumn("detail", { role: "descriptor", header: "Detail" })
    notes.addRow("n1", { cells: { topic: "Basis", detail: "Line 1\nLine 2" } })

    const { manifest } = model.toSpec()
    expect(manifest.sheets[0]?.layout).toEqual({ kind: "grid", colWidth: 14 })
    expect(manifest.sheets[1]?.layout).toBeUndefined()

    const compiled = model.compile()
    expect(compiled.table("Summary").address("rev", "item")).toBe("B5")
    expect(compiled.table("Summary").address("rev", "y1")).toBe("E5")
    expect(compiled.table("Summary").address("rev", "y2")).toBe("F5")
    expect(compiled.table("Summary").address("ebitda", "y2")).toBe("F6")
    expect(compiled.table("KPIs").address("margin", "metric")).toBe("H5")
    expect(compiled.table("KPIs").address("margin", "val")).toBe("J5")
    expect(compiled.num("KPIs", "margin", "val")).toBeCloseTo(0.25)

    const insp = compiled.inspectSheet("Dashboard")
    expect(insp.minCol).toBe(1)
    expect(insp.maxCol).toBe(9)
    expect(insp.colWidths).toEqual({
      0: 2.0,
      1: 14.0,
      2: 14.0,
      3: 14.0,
      4: 14.0,
      5: 14.0,
      6: 2.0,
      7: 14.0,
      8: 14.0,
      9: 14.0,
    })

    const row5 = insp.rows.find((r) => r.excelRow === 5)
    expect(row5).toBeDefined()
    expect(
      row5?.cells.map((c) => [c.physicalA1, c.kind, c.tableId, c.isDescriptor])
    ).toEqual([
      ["B5", "data", "Summary", true],
      ["C5", "overflow", "Summary", true],
      ["D5", "overflow", "Summary", true],
      ["E5", "data", "Summary", false],
      ["F5", "data", "Summary", false],
      ["G5", "empty", null, false],
      ["H5", "data", "KPIs", true],
      ["I5", "overflow", "KPIs", true],
      ["J5", "data", "KPIs", false],
    ])
    const c5 = row5?.cells[1]
    expect(c5?.rowId).toBe("rev")
    expect(c5?.colId).toBe("item")
    expect(c5?.value).toBe("Empty")
    expect(c5?.formattedValue).toBeNull()

    const xlsx = compiled.emitWorkbook()
    expect(xlsx[0]).toBe(0x50)
    expect(xlsx[1]).toBe(0x4b)
    const dashXml = readZipEntryUtf8(xlsx, "xl/worksheets/sheet1.xml")
    expect(dashXml).not.toContain("<mergeCells")
  })

  it("sheet print settings cascade, validation, layout markdown, and .xlsx XML parity (Scenarios 1-8)", () => {
    const paper: PaperSize = "a4"
    const orient: PageOrientation = "landscape"
    const fit: PrintFitMode = "fit_width"
    const hf: HeaderFooterInput = { left: "{model}" }
    const sp: SheetPrintSpecInput = {
      paperSize: paper,
      orientation: orient,
      fit,
    }
    const spo: SheetPrintOverrideInput = { footer: "none" }
    expect(hf.left).toBe("{model}")
    expect(sp.paperSize).toBe("a4")
    expect(spo.footer).toBe("none")

    // Eager SDK validation on Model, Sheet, and BandInput
    expect(
      () =>
        new Model({
          print: { orientation: "portrait" } as unknown as SheetPrintSpecInput,
        })
    ).toThrow("paperSize")
    expect(
      () =>
        new Model({
          print: {
            paperSize: "a4",
            header: "none",
          } as unknown as SheetPrintSpecInput,
        })
    ).toThrow("cannot be 'none'")
    expect(
      () =>
        new Model({
          print: {
            paperSize: "a4",
            footer: "none",
          } as unknown as SheetPrintSpecInput,
        })
    ).toThrow("cannot be 'none'")
    expect(
      () =>
        new Model({
          print: {
            paperSize: "a4",
            scale: 80,
          } as unknown as SheetPrintSpecInput,
        })
    ).toThrow("Unknown print configuration key")
    expect(
      () =>
        new Model({
          print: {
            paperSize: "a4",
            header: { bogus: "x" },
          } as unknown as SheetPrintSpecInput,
        })
    ).toThrow("Unknown print header key")
    expect(
      () =>
        new Model({
          print: {
            paperSize: "a3" as unknown as PaperSize,
          },
        })
    ).toThrow("Invalid paperSize")
    expect(
      () =>
        new Model({
          print: "none" as unknown as SheetPrintSpecInput,
        })
    ).toThrow("Print configuration must be an object, got none")
    expect(
      () =>
        new Sheet("MissingPaper", {
          print: { orientation: "portrait" },
        })
    ).toThrow("paperSize")
    expect(
      () =>
        new Sheet("BadOrient", {
          print: {
            paperSize: "a4",
            orientation: "diagonal" as unknown as PageOrientation,
          },
        })
    ).toThrow("Invalid orientation")
    expect(
      () =>
        new Sheet("BadFit", {
          print: {
            paperSize: "a4",
            fit: "fit_height" as unknown as PrintFitMode,
          },
        })
    ).toThrow("Invalid fit")
    expect(
      () =>
        new Sheet("BadStr", {
          print: "bogus" as unknown as SheetPrintOverrideInput,
        })
    ).toThrow("Print configuration must be an object or 'none'")
    expect(
      () =>
        new Sheet("BadCenter", {
          print: {
            paperSize: "a4",
            centerHorizontally: "yes" as unknown as boolean,
          },
        })
    ).toThrow("centerHorizontally")
    expect(
      () =>
        new Sheet("UnknownKey", {
          print: {
            paperSize: "a4",
            extra: 1,
          } as unknown as SheetPrintOverrideInput,
        })
    ).toThrow("Unknown print configuration key")

    // No-op "none" opt-outs when unset
    const noPrintSheet = new Sheet("NoPrint", { print: "none" })
    expect(noPrintSheet.print).toBeNull()
    expect("print" in noPrintSheet.toSpec()).toBe(false)

    const noopHfSheet = new Sheet("NoopHF", {
      print: { paperSize: "a4", header: "none", footer: "none" },
    })
    expect(noopHfSheet.print).toEqual({ paperSize: "a4" })

    // Band 0 pageBreakBefore & non-boolean pageBreakBefore validation
    const b0Sheet = new Sheet("Band0Check", { print: { paperSize: "a4" } })
    const b0Tbl = b0Sheet.addTable("T0", "T0")
    b0Tbl.addColumn("c1", { header: "C1" })
    b0Tbl.addRow("r1", { cells: { c1: 1.0 } })
    expect(() =>
      b0Sheet.setBands([{ lanes: [b0Tbl], pageBreakBefore: true }])
    ).toThrow("band 0 cannot specify 'pageBreakBefore: true'")
    expect(() =>
      b0Sheet.setBands([
        {
          lanes: [b0Tbl],
          pageBreakBefore: "yes" as unknown as boolean,
        },
      ])
    ).toThrow("pageBreakBefore")

    // Scenario 8: Model print cascade + shallow overrides + wholesale footer replacement +
    // "none" opt-out + pageBreakBefore + XML + Layout Markdown + determinism + valuesOnly
    const model = new Model({
      name: "Acme & Co",
      print: {
        paperSize: "a4",
        header: {
          left: "{model}",
          center: "2026 Forecast {{Rev}}",
          right: "{sheet}",
        },
        footer: {
          left: "M&A Confidential",
          right: "Page {page} of {pages} ({date})",
        },
      },
    })

    const sInherit = model.addSheet("Summary")
    const t1 = sInherit.addTable("SummaryTop", "Executive Summary")
    t1.addColumn("item", { role: "descriptor", header: "Item" })
    t1.addColumn("y1", { header: "2024E", format: "currency", decimals: 0 })
    t1.addColumn("y2", { header: "2025E", format: "currency", decimals: 0 })
    t1.addRow("rev", {
      cells: { item: "Revenue", y1: 100.0 },
      expr: 'SummaryTop["rev", "y1"] * 1.1',
    })
    const t2 = sInherit.addTable("SummaryBot", "Supporting Detail")
    t2.addColumn("item", { role: "descriptor", header: "Item" })
    t2.addColumn("y1", { header: "2024E" })
    t2.addRow("note", { cells: { item: "Line 1\nLine 2", y1: 50.0 } })
    sInherit.setBands([{ lanes: [t1] }, { lanes: [t2], pageBreakBefore: true }])
    expect(sInherit.bands[1]?.pageBreakBefore).toBe(true)

    const sFitPage = model.addSheet("Dashboard", {
      print: {
        paperSize: "letter",
        orientation: "portrait",
        fit: "fit_page",
        centerHorizontally: false,
        header: "none",
        footer: "none",
      },
    })
    const t3 = sFitPage.addTable("Dash", "Dashboard")
    t3.addColumn("k", { role: "descriptor", header: "KPI" })
    t3.addColumn("v", { header: "Value" })
    t3.addRow("m", { cells: { k: "IRR", v: 0.25 } })

    const sReplace = model.addSheet("Appendix", {
      print: { footer: { center: "Appendix Only" } },
    })
    const t4 = sReplace.addTable("App", "Appendix")
    t4.addColumn("k", { role: "descriptor", header: "Key" })
    t4.addColumn("v", { header: "Value" })
    t4.addRow("r1", { cells: { k: "Note", v: 1.0 } })

    const sOptOut = model.addSheet("Scratch", { print: "none" })
    const t5 = sOptOut.addTable("Work", "Scratch")
    t5.addColumn("v", { header: "Val" })
    t5.addRow("r1", { cells: { v: 42.0 } })

    model.addSheet("EmptyPrint")

    const { manifest } = model.toSpec()
    expect(manifest.sheets[0]?.print).toEqual({
      paperSize: "a4",
      header: {
        left: "{model}",
        center: "2026 Forecast {{Rev}}",
        right: "{sheet}",
      },
      footer: {
        left: "M&A Confidential",
        right: "Page {page} of {pages} ({date})",
      },
    })
    expect(manifest.sheets[0]?.bands[1]?.pageBreakBefore).toBe(true)
    expect(manifest.sheets[1]?.print).toEqual({
      paperSize: "letter",
      orientation: "portrait",
      fit: "fit_page",
      centerHorizontally: false,
    })
    expect(manifest.sheets[2]?.print).toEqual({
      paperSize: "a4",
      header: {
        left: "{model}",
        center: "2026 Forecast {{Rev}}",
        right: "{sheet}",
      },
      footer: { center: "Appendix Only" },
    })
    expect(manifest.sheets[3]?.print).toBeUndefined()
    expect(manifest.sheets[4]?.print?.paperSize).toBe("a4")

    const compiled = model.compile()

    // Layout Markdown checks (Scenarios 1, 5, 6)
    const layoutMd = compiled.toMarkdown({ detail: "layout" })
    expect(layoutMd).toContain(
      "Print: a4 landscape | Fit: fit_width (1 x auto) | Area: B2:D11"
    )
    expect(layoutMd).toContain(", page_break):")
    expect(layoutMd).toContain(
      "Print: letter portrait | Fit: fit_page (1 x 1) | Area: B2:C5"
    )
    const scratchSection =
      layoutMd.split("## Sheet: Scratch\n")[1]?.split("## Sheet: ")[0] ?? ""
    expect(scratchSection).not.toContain("Print:")
    const emptySection = layoutMd.split("## Sheet: EmptyPrint\n")[1] ?? ""
    expect(emptySection).toContain("_(no tables)_")
    expect(emptySection).not.toContain("Print:")

    // OpenXML checks, determinism, and valuesOnly parity
    const buf1 = compiled.emitWorkbook()
    const buf2 = compiled.emitWorkbook()
    expect(Buffer.from(buf1)).toEqual(Buffer.from(buf2))
    const bufFrozen = compiled.emitWorkbook({ valuesOnly: true })

    for (const buf of [buf1, bufFrozen]) {
      const wbXml = readZipEntryUtf8(buf, "xl/workbook.xml")
      const sh1Xml = readZipEntryUtf8(buf, "xl/worksheets/sheet1.xml")
      const sh2Xml = readZipEntryUtf8(buf, "xl/worksheets/sheet2.xml")
      const sh3Xml = readZipEntryUtf8(buf, "xl/worksheets/sheet3.xml")
      const sh4Xml = readZipEntryUtf8(buf, "xl/worksheets/sheet4.xml")
      const sh5Xml = readZipEntryUtf8(buf, "xl/worksheets/sheet5.xml")

      expect(wbXml).toContain(
        'name="_xlnm.Print_Area" localSheetId="0">Summary!$B$2:$D$11<'
      )
      expect(wbXml).not.toContain('name="_xlnm.Print_Area" localSheetId="3"')
      expect(wbXml).not.toContain('name="_xlnm.Print_Area" localSheetId="4"')

      expect(sh1Xml).toContain('<pageSetUpPr fitToPage="1"/>')
      expect(sh1Xml).toContain('<printOptions horizontalCentered="1"/>')
      expect(sh1Xml).toContain(
        '<pageSetup paperSize="9" fitToHeight="0" orientation="landscape"'
      )
      expect(sh1Xml).toContain('<rowBreaks count="1" manualBreakCount="1">')
      expect(sh1Xml).toContain('<headerFooter scaleWithDoc="0">')
      expect(sh1Xml).toContain("Acme &amp;&amp; Co")
      expect(sh1Xml).toContain("2026 Forecast {Rev}")
      expect(sh1Xml).toContain("Page &amp;P of &amp;N (&amp;D)")

      expect(sh2Xml).toContain('<pageSetUpPr fitToPage="1"/>')
      expect(sh2Xml).toContain(
        '<pageSetup paperSize="1" orientation="portrait"'
      )
      expect(sh2Xml).not.toContain("fitToWidth=")
      expect(sh2Xml).not.toContain("fitToHeight=")
      expect(sh2Xml).not.toContain("<printOptions")
      expect(sh2Xml).not.toContain("<headerFooter")

      expect(sh3Xml).toContain("Appendix Only")
      expect(sh3Xml).not.toContain("Confidential")
      expect(sh3Xml).not.toContain("Page &amp;P")

      expect(sh4Xml).not.toContain("<pageSetUpPr")
      expect(sh4Xml).not.toContain("<pageSetup")
      expect(sh4Xml).not.toContain("<printOptions")
      expect(sh4Xml).not.toContain("<headerFooter")
      expect(sh4Xml).not.toContain("<rowBreaks")

      expect(sh5Xml).toContain(
        '<pageSetup paperSize="9" fitToHeight="0" orientation="landscape"'
      )
      expect(sh5Xml).not.toContain("<rowBreaks")
    }

    // Direct compile(): undefined stripping on print/header/footer/band AND unknown-key forwarding to WASM
    const minTbl = {
      id: "T1",
      title: "T1",
      columns: [{ kind: "data" as const, id: "c1", headers: ["C1"] }],
      rows: [{ kind: "data" as const, id: "r1", cells: { c1: 1.0 } }],
    }
    const minTbl2 = { ...minTbl, id: "T2" }

    const directCompiled = compile(
      {
        modelName: "DirectUndefStrip",
        sheets: [
          {
            name: "S1",
            print: {
              paperSize: "a4",
              orientation: undefined,
              fit: undefined,
              centerHorizontally: undefined,
              header: { left: undefined, center: "{sheet}", right: undefined },
              footer: undefined,
            },
            bands: [
              { lanes: [{ tables: ["T1"] }], pageBreakBefore: undefined },
              { lanes: [{ tables: ["T2"] }], pageBreakBefore: true },
            ],
          },
        ],
      },
      [minTbl, minTbl2]
    )
    const directXml = readZipEntryUtf8(
      directCompiled.emitWorkbook(),
      "xl/worksheets/sheet1.xml"
    )
    expect(directXml).toContain('<rowBreaks count="1" manualBreakCount="1">')
    expect(directXml).toContain("<oddHeader>")
    expect(directXml).not.toContain("<oddFooter>")

    // Scenario 7: Raw compile() rejects unknown keys and Pass 1 print violations via WASM
    const invalidRawManifests: unknown[] = [
      {
        modelName: "M",
        sheets: [
          {
            name: "S1",
            print: { paperSize: "a4", bogusPrint: 1 },
            bands: [{ lanes: [{ tables: ["T1"] }] }],
          },
        ],
      },
      {
        modelName: "M",
        sheets: [
          {
            name: "S1",
            print: { paperSize: "a4", header: { left: "A", extraZone: "B" } },
            bands: [{ lanes: [{ tables: ["T1"] }] }],
          },
        ],
      },
      {
        modelName: "M",
        sheets: [
          {
            name: "S1",
            print: { paperSize: "a4", header: {} },
            bands: [{ lanes: [{ tables: ["T1"] }] }],
          },
        ],
      },
      {
        modelName: "[Page] Trick",
        sheets: [
          {
            name: "S1",
            print: { paperSize: "a4", header: { left: "A&{model}" } },
            bands: [{ lanes: [{ tables: ["T1"] }] }],
          },
        ],
      },
      {
        modelName: "M",
        sheets: [
          {
            name: "S1",
            print: { paperSize: "a4", fit: "fit_page" },
            bands: [
              { lanes: [{ tables: ["T1"] }] },
              { lanes: [{ tables: ["T2"] }], pageBreakBefore: true },
            ],
          },
        ],
      },
    ]
    for (const badManifest of invalidRawManifests) {
      expect(() =>
        compile(badManifest as Parameters<typeof compile>[0], [minTbl, minTbl2])
      ).toThrow(SpecificationError)
    }
  })
})
