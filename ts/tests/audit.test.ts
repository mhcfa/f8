import { describe, expect, it } from "vitest"
import { formatWithUnit } from "../src/audit.js"
import {
  Cell,
  type CellError,
  type CompiledModel,
  F8Error,
  InspectionError,
  Model,
  type SplitDict,
  TableAuditor,
  type TableViewMode,
  unboxCellValue,
} from "../src/index.js"

function buildTestModel(): CompiledModel {
  const model = new Model({ name: "PerformanceModel", theme: "corporate_navy" })
  const sheet = model.addSheet("Financials")
  const table = sheet.addTable("Performance", "Performance", {
    subtitle: "($ in Millions)",
    currency: "USD",
  })
  table.addColumn("line_item", { role: "descriptor", header: "Line Item" })
  table.addColumn("2023A", {
    role: "measure",
    header: "2023A",
    format: "currency",
    decimals: 1,
  })
  table.addColumn("2024E", {
    role: "measure",
    header: "2024E",
    format: "currency",
    decimals: 1,
  })
  table.addColumn("2025E", {
    role: "measure",
    header: "2025E",
    format: "currency",
    decimals: 1,
  })

  table.addSectionHeader("Revenue & Operations")
  table.addRow("revenue", {
    cells: { line_item: "Total Revenue", "2023A": 100.0 },
    expr: 'Performance["revenue", col.prev] * 1.15',
    format: "currency",
    decimals: 1,
  })
  table.addRow("cogs", {
    cells: {
      line_item: new Cell({ value: "Cost of Goods Sold", indent: 1 }),
      "2023A": 40.0,
    },
    expr: 'Performance["cogs", col.prev] * 1.10',
    format: "currency",
    decimals: 1,
  })
  table.addRow("gross_profit", {
    cells: { line_item: "Gross Profit" },
    expr: 'Performance["revenue", @col] - Performance["cogs", @col]',
    format: "currency",
    decimals: 1,
  })

  table.addSectionHeader("Other Metrics")
  table.addRow("div_zero", {
    cells: { line_item: "Error Row" },
    expr: "1.0 / 0.0",
  })
  table.addRow("sparse_row", {
    cells: {
      line_item: "Sparse Line",
      "2023A": 10.0,
      "2024E": null,
      "2025E": null,
    },
    format: "currency",
    decimals: 1,
  })

  return model.compile()
}

