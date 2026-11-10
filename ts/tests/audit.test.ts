import { describe, expect, it } from "vitest";
import { formatWithUnit } from "../src/audit.js";
import {
  Cell,
  type CompiledModel,
  InspectionError,
  Model,
  type SplitDict,
  TableAuditor,
  unboxCellValue,
} from "../src/index.js";

function buildTestModel(): CompiledModel {
  const model = new Model({ name: "PerformanceModel", theme: "corporate_navy" });
  const sheet = model.addSheet("Financials");
  const table = sheet.addTable("Performance", "Performance", {
    subtitle: "($ in Millions)",
    currency: "USD",
  });
  table.addColumn("line_item", { role: "descriptor", header: "Line Item" });
  table.addColumn("2023A", { role: "measure", header: "2023A", format: "currency", decimals: 1 });
  table.addColumn("2024E", { role: "measure", header: "2024E", format: "currency", decimals: 1 });
  table.addColumn("2025E", { role: "measure", header: "2025E", format: "currency", decimals: 1 });

  table.addSectionHeader("Revenue & Operations");
  table.addRow("revenue", {
    cells: { line_item: "Total Revenue", "2023A": 100.0 },
    expr: 'Performance["revenue", col.prev] * 1.15',
    format: "currency",
    decimals: 1,
  });
  table.addRow("cogs", {
    cells: {
      line_item: new Cell({ value: "Cost of Goods Sold", indent: 1 }),
      "2023A": 40.0,
    },
    expr: 'Performance["cogs", col.prev] * 1.10',
    format: "currency",
    decimals: 1,
  });
  table.addRow("gross_profit", {
    cells: { line_item: "Gross Profit" },
    expr: 'Performance["revenue", @col] - Performance["cogs", @col]',
    format: "currency",
    decimals: 1,
  });

  table.addSectionHeader("Other Metrics");
  table.addRow("div_zero", {
    cells: { line_item: "Error Row" },
    expr: "1.0 / 0.0",
  });
  table.addRow("sparse_row", {
    cells: { line_item: "Sparse Line", "2023A": 10.0, "2024E": null, "2025E": null },
    format: "currency",
    decimals: 1,
  });

  return model.compile();
}