describe("TypeScript TableAuditor & Tabular Export", () => {
  it("Scenario 1: Table accessor on CompiledModel and metadata introspection", () => {
    const compiled = buildTestModel()

    const table = compiled.table("Performance")
    expect(table).toBeInstanceOf(TableAuditor)
    expect(table.id).toBe("Performance")
    expect(table.title).toBe("Performance")
    expect(table.subtitles).toEqual(["($ in Millions)"])
    expect(table.sheet).toBe("Financials")
    expect(table.columns).toEqual(["line_item", "2023A", "2024E", "2025E"])
    expect(table.measureColumns).toEqual(["2023A", "2024E", "2025E"])
    expect(table.rows).toEqual([
      "revenue",
      "cogs",
      "gross_profit",
      "div_zero",
      "sparse_row",
    ])
    expect(table.shape).toEqual([5, 4])
    expect(table.length).toBe(5)

    expect(() => compiled.table("NonExistent")).toThrow(InspectionError)
  })

  it("Scenario 2: Scalar and vector cell lookups and unboxCellValue", () => {
    const compiled = buildTestModel()
    const table = compiled.table("Performance")

    const rev2024 = table.value("revenue", "2024E")
    expect(typeof rev2024).toBe("number")
    expect(rev2024).toBeCloseTo(115.0)

    const formulaRev2024 = table.formula("revenue", "2024E")
    expect(formulaRev2024).not.toBeNull()
    expect(formulaRev2024?.startsWith("=")).toBe(true)

    expect(table.formula("revenue", "2023A")).toBeNull()

    expect(table.expr("revenue", "2024E")).toBe(
      'Performance["revenue", col.prev] * 1.15'
    )
    expect(table.expr("revenue", "2023A")).toBeNull()

    const addr = table.address("revenue", "2023A")
    expect(typeof addr).toBe("string")
    expect(addr.length).toBeGreaterThan(0)

    const cellInfo = table.cell("revenue", "2024E")
    expect(cellInfo.isFormula).toBe(true)
    expect(cellInfo.formulaExpr).toBe('Performance["revenue", col.prev] * 1.15')
    expect(cellInfo.excelFormula).toBe("=C7*1.15")
    expect(cellInfo.physicalA1).toBe("D7")
    expect(cellInfo.formattedValue).toBe("$115.0")

    expect(table.value("div_zero", "2024E")).toBe("#DIV/0!")
    expect(table.value("sparse_row", "2024E")).toBeNull()

    // Scalar num and formatted
    expect(table.num("revenue", "2024E")).toBeCloseTo(115.0)
    expect(table.formatted("revenue", "2024E")).toBe("$115.0")
    expect(table.formatted("revenue", "2024E", { unit: "M" })).toBe("$115.0M")
    expect(table.formatted("sparse_row", "2024E", { unit: "M" })).toBe("")

    // Negative accounting parens unit splicing
    expect(formatWithUnit("($50.0)", "M")).toBe("($50.0M)")
    expect(formatWithUnit("$50.0", "M")).toBe("$50.0M")
    expect(formatWithUnit("$50.0")).toBe("$50.0")
    expect(formatWithUnit("", "M")).toBe("")

    const negModel = new Model()
    const negSheet = negModel.addSheet("S")
    const negTable = negSheet.addTable("Neg", "Neg", { currency: "USD" })
    negTable.addColumn("v", { header: "V", format: "currency", decimals: 1 })
    negTable.addRow("loss", { cells: { v: -50.0 } })
    negTable.addRow("flag", { cells: { v: true } })
    const negAuditor = negModel.compile().table("Neg")
    expect(negAuditor.formatted("loss", "v")).toBe("($50.0)")
    expect(negAuditor.formatted("loss", "v", { unit: "M" })).toBe("($50.0M)")
    expect(() => negAuditor.num("flag", "v")).toThrow(TypeError)

    // 1D rowValues and rowNums
    const revVals = table.rowValues("revenue")
    expect(revVals).toHaveLength(3)
    expect(revVals[0]).toBeCloseTo(100.0)
    expect(revVals[1]).toBeCloseTo(115.0)
    expect(revVals[2]).toBeCloseTo(132.25)

    expect(table.rowValues("revenue", ["line_item", "2023A"])).toEqual([
      "Total Revenue",
      100.0,
    ])
    expect(table.rowValues("sparse_row")).toEqual([10.0, null, null])

    const revNums = table.rowNums("revenue")
    expect(revNums).toHaveLength(3)
    expect(revNums[0]).toBeCloseTo(100.0)
    expect(revNums[1]).toBeCloseTo(115.0)
    expect(revNums[2]).toBeCloseTo(132.25)

    const revSubNums = table.rowNums("revenue", ["2023A", "2024E"])
    expect(revSubNums).toHaveLength(2)
    expect(revSubNums[0]).toBeCloseTo(100.0)
    expect(revSubNums[1]).toBeCloseTo(115.0)

    // Unknown coordinates, rows, or columns throw Error
    expect(() => table.cell("unknown_row", "2024E")).toThrow(
      "Coordinate ('unknown_row', '2024E') not found"
    )
    expect(() => table.value("unknown_row", "2024E")).toThrow(
      "Coordinate ('unknown_row', '2024E') not found"
    )
    expect(() => table.num("unknown", "2024E")).toThrow(
      "Coordinate ('unknown', '2024E') not found"
    )
    expect(() => table.formatted("unknown", "2024E")).toThrow(
      "Coordinate ('unknown', '2024E') not found"
    )
    expect(() => table.rowValues("unknown_row")).toThrow(
      "Row 'unknown_row' not found"
    )
    expect(() => table.rowValues("revenue", ["unknown_col"])).toThrow(
      "Coordinate ('revenue', 'unknown_col') not found"
    )
    expect(() => table.rowNums("unknown_row")).toThrow(
      "Row 'unknown_row' not found"
    )
    expect(() => table.rowNums("revenue", ["unknown_col"])).toThrow(
      "Coordinate ('revenue', 'unknown_col') not found"
    )

    // Non-numeric cells throw TypeError in num() and rowNums()
    expect(() => table.num("revenue", "line_item")).toThrow(TypeError)
    expect(() => table.num("sparse_row", "2024E")).toThrow(TypeError)
    expect(() => table.num("div_zero", "2024E")).toThrow(TypeError)
    expect(() => table.rowNums("sparse_row")).toThrow(TypeError)
    expect(() => table.rowNums("div_zero")).toThrow(TypeError)

    // unboxCellValue variants and raw primitives
    expect(unboxCellValue({ Number: 100.0 })).toBe(100.0)
    expect(unboxCellValue({ Number: 42 })).toBe(42.0)
    expect(unboxCellValue({ Text: "Hello" })).toBe("Hello")
    expect(unboxCellValue({ Boolean: true })).toBe(true)
    expect(unboxCellValue({ Boolean: false })).toBe(false)
    expect(unboxCellValue("Empty")).toBeNull()
    expect(unboxCellValue("empty")).toBeNull()
    expect(unboxCellValue({ Empty: null })).toBeNull()
    expect(unboxCellValue(null)).toBeNull()
    expect(unboxCellValue(undefined)).toBeNull()
    expect(unboxCellValue(123.45)).toBe(123.45)
    expect(unboxCellValue("raw text")).toBe("raw text")
    expect(unboxCellValue(true)).toBe(true)

    expect(unboxCellValue({ Error: "Div0" })).toBe("#DIV/0!")
    expect(unboxCellValue({ Error: "Value" })).toBe("#VALUE!")
    expect(unboxCellValue({ Error: "Ref" })).toBe("#REF!")
    expect(unboxCellValue({ Error: "Num" })).toBe("#NUM!")
    expect(unboxCellValue({ Error: "Na" })).toBe("#N/A")
    expect(unboxCellValue({ Error: "Name" })).toBe("#NAME?")
  })

  it("Scenario 3: Tabular dictionary exports (toDict)", () => {
    const compiled = buildTestModel()
    const table = compiled.table("Performance")

    // Default and explicit records orient
    const defaultRecords = table.toDict()
    const records = table.toDict("records")
    expect(defaultRecords).toEqual(records)
    expect(Array.isArray(records)).toBe(true)
    expect(records).toHaveLength(5)

    for (const rowDict of records) {
      expect(Object.keys(rowDict)).toEqual(table.columns)
    }

    const revRow = records[0]
    expect(revRow?.line_item).toBe("Total Revenue")
    expect(revRow?.["2023A"]).toBe(100.0)
    expect(revRow?.["2024E"]).toBeCloseTo(115.0)

    const sparseRow = records[4]
    expect(sparseRow?.line_item).toBe("Sparse Line")
    expect(sparseRow?.["2023A"]).toBe(10.0)
    expect(sparseRow?.["2024E"]).toBeNull()
    expect(sparseRow?.["2025E"]).toBeNull()

    // dict orient
    const colDict = table.toDict("dict")
    expect(Object.keys(colDict)).toEqual([
      "line_item",
      "2023A",
      "2024E",
      "2025E",
    ])
    for (const colName of table.columns) {
      expect(colDict[colName]).toHaveLength(table.rows.length)
    }
    expect(colDict.line_item?.[0]).toBe("Total Revenue")
    expect(colDict["2023A"]?.[0]).toBe(100.0)
    expect(colDict["2024E"]?.[0]).toBeCloseTo(115.0)

    // index orient
    const indexDict = table.toDict("index")
    expect(Object.keys(indexDict)).toEqual([
      "revenue",
      "cogs",
      "gross_profit",
      "div_zero",
      "sparse_row",
    ])
    expect(indexDict.revenue?.line_item).toBe("Total Revenue")
    expect(indexDict.revenue?.["2024E"]).toBeCloseTo(115.0)
    expect(indexDict.sparse_row?.["2025E"]).toBeNull()

    // split orient
    const splitRes: SplitDict = table.toDict("split")
    expect(splitRes.columns).toEqual(["line_item", "2023A", "2024E", "2025E"])
    expect(splitRes.index).toEqual([
      "revenue",
      "cogs",
      "gross_profit",
      "div_zero",
      "sparse_row",
    ])
    expect(splitRes.data).toHaveLength(5)
    expect(splitRes.data[0]).toHaveLength(4)
    expect(splitRes.data[0]?.[0]).toBe("Total Revenue")
    expect(splitRes.data[0]?.[1]).toBe(100.0)
    expect(splitRes.data[0]?.[2]).toBeCloseTo(115.0)
  })

  it("Scenario 4: GitHub-Flavored Markdown rendering (toMarkdown)", () => {
    const compiled = buildTestModel()
    const table = compiled.table("Performance")

    const mdValues = table.toMarkdown("values")
    expect(typeof mdValues).toBe("string")
    expect(mdValues.endsWith("\n")).toBe(true)
    expect(table.toMarkdown()).toBe(mdValues)
    expect(table.toMarkdown({ view: "values" })).toBe(mdValues)

    const lines = mdValues.split("\n").filter((l) => l.length > 0)
    expect(lines[0]).toContain("Line Item")
    expect(lines[0]).toContain("2023A")
    expect(lines[0]).toContain("2024E")
    expect(lines[1]).toContain(":---")
    expect(lines[1]).toContain("---:")

    expect(
      lines.some((line) => line.includes("**Revenue & Operations**"))
    ).toBe(true)
    expect(lines.some((line) => line.includes("**Other Metrics**"))).toBe(true)
    expect(lines.some((line) => line.includes("  Cost of Goods Sold"))).toBe(
      true
    )
    expect(lines.some((line) => line.includes("$100.0"))).toBe(true)
    expect(lines.some((line) => line.includes("$115.0"))).toBe(true)
    expect(
      lines.some((line) => line.includes("Sparse Line") && line.includes("—"))
    ).toBe(true)

    // Formulas mode
    const mdFormulas = table.toMarkdown("formulas")
    expect(table.toMarkdown({ view: "formulas" })).toBe(mdFormulas)
    expect(
      mdFormulas
        .split("\n")
        .some((line) => line.includes("=C") || line.includes("=B"))
    ).toBe(true)

    // Expr mode
    const mdExpr = table.toMarkdown("expr")
    expect(table.toMarkdown({ view: "expr" })).toBe(mdExpr)
    expect(
      mdExpr
        .split("\n")
        .some(
          (line) => line.includes("Performance") && line.includes("col.prev")
        )
    ).toBe(true)

    // Addresses mode
    const mdAddrs = table.toMarkdown("addresses")
    expect(table.toMarkdown({ view: "addresses" })).toBe(mdAddrs)
    expect(
      mdAddrs
        .split("\n")
        .some((line) => line.includes("C7") || line.includes("D7"))
    ).toBe(true)

    // Multi-tier headers, subtotal/total rows, subsetting, and showIds
    const model = new Model({
      name: "ThreeStatementModel",
      theme: "corporate_navy",
    })
    const sheet = model.addSheet("Model")
    const inc = sheet.addTable("IncomeStatement", "Income Statement", {
      currency: "USD",
    })
    inc.addColumn("line_item", { role: "descriptor", header: "Line Item" })
    inc.addColumn("y2023a", {
      role: "measure",
      group: "Historical Actuals",
      header: "2023A",
      format: "currency",
      decimals: 1,
    })
    inc.addColumnGutter()
    inc.addColumn("y2024e", {
      role: "measure",
      group: "Forecast Projections",
      header: "2024E",
      format: "currency",
      decimals: 1,
    })
    inc.addColumn("y2025e", {
      role: "measure",
      group: "Forecast Projections",
      header: "2025E",
      format: "currency",
      decimals: 1,
    })

    inc.addSectionHeader("Operating Expenses")
    inc.addRow("revenue", {
      cells: { line_item: "Revenue", y2023a: 100.0 },
      expr: 'IncomeStatement["revenue", col.prev] * 1.10',
      format: "currency",
      decimals: 1,
    })
    inc.addRowGutter()
    inc.addRow("sga", {
      cells: {
        line_item: new Cell({ value: "Salaries & SG&A Overhead", indent: 1 }),
        y2023a: 30.0,
      },
      expr: 'IncomeStatement["sga", col.prev] * 1.05',
      format: "currency",
      decimals: 1,
    })
    inc.addRow("net_income", {
      cells: { line_item: "Net Income" },
      expr: 'IncomeStatement["revenue", @col] - IncomeStatement["sga", @col]',
      style: "total",
      format: "currency",
      decimals: 1,
    })

    const isTable = model.compile().table("IncomeStatement")
    expect(isTable.sheet).toBe("Model")
    expect(isTable.shape).toEqual([3, 4])
    expect(isTable.columns).toEqual(["line_item", "y2023a", "y2024e", "y2025e"])
    expect(isTable.measureColumns).toEqual(["y2023a", "y2024e", "y2025e"])
    expect(isTable.rows).toEqual(["revenue", "sga", "net_income"])

    const md = isTable.toMarkdown("values")
    const isLines = md.split("\n").filter((l) => l.length > 0)
    expect(isLines[0]).toContain("Historical Actuals: 2023A")
    expect(isLines[0]).toContain("Forecast Projections: 2024E")
    expect(isLines[1]).toContain(":---")
    expect(isLines[1]).toContain("---:")
    expect(md).not.toContain("[y2023a]")
    expect(md).not.toContain("[net_income]")
    expect(md).toContain("**Operating Expenses**")
    expect(md).toContain("  Salaries & SG&A Overhead")
    expect(md).toContain("**Net Income**")
    expect(md).toContain("**$70.0**")

    // Subsetting rows and cols (retaining descriptor column and omitting section headers by default)
    const mdSub = isTable.toMarkdown({
      rows: ["revenue", "net_income"],
      cols: ["y2023a", "y2025e"],
    })
    const subLines = mdSub.split("\n").filter((l) => l.length > 0)
    expect(subLines).toHaveLength(4)
    expect(subLines[0]).toContain("Line Item")
    expect(subLines[0]).toContain("Historical Actuals: 2023A")
    expect(subLines[0]).toContain("Forecast Projections: 2025E")
    expect(mdSub).not.toContain("2024E")
    expect(mdSub).not.toContain("**Operating Expenses**")
    expect(mdSub).not.toContain("Salaries")

    // Subsetting rows with explicit includeSectionHeaders: true (both overload forms)
    const mdSubSec = isTable.toMarkdown({
      rows: ["revenue", "net_income"],
      cols: ["y2023a", "y2025e"],
      includeSectionHeaders: true,
    })
    expect(mdSubSec).toContain("**Operating Expenses**")
    expect(
      isTable.toMarkdown("values", {
        rows: ["revenue", "net_income"],
        cols: ["y2023a", "y2025e"],
        includeSectionHeaders: true,
      })
    ).toBe(mdSubSec)

    // Multi-section subsetting with includeSectionHeaders: true omits orphaned section headers
    const multiSecModel = new Model({
      name: "MultiSec",
      theme: "corporate_navy",
    })
    const multiSecSheet = multiSecModel.addSheet("Sheet1")
    const multiSecTbl = multiSecSheet.addTable("MultiSecTable", "Multi Section")
    multiSecTbl.addColumn("line_item", { role: "descriptor", header: "Metric" })
    multiSecTbl.addColumn("val", { role: "measure", header: "Value" })
    multiSecTbl.addSectionHeader("Section 1: Empty Match")
    multiSecTbl.addRow("r1", { cells: { line_item: "Row 1", val: 10.0 } })
    multiSecTbl.addSectionHeader("Section 2: Active Match")
    multiSecTbl.addRow("r2", { cells: { line_item: "Row 2", val: 20.0 } })
    multiSecTbl.addSectionHeader("Section 3: Trailing Empty Match")
    multiSecTbl.addRow("r3", { cells: { line_item: "Row 3", val: 30.0 } })
    const mdFilteredSec = multiSecModel
      .compile()
      .table("MultiSecTable")
      .toMarkdown({
        rows: ["r2"],
        includeSectionHeaders: true,
      })
    expect(mdFilteredSec).not.toContain("**Section 1: Empty Match**")
    expect(mdFilteredSec).toContain("**Section 2: Active Match**")
    expect(mdFilteredSec).toContain("Row 2")
    expect(mdFilteredSec).not.toContain("**Section 3: Trailing Empty Match**")

    // Unknown row or unknown col in toMarkdown throws Error
    expect(() => isTable.toMarkdown({ rows: ["unknown_row"] })).toThrow(
      "Row 'unknown_row' not found"
    )
    expect(() => isTable.toMarkdown({ cols: ["unknown_col"] })).toThrow(
      "Column 'unknown_col' not found"
    )

    // showIds: true appends [col_id] and [row_id] when display text differs from ID
    const mdIds = isTable.toMarkdown({ showIds: true })
    expect(mdIds).toContain("Historical Actuals: 2023A [y2023a]")
    expect(mdIds).toContain("Net Income [net_income]")

    // Group + multi-line stacked headers and ungrouped multi-line headers
    const lboModel = new Model({ name: "LboHeaders", theme: "corporate_navy" })
    const lboSheet = lboModel.addSheet("LBO")
    const lboTbl = lboSheet.addTable("Schedule", "LBO Schedule")
    lboTbl.addColumn("label", { role: "descriptor", header: "($ in millions)" })
    lboTbl.addColumn("mrq", {
      group: "Historical",
      headers: ["MRQ", "2012-03-31"],
    })
    lboTbl.addColumn("standalone", {
      headers: ["Standalone", "at Close", "2012-12-31"],
    })
    lboTbl.addColumn("group_only", { group: "Summary" })
    lboTbl.addRow("rev", {
      cells: {
        label: "Revenue",
        mrq: 100.0,
        standalone: 110.0,
        group_only: 120.0,
      },
    })
    const lboMd = lboModel.compile().table("Schedule").toMarkdown()
    const lboHeaderLine = lboMd.split("\n")[0]
    expect(lboHeaderLine).toContain("Historical: MRQ 2012-03-31")
    expect(lboHeaderLine).toContain("Standalone at Close 2012-12-31")
    expect(lboHeaderLine).toContain("Summary")
  })

  it("Scenario 5: row/col IDs with comma-quote and GFM Markdown pipe/newline escaping", () => {
    const model = new Model({ name: "SpecialChars", theme: "corporate_navy" })
    const sheet = model.addSheet("Sheet1")
    const tbl = sheet.addTable("T", "Special Table")
    const rowKey = 'rev", "sub'
    const colKey = 'val", "c'
    tbl.addColumn("label", {
      role: "descriptor",
      header: "Item | Name\nLine 2",
    })
    tbl.addColumn(colKey, { role: "measure", header: "Value | Col" })
    tbl.addSectionHeader("Sec | Header\nSub")
    tbl.addRow(rowKey, {
      cells: {
        label: "A | B\nC",
        [colKey]: 42.0,
      },
    })

    const compiled = model.compile()
    const auditor = compiled.table("T")

    const cellInfo = auditor.cell(rowKey, colKey)
    expect(cellInfo.row).toBe(rowKey)
    expect(cellInfo.col).toBe(colKey)
    expect(auditor.value(rowKey, colKey)).toBe(42.0)
    expect(auditor.toDict("index")[rowKey]?.[colKey]).toBe(42.0)

    const md = auditor.toMarkdown()
    const lines = md.split("\n").filter((l) => l.length > 0)
    expect(lines).toHaveLength(4)
    expect(lines[0]).toContain("Item \\| Name Line 2")
    expect(lines[0]).toContain("Value \\| Col")
    expect(lines[2]).toContain("**Sec \\| Header Sub**")
    expect(lines[3]).toContain("A \\| B C")

    const untypedAuditor: {
      toDict(orient: string): unknown
      toMarkdown(view: string): string
    } = auditor
    expect(() => untypedAuditor.toDict("invalid")).toThrow(/Invalid orient/)
    expect(() => untypedAuditor.toMarkdown("invalid")).toThrow(
      /Invalid view mode/
    )
  })
})

function buildParityModel(): CompiledModel {
  const model = new Model({ name: "ParityModel", theme: "corporate_navy" })

  const fin = model.addSheet("Financials")
  const inc = fin.addTable("IncomeStatement", "Income Statement", {
    subtitle: ["($ in Millions)", "Fiscal Year"],
    currency: "USD",
  })
  inc.addColumn("line_item", { role: "descriptor", header: "Line Item" })
  inc.addColumn("y2023a", {
    role: "measure",
    group: "Historical",
    header: "2023A",
    format: "currency",
    decimals: 1,
  })
  inc.addColumnGutter()
  inc.addColumn("2024E", {
    role: "measure",
    header: "2024E",
    format: "currency",
    decimals: 1,
  })

  inc.addSectionHeader("Revenue & Margin")
  inc.addRow("revenue", {
    cells: {
      line_item: "Revenue",
      y2023a: 100.0,
      "2024E": new Cell({
        expr: 'IncomeStatement["revenue", col.prev] * 1.15',
      }),
    },
  })
  inc.addRow("cogs", {
    cells: {
      line_item: new Cell({ value: "Cost | Goods\nSold", indent: 1 }),
      y2023a: 40.0,
      "2024E": 44.0,
    },
  })
  inc.addRow("sparse", {
    cells: { line_item: "Memo", y2023a: 5.0, "2024E": null },
  })
  inc.addRowGutter()
  inc.addRow("gross_profit", {
    style: "subtotal",
    cells: { line_item: "Gross Profit" },
    expr: 'IncomeStatement["revenue", @col] - IncomeStatement["cogs", @col]',
  })
  inc.addSectionHeader("Checks | Errors")
  inc.addRow("div_err", {
    style: "total",
    cells: {
      line_item: "Net | Check",
      y2023a: 10.0,
      "2024E": new Cell({ expr: "1 / 0" }),
    },
  })

  const pure = fin.addTable("PureMeasures", "PureMeasures")
  pure.addColumn("c1", { role: "measure", header: "c1" })
  pure.addColumn("c2", { role: "measure", header: "Col Two" })
  pure.addRow("r1", { cells: { c1: 1.0, c2: new Cell({ expr: "NA()" }) } })
  pure.addRow("r2", { style: "total", cells: { c1: 2.0, c2: 3.0 } })

  const val = model.addSheet("Valuation")
  const comps = val.addTable("Comps", "Trading Comps", { currency: "USD" })
  comps.addColumn("ticker", { role: "descriptor", header: "Ticker" })
  comps.addColumn("company", { role: "descriptor", header: "Company Name" })
  comps.addColumn("ev", {
    role: "measure",
    header: "EV",
    format: "currency",
    decimals: 1,
  })
  comps.addRow("aapl", {
    cells: { ticker: "AAPL", company: "Apple Inc.", ev: 3000.0 },
  })
  comps.addRow("msft_row", {
    cells: {
      ticker: "MSFT",
      company: "Microsoft",
      ev: new Cell({ expr: "LN(-1)" }),
    },
  })

  model.addSheet("Notes")
  return model.compile()
}