describe("TypeScript TableAuditor & Tabular Export", () => {
  it("Scenario 1: Table accessor on CompiledModel and metadata introspection", () => {
    const compiled = buildTestModel();

    const table = compiled.table("Performance");
    expect(table).toBeInstanceOf(TableAuditor);
    expect(table.id).toBe("Performance");
    expect(table.title).toBe("Performance");
    expect(table.subtitle).toBe("($ in Millions)");
    expect(table.sheet).toBe("Financials");
    expect(table.columns).toEqual(["line_item", "2023A", "2024E", "2025E"]);
    expect(table.measureColumns).toEqual(["2023A", "2024E", "2025E"]);
    expect(table.rows).toEqual(["revenue", "cogs", "gross_profit", "div_zero", "sparse_row"]);
    expect(table.shape).toEqual([5, 4]);
    expect(table.length).toBe(5);

    expect(() => compiled.table("NonExistent")).toThrow(InspectionError);
  });

  it("Scenario 2: Scalar and vector cell lookups and unboxCellValue", () => {
    const compiled = buildTestModel();
    const table = compiled.table("Performance");

    const rev2024 = table.value("revenue", "2024E");
    expect(typeof rev2024).toBe("number");
    expect(rev2024).toBeCloseTo(115.0);

    const formulaRev2024 = table.formula("revenue", "2024E");
    expect(formulaRev2024).not.toBeNull();
    expect(formulaRev2024?.startsWith("=")).toBe(true);

    expect(table.formula("revenue", "2023A")).toBeNull();

    expect(table.expr("revenue", "2024E")).toBe('Performance["revenue", col.prev] * 1.15');
    expect(table.expr("revenue", "2023A")).toBeNull();

    const addr = table.address("revenue", "2023A");
    expect(typeof addr).toBe("string");
    expect(addr.length).toBeGreaterThan(0);

    const cellInfo = table.cell("revenue", "2024E");
    expect(cellInfo.isFormula).toBe(true);
    expect(cellInfo.formulaExpr).toBe('Performance["revenue", col.prev] * 1.15');
    expect(cellInfo.excelFormula).toBe("=C7*1.15");
    expect(cellInfo.physicalA1).toBe("D7");
    expect(cellInfo.formattedValue).toBe("$115.0");

    expect(table.value("div_zero", "2024E")).toBe("#DIV/0!");
    expect(table.value("sparse_row", "2024E")).toBeNull();

    // Scalar num and formatted
    expect(table.num("revenue", "2024E")).toBeCloseTo(115.0);
    expect(table.formatted("revenue", "2024E")).toBe("$115.0");
    expect(table.formatted("revenue", "2024E", { unit: "M" })).toBe("$115.0M");
    expect(table.formatted("sparse_row", "2024E", { unit: "M" })).toBe("");

    // Negative accounting parens unit splicing
    expect(formatWithUnit("($50.0)", "M")).toBe("($50.0M)");
    expect(formatWithUnit("$50.0", "M")).toBe("$50.0M");
    expect(formatWithUnit("$50.0")).toBe("$50.0");
    expect(formatWithUnit("", "M")).toBe("");

    const negModel = new Model();
    const negSheet = negModel.addSheet("S");
    const negTable = negSheet.addTable("Neg", "Neg", { currency: "USD" });
    negTable.addColumn("v", { header: "V", format: "currency", decimals: 1 });
    negTable.addRow("loss", { cells: { v: -50.0 } });
    negTable.addRow("flag", { cells: { v: true } });
    const negAuditor = negModel.compile().table("Neg");
    expect(negAuditor.formatted("loss", "v")).toBe("($50.0)");
    expect(negAuditor.formatted("loss", "v", { unit: "M" })).toBe("($50.0M)");
    expect(() => negAuditor.num("flag", "v")).toThrow(TypeError);

    // 1D rowValues and rowNums
    const revVals = table.rowValues("revenue");
    expect(revVals).toHaveLength(3);
    expect(revVals[0]).toBeCloseTo(100.0);
    expect(revVals[1]).toBeCloseTo(115.0);
    expect(revVals[2]).toBeCloseTo(132.25);

    expect(table.rowValues("revenue", ["line_item", "2023A"])).toEqual(["Total Revenue", 100.0]);
    expect(table.rowValues("sparse_row")).toEqual([10.0, null, null]);

    const revNums = table.rowNums("revenue");
    expect(revNums).toHaveLength(3);
    expect(revNums[0]).toBeCloseTo(100.0);
    expect(revNums[1]).toBeCloseTo(115.0);
    expect(revNums[2]).toBeCloseTo(132.25);

    const revSubNums = table.rowNums("revenue", ["2023A", "2024E"]);
    expect(revSubNums).toHaveLength(2);
    expect(revSubNums[0]).toBeCloseTo(100.0);
    expect(revSubNums[1]).toBeCloseTo(115.0);

    // Unknown coordinates, rows, or columns throw Error
    expect(() => table.cell("unknown_row", "2024E")).toThrow(
      "Coordinate ('unknown_row', '2024E') not found",
    );
    expect(() => table.value("unknown_row", "2024E")).toThrow(
      "Coordinate ('unknown_row', '2024E') not found",
    );
    expect(() => table.num("unknown", "2024E")).toThrow(
      "Coordinate ('unknown', '2024E') not found",
    );
    expect(() => table.formatted("unknown", "2024E")).toThrow(
      "Coordinate ('unknown', '2024E') not found",
    );
    expect(() => table.rowValues("unknown_row")).toThrow("Row 'unknown_row' not found");
    expect(() => table.rowValues("revenue", ["unknown_col"])).toThrow(
      "Coordinate ('revenue', 'unknown_col') not found",
    );
    expect(() => table.rowNums("unknown_row")).toThrow("Row 'unknown_row' not found");
    expect(() => table.rowNums("revenue", ["unknown_col"])).toThrow(
      "Coordinate ('revenue', 'unknown_col') not found",
    );

    // Non-numeric cells throw TypeError in num() and rowNums()
    expect(() => table.num("revenue", "line_item")).toThrow(TypeError);
    expect(() => table.num("sparse_row", "2024E")).toThrow(TypeError);
    expect(() => table.num("div_zero", "2024E")).toThrow(TypeError);
    expect(() => table.rowNums("sparse_row")).toThrow(TypeError);
    expect(() => table.rowNums("div_zero")).toThrow(TypeError);

    // unboxCellValue variants and raw primitives
    expect(unboxCellValue({ Number: 100.0 })).toBe(100.0);
    expect(unboxCellValue({ Number: 42 })).toBe(42.0);
    expect(unboxCellValue({ Text: "Hello" })).toBe("Hello");
    expect(unboxCellValue({ Boolean: true })).toBe(true);
    expect(unboxCellValue({ Boolean: false })).toBe(false);
    expect(unboxCellValue("Empty")).toBeNull();
    expect(unboxCellValue("empty")).toBeNull();
    expect(unboxCellValue({ Empty: null })).toBeNull();
    expect(unboxCellValue(null)).toBeNull();
    expect(unboxCellValue(undefined)).toBeNull();
    expect(unboxCellValue(123.45)).toBe(123.45);
    expect(unboxCellValue("raw text")).toBe("raw text");
    expect(unboxCellValue(true)).toBe(true);

    expect(unboxCellValue({ Error: "Div0" })).toBe("#DIV/0!");
    expect(unboxCellValue({ Error: "Value" })).toBe("#VALUE!");
    expect(unboxCellValue({ Error: "Ref" })).toBe("#REF!");
    expect(unboxCellValue({ Error: "Num" })).toBe("#NUM!");
    expect(unboxCellValue({ Error: "Na" })).toBe("#N/A");
    expect(unboxCellValue({ Error: "Name" })).toBe("#NAME?");
  });

  it("Scenario 3: Tabular dictionary exports (toDict)", () => {
    const compiled = buildTestModel();
    const table = compiled.table("Performance");

    // Default and explicit records orient
    const defaultRecords = table.toDict();
    const records = table.toDict("records");
    expect(defaultRecords).toEqual(records);
    expect(Array.isArray(records)).toBe(true);
    expect(records).toHaveLength(5);

    for (const rowDict of records) {
      expect(Object.keys(rowDict)).toEqual(table.columns);
    }

    const revRow = records[0];
    expect(revRow?.line_item).toBe("Total Revenue");
    expect(revRow?.["2023A"]).toBe(100.0);
    expect(revRow?.["2024E"]).toBeCloseTo(115.0);

    const sparseRow = records[4];
    expect(sparseRow?.line_item).toBe("Sparse Line");
    expect(sparseRow?.["2023A"]).toBe(10.0);
    expect(sparseRow?.["2024E"]).toBeNull();
    expect(sparseRow?.["2025E"]).toBeNull();

    // dict orient
    const colDict = table.toDict("dict");
    expect(Object.keys(colDict)).toEqual(["line_item", "2023A", "2024E", "2025E"]);
    for (const colName of table.columns) {
      expect(colDict[colName]).toHaveLength(table.rows.length);
    }
    expect(colDict.line_item?.[0]).toBe("Total Revenue");
    expect(colDict["2023A"]?.[0]).toBe(100.0);
    expect(colDict["2024E"]?.[0]).toBeCloseTo(115.0);

    // index orient
    const indexDict = table.toDict("index");
    expect(Object.keys(indexDict)).toEqual([
      "revenue",
      "cogs",
      "gross_profit",
      "div_zero",
      "sparse_row",
    ]);
    expect(indexDict.revenue?.line_item).toBe("Total Revenue");
    expect(indexDict.revenue?.["2024E"]).toBeCloseTo(115.0);
    expect(indexDict.sparse_row?.["2025E"]).toBeNull();

    // split orient
    const splitRes: SplitDict = table.toDict("split");
    expect(splitRes.columns).toEqual(["line_item", "2023A", "2024E", "2025E"]);
    expect(splitRes.index).toEqual(["revenue", "cogs", "gross_profit", "div_zero", "sparse_row"]);
    expect(splitRes.data).toHaveLength(5);
    expect(splitRes.data[0]).toHaveLength(4);
    expect(splitRes.data[0]?.[0]).toBe("Total Revenue");
    expect(splitRes.data[0]?.[1]).toBe(100.0);
    expect(splitRes.data[0]?.[2]).toBeCloseTo(115.0);
  });

  it("Scenario 4: GitHub-Flavored Markdown rendering (toMarkdown)", () => {
    const compiled = buildTestModel();
    const table = compiled.table("Performance");

    const mdValues = table.toMarkdown("values");
    expect(typeof mdValues).toBe("string");
    expect(mdValues.endsWith("\n")).toBe(true);
    expect(table.toMarkdown()).toBe(mdValues);
    expect(table.toMarkdown({ view: "values" })).toBe(mdValues);

    const lines = mdValues.split("\n").filter((l) => l.length > 0);
    expect(lines[0]).toContain("Line Item");
    expect(lines[0]).toContain("2023A");
    expect(lines[0]).toContain("2024E");
    expect(lines[1]).toContain(":---");
    expect(lines[1]).toContain("---:");

    expect(lines.some((line) => line.includes("**Revenue & Operations**"))).toBe(true);
    expect(lines.some((line) => line.includes("**Other Metrics**"))).toBe(true);
    expect(lines.some((line) => line.includes("  Cost of Goods Sold"))).toBe(true);
    expect(lines.some((line) => line.includes("$100.0"))).toBe(true);
    expect(lines.some((line) => line.includes("$115.0"))).toBe(true);
    expect(lines.some((line) => line.includes("Sparse Line") && line.includes("—"))).toBe(true);

    // Formulas mode
    const mdFormulas = table.toMarkdown("formulas");
    expect(table.toMarkdown({ view: "formulas" })).toBe(mdFormulas);
    expect(mdFormulas.split("\n").some((line) => line.includes("=C") || line.includes("=B"))).toBe(
      true,
    );

    // Expr mode
    const mdExpr = table.toMarkdown("expr");
    expect(table.toMarkdown({ view: "expr" })).toBe(mdExpr);
    expect(
      mdExpr.split("\n").some((line) => line.includes("Performance") && line.includes("col.prev")),
    ).toBe(true);

    // Addresses mode
    const mdAddrs = table.toMarkdown("addresses");
    expect(table.toMarkdown({ view: "addresses" })).toBe(mdAddrs);
    expect(mdAddrs.split("\n").some((line) => line.includes("C7") || line.includes("D7"))).toBe(
      true,
    );

    // Multi-tier headers, subtotal/total rows, subsetting, and showIds
    const model = new Model({ name: "ThreeStatementModel", theme: "corporate_navy" });
    const sheet = model.addSheet("Model");
    const inc = sheet.addTable("IncomeStatement", "Income Statement", { currency: "USD" });
    inc.addColumn("line_item", { role: "descriptor", header: "Line Item" });
    inc.addColumn("y2023a", {
      role: "measure",
      group: "Historical Actuals",
      header: "2023A",
      format: "currency",
      decimals: 1,
    });
    inc.addColumnGutter();
    inc.addColumn("y2024e", {
      role: "measure",
      group: "Forecast Projections",
      header: "2024E",
      format: "currency",
      decimals: 1,
    });
    inc.addColumn("y2025e", {
      role: "measure",
      group: "Forecast Projections",
      header: "2025E",
      format: "currency",
      decimals: 1,
    });

    inc.addSectionHeader("Operating Expenses");
    inc.addRow("revenue", {
      cells: { line_item: "Revenue", y2023a: 100.0 },
      expr: 'IncomeStatement["revenue", col.prev] * 1.10',
      format: "currency",
      decimals: 1,
    });
    inc.addRowGutter();
    inc.addRow("sga", {
      cells: {
        line_item: new Cell({ value: "Salaries & SG&A Overhead", indent: 1 }),
        y2023a: 30.0,
      },
      expr: 'IncomeStatement["sga", col.prev] * 1.05',
      format: "currency",
      decimals: 1,
    });
    inc.addRow("net_income", {
      cells: { line_item: "Net Income" },
      expr: 'IncomeStatement["revenue", @col] - IncomeStatement["sga", @col]',
      style: "total",
      format: "currency",
      decimals: 1,
    });

    const isTable = model.compile().table("IncomeStatement");
    expect(isTable.sheet).toBe("Model");
    expect(isTable.shape).toEqual([3, 4]);
    expect(isTable.columns).toEqual(["line_item", "y2023a", "y2024e", "y2025e"]);
    expect(isTable.measureColumns).toEqual(["y2023a", "y2024e", "y2025e"]);
    expect(isTable.rows).toEqual(["revenue", "sga", "net_income"]);

    const md = isTable.toMarkdown("values");
    const isLines = md.split("\n").filter((l) => l.length > 0);
    expect(isLines[0]).toContain("Historical Actuals: 2023A");
    expect(isLines[0]).toContain("Forecast Projections: 2024E");
    expect(isLines[1]).toContain(":---");
    expect(isLines[1]).toContain("---:");
    expect(md).not.toContain("[y2023a]");
    expect(md).not.toContain("[net_income]");
    expect(md).toContain("**Operating Expenses**");
    expect(md).toContain("  Salaries & SG&A Overhead");
    expect(md).toContain("**Net Income**");
    expect(md).toContain("**$70.0**");

    // Subsetting rows and cols (retaining descriptor column and omitting section headers by default)
    const mdSub = isTable.toMarkdown({
      rows: ["revenue", "net_income"],
      cols: ["y2023a", "y2025e"],
    });
    const subLines = mdSub.split("\n").filter((l) => l.length > 0);
    expect(subLines).toHaveLength(4);
    expect(subLines[0]).toContain("Line Item");
    expect(subLines[0]).toContain("Historical Actuals: 2023A");
    expect(subLines[0]).toContain("Forecast Projections: 2025E");
    expect(mdSub).not.toContain("2024E");
    expect(mdSub).not.toContain("**Operating Expenses**");
    expect(mdSub).not.toContain("Salaries");

    // Subsetting rows with explicit includeSectionHeaders: true (both overload forms)
    const mdSubSec = isTable.toMarkdown({
      rows: ["revenue", "net_income"],
      cols: ["y2023a", "y2025e"],
      includeSectionHeaders: true,
    });
    expect(mdSubSec).toContain("**Operating Expenses**");
    expect(
      isTable.toMarkdown("values", {
        rows: ["revenue", "net_income"],
        cols: ["y2023a", "y2025e"],
        includeSectionHeaders: true,
      }),
    ).toBe(mdSubSec);

    // Multi-section subsetting with includeSectionHeaders: true omits orphaned section headers
    const multiSecModel = new Model({ name: "MultiSec", theme: "corporate_navy" });
    const multiSecSheet = multiSecModel.addSheet("Sheet1");
    const multiSecTbl = multiSecSheet.addTable("MultiSecTable", "Multi Section");
    multiSecTbl.addColumn("line_item", { role: "descriptor", header: "Metric" });
    multiSecTbl.addColumn("val", { role: "measure", header: "Value" });
    multiSecTbl.addSectionHeader("Section 1: Empty Match");
    multiSecTbl.addRow("r1", { cells: { line_item: "Row 1", val: 10.0 } });
    multiSecTbl.addSectionHeader("Section 2: Active Match");
    multiSecTbl.addRow("r2", { cells: { line_item: "Row 2", val: 20.0 } });
    multiSecTbl.addSectionHeader("Section 3: Trailing Empty Match");
    multiSecTbl.addRow("r3", { cells: { line_item: "Row 3", val: 30.0 } });
    const mdFilteredSec = multiSecModel
      .compile()
      .table("MultiSecTable")
      .toMarkdown({
        rows: ["r2"],
        includeSectionHeaders: true,
      });
    expect(mdFilteredSec).not.toContain("**Section 1: Empty Match**");
    expect(mdFilteredSec).toContain("**Section 2: Active Match**");
    expect(mdFilteredSec).toContain("Row 2");
    expect(mdFilteredSec).not.toContain("**Section 3: Trailing Empty Match**");

    // Unknown row or unknown col in toMarkdown throws Error
    expect(() => isTable.toMarkdown({ rows: ["unknown_row"] })).toThrow(
      "Row 'unknown_row' not found",
    );
    expect(() => isTable.toMarkdown({ cols: ["unknown_col"] })).toThrow(
      "Column 'unknown_col' not found",
    );

    // showIds: true appends [col_id] and [row_id] when display text differs from ID
    const mdIds = isTable.toMarkdown({ showIds: true });
    expect(mdIds).toContain("Historical Actuals: 2023A [y2023a]");
    expect(mdIds).toContain("Net Income [net_income]");

    // Group + multi-line stacked headers and ungrouped multi-line headers
    const lboModel = new Model({ name: "LboHeaders", theme: "corporate_navy" });
    const lboSheet = lboModel.addSheet("LBO");
    const lboTbl = lboSheet.addTable("Schedule", "LBO Schedule");
    lboTbl.addColumn("label", { role: "descriptor", header: "($ in millions)" });
    lboTbl.addColumn("mrq", { group: "Historical", headers: ["MRQ", "2012-03-31"] });
    lboTbl.addColumn("standalone", { headers: ["Standalone", "at Close", "2012-12-31"] });
    lboTbl.addColumn("group_only", { group: "Summary" });
    lboTbl.addRow("rev", {
      cells: { label: "Revenue", mrq: 100.0, standalone: 110.0, group_only: 120.0 },
    });
    const lboMd = lboModel.compile().table("Schedule").toMarkdown();
    const lboHeaderLine = lboMd.split("\n")[0];
    expect(lboHeaderLine).toContain("Historical: MRQ 2012-03-31");
    expect(lboHeaderLine).toContain("Standalone at Close 2012-12-31");
    expect(lboHeaderLine).toContain("Summary");
  });
});