const EXPECTED_PARITY_VALUES_MD = [
  "# ParityModel",
  "Sheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3",
  "",
  "## Runtime Errors",
  "",
  '- `IncomeStatement["div_err", "2024E"]` (`\'Financials\'!E15`): `#DIV/0!` (`=1/0`)',
  '- `PureMeasures["r1", "c2"]` (`\'Financials\'!C21`): `#N/A` (`=NA()`)',
  '- `Comps["msft_row", "ev"]` (`\'Valuation\'!D6`): `#NUM!` (`=LN(-1)`)',
  "",
  "## Sheet: Financials",
  "",
  "### `IncomeStatement` — Income Statement",
  "_($ in Millions)_",
  "_Fiscal Year_",
  "",
  "| Line Item [line_item] | Historical: 2023A [y2023a] | 2024E |",
  "| :--- | ---: | ---: |",
  "| **Revenue & Margin** | | |",
  "| Revenue | $100.0 | $115.0 |",
  "|   Cost \\| Goods Sold [cogs] | $40.0 | $44.0 |",
  "| Memo [sparse] | $5.0 | — |",
  "| **Gross Profit [gross_profit]** | **$60.0** | **$71.0** |",
  "| **Checks \\| Errors** | | |",
  "| **Net \\| Check [div_err]** | **$10.0** | **#DIV/0!** |",
  "",
  "### `PureMeasures`",
  "",
  "| Row | c1 | Col Two [c2] |",
  "| :--- | ---: | ---: |",
  "| r1 | 1 | #N/A |",
  "| **r2** | **2** | **3** |",
  "",
  "## Sheet: Valuation",
  "",
  "### `Comps` — Trading Comps",
  "",
  "| Ticker | Company Name [company] | EV |",
  "| :--- | :--- | ---: |",
  "| AAPL | Apple Inc. | $3,000.0 |",
  "| MSFT [msft_row] | Microsoft | #NUM! |",
  "",
  "## Sheet: Notes",
  "",
  "_(no tables)_",
  "",
].join("\n")

const EXPECTED_PARITY_INDEX_MD = [
  "# ParityModel",
  "Sheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3",
  "",
  "## Runtime Errors",
  "",
  '- `IncomeStatement["div_err", "2024E"]` (`\'Financials\'!E15`): `#DIV/0!` (`=1/0`)',
  '- `PureMeasures["r1", "c2"]` (`\'Financials\'!C21`): `#N/A` (`=NA()`)',
  '- `Comps["msft_row", "ev"]` (`\'Valuation\'!D6`): `#NUM!` (`=LN(-1)`)',
  "",
  "## Sheet: Financials",
  "",
  "### `IncomeStatement` — Income Statement",
  "_($ in Millions)_",
  "_Fiscal Year_",
  "",
  "Columns: `line_item` (Line Item, descriptor), `y2023a` (Historical: 2023A), `2024E`",
  "Rows:",
  "- **Revenue & Margin**",
  "  - `revenue`",
  "  - `cogs`: Cost | Goods Sold",
  "  - `sparse`: Memo",
  "  - **`gross_profit`**: Gross Profit (subtotal)",
  "- **Checks | Errors**",
  "  - **`div_err`**: Net | Check (total)",
  "",
  "### `PureMeasures`",
  "",
  "Columns: `c1`, `c2` (Col Two)",
  "Rows:",
  "- `r1`",
  "- **`r2`** (total)",
  "",
  "## Sheet: Valuation",
  "",
  "### `Comps` — Trading Comps",
  "",
  "Columns: `ticker` (descriptor), `company` (Company Name, descriptor), `ev`",
  "Rows:",
  "- `aapl`: AAPL | Apple Inc.",
  "- `msft_row`: MSFT | Microsoft",
  "",
  "## Sheet: Notes",
  "",
  "_(no tables)_",
  "",
].join("\n")

const EXPECTED_PARITY_LAYOUT_MD = [
  "# ParityModel",
  "Sheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3",
  "",
  "## Runtime Errors",
  "",
  '- `IncomeStatement["div_err", "2024E"]` (`\'Financials\'!E15`): `#DIV/0!` (`=1/0`)',
  '- `PureMeasures["r1", "c2"]` (`\'Financials\'!C21`): `#N/A` (`=NA()`)',
  '- `Comps["msft_row", "ev"]` (`\'Valuation\'!D6`): `#NUM!` (`=LN(-1)`)',
  "",
  "## Sheet: Financials",
  "Layout: auto | Bounds: B2:E22 (21r x 4c, w:40)",
  "Columns: A:2, B:14, C:12, D:2, E:12",
  "",
  "- Band 1 (R2:R15, 14r x cols B..E, w:40):",
  "  - `IncomeStatement` [B2:E15] 14r x 4c (w:40) | stack | sub:2 hdr:R6:R7(grouped) body:R8:R15 | cols: desc B(1c), measure C(1c), gutter D(1c), measure E(1c) | wrap:1",
  "- Band 2 (R18:R22, 5r x cols B..C, w:26, gap:2r, right_empty:D..E (2c, w:14)):",
  "  - `PureMeasures` [B18:C22] 5r x 2c (w:26) | stack | title->E (+2 empty) hdr:R20 body:R21:R22 | cols: measure B..C(2c)",
  "",
  "## Sheet: Valuation",
  "Layout: auto | Bounds: B2:D6 (5r x 3c, w:36)",
  "Columns: A:2, B..D:12",
  "",
  "- Band 1 (R2:R6, 5r x cols B..D, w:36):",
  "  - `Comps` [B2:D6] 5r x 3c (w:36) | stack | hdr:R4 body:R5:R6 | cols: desc B..C(2c), measure D(1c)",
  "",
  "## Sheet: Notes",
  "",
  "_(no tables)_",
  "",
].join("\n")

describe("CompiledModel runtime error audit & workbook Markdown", () => {
  it("Scenarios 1-2: errors() on clean and multi-sheet errored workbooks", () => {
    const cleanModel = new Model({ name: "CleanModel" })
    const cleanSheet = cleanModel.addSheet("Summary")
    const cleanTbl = cleanSheet.addTable("KPIs", "KPIs")
    cleanTbl.addColumn("metric", { role: "descriptor", header: "Metric" })
    cleanTbl.addColumn("val", { role: "measure", header: "Value" })
    cleanTbl.addRow("rev", { cells: { metric: "Revenue", val: 100.0 } })
    const clean = cleanModel.compile()
    expect(clean.errors()).toEqual([])
    const cleanMd = clean.toMarkdown()
    expect(cleanMd).toContain("| Errors: 0\n")
    expect(cleanMd).not.toContain("## Runtime Errors")

    const compiled = buildParityModel()
    const errs: CellError[] = compiled.errors()
    expect(errs).toEqual([
      {
        coordinate: {
          table: "IncomeStatement",
          row: "div_err",
          col: "2024E",
        },
        sheet: "Financials",
        physicalAddress: "'Financials'!E15",
        error: "#DIV/0!",
        formulaExpr: "1 / 0",
        excelFormula: "=1/0",
      },
      {
        coordinate: { table: "PureMeasures", row: "r1", col: "c2" },
        sheet: "Financials",
        physicalAddress: "'Financials'!C21",
        error: "#N/A",
        formulaExpr: "NA()",
        excelFormula: "=NA()",
      },
      {
        coordinate: { table: "Comps", row: "msft_row", col: "ev" },
        sheet: "Valuation",
        physicalAddress: "'Valuation'!D6",
        error: "#NUM!",
        formulaExpr: "LN(-1)",
        excelFormula: "=LN(-1)",
      },
    ])
  })

  it("Scenarios 3-8 & 10: toMarkdown values, index, filters, validation, and cross-language parity", () => {
    const compiled = buildParityModel()

    // Default values mode and overload parity
    expect(compiled.toMarkdown()).toBe(EXPECTED_PARITY_VALUES_MD)
    expect(compiled.toMarkdown({ detail: "values", showIds: true })).toBe(
      EXPECTED_PARITY_VALUES_MD
    )
    expect(compiled.toMarkdown("values", { showIds: true })).toBe(
      EXPECTED_PARITY_VALUES_MD
    )

    // Index mode (ignores showIds)
    expect(compiled.toMarkdown("index")).toBe(EXPECTED_PARITY_INDEX_MD)
    expect(compiled.toMarkdown({ detail: "index", showIds: false })).toBe(
      EXPECTED_PARITY_INDEX_MD
    )

    // Layout mode: cross-language byte-identical parity & determinism (ignores showIds)
    expect(compiled.toMarkdown("layout")).toBe(EXPECTED_PARITY_LAYOUT_MD)
    expect(compiled.toMarkdown({ detail: "layout", showIds: false })).toBe(
      EXPECTED_PARITY_LAYOUT_MD
    )
    expect(compiled.toMarkdown("layout", { showIds: false })).toBe(
      EXPECTED_PARITY_LAYOUT_MD
    )

    // Partial tables filter in layout mode tags unselected siblings (context) and scopes errors
    const mdLayoutPartial = compiled.toMarkdown("layout", {
      tables: ["PureMeasures"],
    })
    expect(mdLayoutPartial).toContain("## Sheet: Financials\n")
    expect(mdLayoutPartial).not.toContain("## Sheet: Valuation")
    expect(mdLayoutPartial).not.toContain("## Sheet: Notes")
    expect(mdLayoutPartial).toContain(
      "  - `IncomeStatement` (context) [B2:E15]"
    )
    expect(mdLayoutPartial).toContain("  - `PureMeasures` [B18:C22]")
    expect(mdLayoutPartial).not.toContain("`PureMeasures` (context)")
    expect(mdLayoutPartial).toContain('PureMeasures["r1", "c2"]')
    expect(mdLayoutPartial).not.toContain('IncomeStatement["div_err", "2024E"]')

    // showIds: false omits synthetic Row column and inline IDs
    const mdNoIds = compiled.toMarkdown("values", { showIds: false })
    expect(mdNoIds).not.toContain("| Row |")
    expect(mdNoIds).toContain("| c1 | Col Two |")
    expect(mdNoIds).not.toContain("[msft_row]")
    expect(mdNoIds).toContain("| MSFT | Microsoft | #NUM! |")

    // Filtered runtime errors & empty sheet
    const mdValOnly = compiled.toMarkdown({ sheets: ["Valuation"] })
    expect(mdValOnly).toContain("| Errors: 3\n")
    expect(mdValOnly).toContain("## Runtime Errors")
    expect(mdValOnly).toContain('Comps["msft_row", "ev"]')
    expect(mdValOnly).not.toContain("IncomeStatement")

    const mdNotesOnly = compiled.toMarkdown({ sheets: ["Notes"] })
    expect(mdNotesOnly).toContain("| Errors: 3\n")
    expect(mdNotesOnly).not.toContain("## Runtime Errors")
    expect(mdNotesOnly).toContain("## Sheet: Notes\n\n_(no tables)_\n")

    // Union + dedup + canonical declaration order
    const mdUnion = compiled.toMarkdown({
      sheets: ["Valuation", "Valuation"],
      tables: ["Comps", "PureMeasures", "PureMeasures"],
    })
    expect(mdUnion.split("### `Comps`").length - 1).toBe(1)
    expect(mdUnion.split("### `PureMeasures`").length - 1).toBe(1)
    expect(mdUnion).not.toContain("### `IncomeStatement`")
    expect(mdUnion.indexOf("## Sheet: Financials")).toBeLessThan(
      mdUnion.indexOf("## Sheet: Valuation")
    )

    // Empty allowlists emit only the model header
    const expectedHeaderOnly =
      "# ParityModel\nSheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3\n"
    expect(compiled.toMarkdown({ sheets: [] })).toBe(expectedHeaderOnly)
    expect(compiled.toMarkdown({ tables: [] })).toBe(expectedHeaderOnly)
    expect(compiled.toMarkdown({ sheets: [], tables: [] })).toBe(
      expectedHeaderOnly
    )
    expect(compiled.toMarkdown("layout", { sheets: [] })).toBe(
      expectedHeaderOnly
    )
    expect(compiled.toMarkdown("layout", { tables: [] })).toBe(
      expectedHeaderOnly
    )

    // Strict sheet/table not-found errors
    expect(() => compiled.toMarkdown({ sheets: ["UnknownSheet"] })).toThrow(
      InspectionError
    )
    expect(() => compiled.toMarkdown({ tables: ["UnknownTable"] })).toThrow(
      InspectionError
    )
    expect(() =>
      compiled.toMarkdown("layout", { sheets: ["UnknownSheet"] })
    ).toThrow(InspectionError)
    expect(() =>
      compiled.toMarkdown("layout", { tables: ["UnknownTable"] })
    ).toThrow(InspectionError)

    // Scenario 8: Invalid detail throws plain Error (NOT F8Error)
    const untypedModel: {
      toMarkdown(arg: string | { detail: string }): string
    } = compiled
    for (const badArg of ["summary", { detail: "unknown" }]) {
      let caught: unknown = null
      try {
        untypedModel.toMarkdown(badArg)
      } catch (err: unknown) {
        caught = err
      }
      expect(caught).toBeInstanceOf(Error)
      expect(caught).not.toBeInstanceOf(F8Error)
      const expectedDetail = typeof badArg === "string" ? badArg : badArg.detail
      if (caught instanceof Error) {
        expect(caught.message).toBe(
          `Invalid detail mode '${expectedDetail}'. Expected one of 'values', 'index', 'layout'.`
        )
      }
    }

    // Disposed model throws F8Error before detail validation
    compiled.dispose()
    expect(() => compiled.errors()).toThrow(F8Error)
    expect(() => untypedModel.toMarkdown("summary")).toThrow(F8Error)
  })

  it('toMarkdown("layout") grid lanes, width inflation, and auto chrome metadata', () => {
    const model = new Model({ name: "LayoutEndToEnd" })
    const dash = model.addSheet("Dashboard", {
      layout: "grid",
      colWidth: 12.0,
    })

    const t1 = dash.addTable("T1", "Top Left")
    t1.addColumn("item", { role: "descriptor", header: "Item" })
    t1.addColumn("v1", { header: "V1", format: "currency", decimals: 1 })
    t1.addRow("r1", { cells: { item: "Revenue", v1: 123_456_789.0 } })

    const t2 = dash.addTable("T2", "Bottom Left", { below: t1 })
    t2.addColumn("item", { role: "descriptor", header: "Item" })
    t2.addColumn("v1", { header: "V1", format: "currency", decimals: 1 })
    t2.addRow("r1", { cells: { item: "Cost", v1: 50.0 } })

    const t3 = dash.addTable("T3", "Right Tall", { rightOf: t1 })
    t3.addColumn("item", { role: "descriptor", header: "Item" })
    t3.addColumn("v1", { header: "V1" })
    for (let idx = 1; idx <= 11; idx++) {
      t3.addRow(`r${idx}`, { cells: { item: `Row ${idx}`, v1: idx } })
    }

    const autoSheet = model.addSheet("Detail", {
      freezePanes: { row: 6, col: 2 },
      showGridlines: true,
      tabColor: "#1F4E79",
    })
    const tbl = autoSheet.addTable("Notes", "Notes Table", {
      subtitle: ["Line 1", "Line 2"],
      collapsible: true,
    })
    tbl.addColumn("item", { role: "descriptor", header: "Item" })
    tbl.addColumn("val", { group: "Grp", header: "Val" })
    tbl.addRow("r1", { cells: { item: "Multi\nLine", val: 10.0 } })

    const oneRow = autoSheet.addTable("Single", "", { collapsible: true })
    oneRow.addColumn("v")
    oneRow.addRow("r1", { cells: { v: 1.0 } })

    const compiled = model.compile()
    try {
      const md = compiled.toMarkdown("layout")
      expect(md).toContain("Layout: grid (W=15, base=12)")
      expect(md).toContain("  - Lane B..C (")
      expect(md).toContain("empty_below:")
      expect(md).toContain("    - `T1` ")
      expect(md).toContain("    - `T2` ")
      expect(md).toContain("    - `T3` ")
      expect(md).toContain(
        "| Freeze: C7 (R1:R6, A..B) | Gridlines: on | Tab: #1F4E79\n"
      )
      expect(md).toContain("sub:2 hdr:")
      expect(md).toContain("(grouped)")
      expect(md).toContain("collapsible | cols:")
      expect(md).toContain("| wrap:1\n")
      const singleLine = md
        .split("\n")
        .find((line) => line.includes("`Single`"))
      expect(singleLine).toBeDefined()
      expect(singleLine).toContain("untitled hdr:none body:")
      expect(singleLine).not.toContain("collapsible")
    } finally {
      compiled.dispose()
    }

    // Subtitle overflow indicator & grid descriptor req:W (cliff spill + lane-coupling victim)
    const diagModel = new Model({ name: "LayoutDiag" })
    const gridSheet = diagModel.addSheet("Grid", {
      layout: "grid",
      colWidth: 12.0,
    })

    const culprit = gridSheet.addTable("Culprit", "Culprit Table", {
      subtitle:
        "A long single-line subtitle that clearly overflows the narrow panel width",
    })
    culprit.addColumn("item", { role: "descriptor", header: "Item" })
    culprit.addColumn("val", { header: "Val" })
    culprit.addRow("r1", { cells: { item: "A".repeat(24), val: 1.0 } })

    const victim = gridSheet.addTable("Victim", "Victim Table", {
      subtitle: "($ in M)",
    })
    victim.addColumn("item", { role: "descriptor", header: "Item" })
    victim.addColumn("val", { header: "Val" })
    victim.addRow("r1", { cells: { item: "Revenue", val: 2.0 } })

    const diagCompiled = diagModel.compile()
    try {
      const diagMd = diagCompiled.toMarkdown("layout")
      const culpritLine = diagMd
        .split("\n")
        .find((line) => line.includes("`Culprit`"))
      const victimLine = diagMd
        .split("\n")
        .find((line) => line.includes("`Victim`"))
      expect(culpritLine).toContain("sub:1(73ch>w)")
      expect(culpritLine).toContain(
        "cols: desc B..D(3c, req:25), measure E(1c)"
      )
      expect(victimLine).toContain("sub:1 hdr:")
      expect(victimLine).not.toContain("ch>w")
      expect(victimLine).toContain("cols: desc B..D(3c, req:8), measure E(1c)")
    } finally {
      diagCompiled.dispose()
    }
  })

  it("Scenario 9: TableAuditor.toMarkdown compact mode and backward compatibility", () => {
    const compiled = buildParityModel()
    const inc = compiled.table("IncomeStatement")

    const views: readonly TableViewMode[] = [
      "values",
      "formulas",
      "expr",
      "addresses",
    ]
    for (const view of views) {
      const padded = inc.toMarkdown(view)
      const compact = inc.toMarkdown(view, { compact: true })
      expect(padded).toBe(inc.toMarkdown(view, { compact: false }))
      expect(compact.length).toBeLessThan(padded.length)

      const compactLines = compact.split("\n")
      expect(compactLines[1]).toBe("| :--- | ---: | ---: |")
      expect(padded.split("\n")[1]).not.toBe(compactLines[1])
    }

    const compactValues = inc.toMarkdown("values", {
      showIds: true,
      compact: true,
    })
    expect(compactValues).toBe(
      [
        "| Line Item [line_item] | Historical: 2023A [y2023a] | 2024E |",
        "| :--- | ---: | ---: |",
        "| **Revenue & Margin** | | |",
        "| Revenue | $100.0 | $115.0 |",
        "|   Cost \\| Goods Sold [cogs] | $40.0 | $44.0 |",
        "| Memo [sparse] | $5.0 | — |",
        "| **Gross Profit [gross_profit]** | **$60.0** | **$71.0** |",
        "| **Checks \\| Errors** | | |",
        "| **Net \\| Check [div_err]** | **$10.0** | **#DIV/0!** |",
        "",
      ].join("\n")
    )

    const compactFormulas = inc.toMarkdown("formulas", { compact: true })
    expect(compactFormulas).toContain("| Revenue | $100.0 | =C9*1.15 |")
    expect(compactFormulas).toContain("| Memo | $5.0 | |")
  })

  it('supports style: "bold" across rows, columns, cells, and Markdown rendering', () => {
    const model = new Model({ name: "BoldModel" })
    const sheet = model.addSheet("Summary")
    const tbl = sheet.addTable("KPIs", "Key Metrics", { currency: "USD" })
    tbl.addColumn("item", { role: "descriptor", header: "Metric" })
    tbl.addColumn("base", {
      header: "Base Case",
      style: "bold",
    })
    tbl.addColumn("upside", { header: "Upside" })

    tbl.addRow("rev", {
      format: "currency",
      cells: {
        item: "Revenue",
        base: new Cell({ value: 100.0, style: "bold" }),
        upside: 120.0,
      },
    })
    tbl.addRow("margin", {
      style: "bold",
      format: "percentage",
      cells: { item: "EBITDA Margin", base: 0.25, upside: 0.28 },
    })
    tbl.addRow("ebitda", {
      style: "subtotal",
      format: "currency",
      cells: { item: "EBITDA", base: 25.0, upside: 34.0 },
    })
    tbl.addRow("net_income", {
      style: "total",
      format: "currency",
      cells: { item: "Net Income", base: 18.0, upside: 24.0 },
    })

    const compiled = model.compile()
    try {
      const inspected = compiled.inspectTable("KPIs")
      expect(inspected.columns[1]?.style).toBe("bold")
      expect(inspected.rows[1]?.style).toBe("bold")

      const mdValues = compiled.toMarkdown("values", { showIds: false })
      expect(mdValues).toContain("| Revenue | $100 | $120 |\n")
      expect(mdValues).toContain(
        "| **EBITDA Margin** | **25.0%** | **28.0%** |\n"
      )
      expect(mdValues).toContain("| **EBITDA** | **$25** | **$34** |\n")
      expect(mdValues).toContain("| **Net Income** | **$18** | **$24** |\n")

      const mdIndex = compiled.toMarkdown("index")
      expect(mdIndex).toContain("- `margin`: EBITDA Margin\n")
      expect(mdIndex).not.toContain("(bold)")
      expect(mdIndex).not.toContain("**`margin`**")
      expect(mdIndex).toContain("- **`ebitda`** (subtotal)\n")
      expect(mdIndex).toContain("- **`net_income`**: Net Income (total)\n")

      const auditorCompact = compiled
        .table("KPIs")
        .toMarkdown("values", { compact: true })
      expect(auditorCompact).toContain("| Revenue | $100 | $120 |\n")
      expect(auditorCompact).toContain(
        "| **EBITDA Margin** | **25.0%** | **28.0%** |\n"
      )

      const xlsx = compiled.emitWorkbook()
      expect(xlsx[0]).toBe(0x50)
      expect(xlsx[1]).toBe(0x4b)
    } finally {
      compiled.dispose()
    }
  })
})
