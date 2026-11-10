import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { describe, expect, it } from "vitest";
import {
  type AxisSelector,
  type AxisSlice,
  Cell,
  type CellInputValue,
  CompiledModel,
  Model,
  type Shade,
  Sheet,
  SpecificationError,
  Table,
} from "../src/index.js";

describe("TypeScript Fluent Builder DSL", () => {
  it("Scenario 1: end-to-end model construction, compilation, and .xlsx save", () => {
    const model = new Model({ name: "ThreeStatement", theme: "corporate_navy" });
    const sheet = model.addSheet("Income Statement", { showGridlines: true });
    const table = sheet.addTable("is_table", "Income Statement", {
      subtitle: "Audited Historicals & Projections",
      currency: "USD",
    });
    table
      .addColumn("line_item", { role: "descriptor", header: "Line Item" })
      .addColumn("2023A", { role: "measure", header: "2023A", format: "currency", decimals: 0 })
      .addColumn("2024E", { role: "measure", header: "2024E", format: "currency", decimals: 0 });

    table.addSectionHeader("Revenue & Gross Profit", { note: "Audited" });
    table.addRow("rev", {
      cells: { line_item: "Total Revenue", "2023A": 1000.0 },
      expr: 'is_table["rev", col.prev] * 1.10',
      format: "currency",
    });
    table.addRow("cogs", {
      cells: {
        line_item: new Cell({ value: "Cost of Goods Sold", indent: 1 }),
        "2023A": 400.0,
      },
      expr: 'is_table["cogs", col.prev] * 1.05',
      format: "currency",
    });
    table.addRow("gp", {
      cells: {
        line_item: new Cell({ value: "Gross Profit", style: "subtotal" }),
      },
      expr: 'is_table["rev", @col] - is_table["cogs", @col]',
      style: "subtotal",
      format: "currency",
    });

    const compiled = model.compile();
    expect(compiled).toBeInstanceOf(CompiledModel);
    const summary = compiled.summary();
    expect(summary.modelName).toBe("ThreeStatement");
    expect(summary.totalTables).toBe(1);
    expect(summary.totalCells).toBe(9);

    const rev2024 = compiled.inspectCell("is_table", "rev", "2024E");
    expect(rev2024.evaluatedValue).toEqual({ Number: 1100.0 });
    const cogs2024 = compiled.inspectCell("is_table", "cogs", "2024E");
    expect(cogs2024.evaluatedValue).toEqual({ Number: 420.0 });
    const gp2023 = compiled.inspectCell("is_table", "gp", "2023A");
    expect(gp2023.evaluatedValue).toEqual({ Number: 600.0 });
    const gp2024 = compiled.inspectCell("is_table", "gp", "2024E");
    expect(gp2024.evaluatedValue).toEqual({ Number: 680.0 });

    const tempDir = mkdtempSync(join(tmpdir(), "f8-builder-test-"));
    const target = join(tempDir, "from_model", "out.xlsx");
    try {
      compiled.save(target);
      expect(existsSync(target)).toBe(true);
      const bytes = readFileSync(target);
      expect(bytes[0]).toBe(0x50);
      expect(bytes[1]).toBe(0x4b);
      expect(bytes[2]).toBe(0x03);
      expect(bytes[3]).toBe(0x04);
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("explicit 0D cell indentation via Cell and DetailedCellInput", () => {
    const model = new Model();
    const sheet = model.addSheet("Sheet1");
    const table = sheet.addTable("T1", "Table 1");
    table.addColumn("line_item", { role: "descriptor", header: "Line Item" });
    table.addColumn("v", { role: "measure", header: "Value" });

    table.addRow("r1", {
      cells: {
        line_item: new Cell({ value: "Cost of Goods Sold", indent: 1 }),
        v: 100.0,
      },
    });
    table.addRow("r2", {
      cells: {
        line_item: { value: "Operating Expenses", indent: 2 },
        v: 50.0,
      },
    });

    const { tables } = model.toSpec();
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
    });

    const compiled = model.compile();
    expect(compiled.summary().totalCells).toBe(4);
  });

  it("symmetrical column and row spacing gutters", () => {
    const model = new Model();
    const sheet = model.addSheet("Sheet1");
    const table = sheet.addTable("T1", "Table 1");

    table.addColumn("c1", { header: "Col 1" });
    table.addColumnGutter();
    table.addColumn("c2", { header: "Col 2" });

    table.addRow("r1", { cells: { c1: 10.0, c2: 20.0 } });
    table.addRowGutter();
    table.addRow("r2", { cells: { c1: 30.0, c2: 40.0 } });

    const { tables } = model.toSpec();
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
    });
  });

  it("point cell override via setCell", () => {
    const model = new Model();
    const sheet = model.addSheet("Sheet1");
    const table = sheet.addTable("T1", "Table 1");
    table.addColumn("c1", { header: "Col 1" });
    table.addColumn("c2", { header: "Col 2" });
    table.addColumn("c3", { header: "Col 3" });

    table.addRow("rev", { cells: { c1: 100.0 }, expr: "PREV * 1.05" });
    table.addRow("existing", {
      cells: { c1: new Cell({ value: 50.0, format: "currency" }) },
    });

    // Point override on row with vector formula
    table.setCell("rev", "c1", { value: 150.0, note: "Audited", indent: 0 });
    // Updating an existing cell preserving previous value and format
    table.setCell("existing", "c1", { note: "Updated note" });
    // Passing pre-constructed Cell
    table.setCell("rev", "c2", new Cell({ expr: "PREV * 1.10" }));
    // Positional null value override
    table.setCell("rev", "c3", null);

    const { tables } = model.toSpec();
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
    });

    // Non-existent row or column throws Error
    expect(() => table.setCell("missing", "c1", 10.0)).toThrow("Row 'missing' does not exist");
    expect(() => table.setCell("rev", "missing", 10.0)).toThrow("Column 'missing' does not exist");

    // Creating a new cell without value or expr throws Error
    expect(() => table.setCell("existing", "c2", { note: "Invalid" })).toThrow(
      "either 'value' or 'expr'",
    );

    // Specifying both value and expr in DetailedCellInput throws Error
    expect(() => table.setCell("existing", "c2", { value: 10.0, expr: "10.0" })).toThrow(
      "cannot specify both 'value' and 'expr'",
    );
  });

  it("header specification validation", () => {
    const table = new Table("T1", "Table 1");
    expect(() =>
      table.addColumn("rev", { header: "Revenue", headers: ["Financials", "Revenue"] }),
    ).toThrow("both 'header' and 'headers'");

    for (const invalidGroup of ["", "   "]) {
      expect(() => table.addColumn("rev", { group: invalidGroup, header: "Revenue" })).toThrow(
        "Column 'group' cannot be empty",
      );
    }

    table.addColumn("mrq", { group: "Historical", headers: ["MRQ", "2012-03-31"] });
    table.addColumn("standalone", { headers: ["Standalone", "at Close", "2012-12-31"] });
    const spec = table.toSpec();
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
    ]);
  });

  it("duplicate identifier validation", () => {
    const model = new Model();
    const sheet = model.addSheet("Sheet1");
    expect(() => model.addSheet("Sheet1")).toThrow("Sheet 'Sheet1' already exists");

    const table = sheet.addTable("T1", "Table 1");
    expect(() => sheet.addTable("T1", "Duplicate Table")).toThrow("Table 'T1' already exists");

    const sheet2 = model.addSheet("Sheet2");
    expect(() => sheet2.addTable("T1", "Cross-sheet duplicate")).toThrow(
      "Table 'T1' already exists in model",
    );

    const standaloneSheet = new Sheet("Standalone");
    standaloneSheet.addTable("ST1", "Standalone 1");
    expect(() => standaloneSheet.addTable("ST1", "Duplicate Standalone")).toThrow(
      "Table 'ST1' already exists in sheet 'Standalone'",
    );

    table.addColumn("c1", { header: "Col 1" });
    expect(() => table.addColumn("c1", { header: "Col 1 duplicate" })).toThrow(
      "Column 'c1' already exists",
    );

    table.addRow("r1", { cells: { c1: 1.0 } });
    expect(() => table.addRow("r1", { cells: { c1: 2.0 } })).toThrow("Row 'r1' already exists");
  });

  it("global and sheet-scoped lookups", () => {
    const model = new Model();
    const s1 = model.addSheet("Financials");
    const s2 = model.addSheet("Operations");
    const t1 = s1.addTable("is_table", "Income Statement");
    const t2 = s2.addTable("headcount", "Headcount");

    expect(model.getSheet("Financials")).toBe(s1);
    expect(model.getSheet("Operations")).toBe(s2);
    expect(model.getTable("is_table")).toBe(t1);
    expect(model.getTable("headcount")).toBe(t2);
    expect(s1.getTable("is_table")).toBe(t1);
    expect(s2.getTable("headcount")).toBe(t2);
    expect(model.sheets).toEqual([s1, s2]);
    expect(model.tables).toEqual([t1, t2]);
    expect(s1.tables).toEqual([t1]);

    expect(() => model.getSheet("unknown")).toThrow("Sheet 'unknown' not found");
    expect(() => model.getTable("unknown")).toThrow("Table 'unknown' not found");
    expect(() => s1.getTable("headcount")).toThrow(
      "Table 'headcount' not found in sheet 'Financials'",
    );
  });

  it("cell payload validation and explicit nulls", () => {
    const c1 = new Cell({ value: 100.0 });
    expect(c1.value).toBe(100.0);
    expect(c1.expr).toBeNull();

    const c2 = new Cell({ expr: "PREV * 1.05" });
    expect(c2.expr).toBe("PREV * 1.05");
    expect(c2.value).toBeUndefined();

    const c3 = new Cell({ value: null });
    expect(c3.value).toBeNull();
    expect(c3.expr).toBeNull();

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
    });
    expect(c4.value).toBe(42.0);
    expect(c4.style).toBe("total");
    expect(c4.highlight).toBe(true);
    expect(c4.shade).toBe("muted");
    expect(c4.format).toBe("currency");
    expect(c4.decimals).toBe(2);
    expect(c4.currency).toBe("USD");
    expect(c4.indent).toBe(1);
    expect(c4.note).toBe("Source: audit");

    expect(() => new Cell({ value: 100.0, expr: "A + B" })).toThrow("both");
    expect(() => new Cell({})).toThrow("either");
  });

  it("worksheet freezePanes split coordinates and validation", () => {
    const model = new Model();
    const sheet = model.addSheet("Sheet1", {
      freezePanes: { row: 4, col: 2 },
      tabColor: "#1B365D",
    });
    const table = sheet.addTable("T1", "Table 1");
    table.addColumn("c1", { header: "Col 1" });
    table.addRow("r1", { cells: { c1: 10.0 } });

    const { manifest } = model.toSpec();
    expect(manifest).toEqual({
      modelName: "Model",
      theme: "corporate_navy",
      sheets: [
        {
          name: "Sheet1",
          showGridlines: true,
          tabColor: "#1B365D",
          freezePanes: { row: 4, col: 2 },
          tables: ["T1"],
        },
      ],
    });

    expect(() => model.addSheet("InvalidSheet", { freezePanes: { row: -1, col: 2 } })).toThrow(
      "freezePanes",
    );
    expect(() => new Sheet("DirectInvalid", { freezePanes: { row: 2, col: -1 } })).toThrow(
      "freezePanes",
    );
  });

  it("normalizes currency codes to lowercase and unwraps pure Cell values in toSpec()", () => {
    const model = new Model();
    const sheet = model.addSheet("Sheet1");
    const table = sheet.addTable("T1", "Table 1", { currency: "USD" });
    table.addColumn("c1", { header: "Col 1", currency: "EUR" });
    table.addColumn("c2", { header: "Col 2" });
    table.addColumn("c3", { header: "Col 3" });
    table.addRow("r1", {
      cells: {
        c1: new Cell({ value: 100.0, currency: "GBP" }),
        c2: { value: 50.0, currency: "CAD" },
        c3: new Cell({ value: 25.0 }),
      },
      currency: "JPY",
    });

    const { tables } = model.toSpec();
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
    });
  });

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
        title_row_height: 26.0,
        header_row_height: 20.0,
        section_row_height: 20.0,
        data_row_height: 16.0,
        gutter_row_height: 10.0,
        gutter_col_width: 4.0,
        title_gutter_row_height: 9.0,
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
    };

    const model = new Model({ name: "CustomThemeModel", theme: customTheme });
    const sheet = model.addSheet("Sheet1");
    const table = sheet.addTable("T1", "Theme Table");
    table.addColumn("c1", { header: "C1", format: "currency" });
    table.addRow("r1", { cells: { c1: 100.0 } });

    const { manifest } = model.toSpec();
    expect(manifest.theme).toEqual(customTheme);

    const compiled = model.compile();
    expect(compiled.summary().themeName).toBe("firm_custom");
    expect(compiled.formatted("T1", "r1", "c1")).toBe("$100.00");

    // Theme from file URL
    const tempDir = mkdtempSync(join(tmpdir(), "f8-theme-url-"));
    try {
      const themePath = join(tempDir, "theme.json");
      writeFileSync(themePath, JSON.stringify(customTheme), "utf-8");

      const urlModel = new Model({
        name: "UrlThemeModel",
        theme: pathToFileURL(themePath),
      });
      expect(urlModel.theme).toEqual(customTheme);

      const invalidThemePath = join(tempDir, "invalid_theme.json");
      writeFileSync(invalidThemePath, "[1, 2, 3]", "utf-8");
      expect(
        () => new Model({ name: "InvalidTheme", theme: pathToFileURL(invalidThemePath) }),
      ).toThrow("must contain a JSON object at root");
    } finally {
      rmSync(tempDir, { recursive: true, force: true });
    }
  });

  it("semantic background shade DSL and conflict detection", () => {
    const model = new Model();
    const sheet = model.addSheet("Sheet1");
    const table = sheet.addTable("T1", "Shaded Table");
    table.addColumn("label", { role: "descriptor", header: "Item" });
    table.addColumn("2023a", { header: "2023A", shade: "muted" });
    table.addColumn("stub", { header: "Stub", shade: "alt" });
    table.addColumn("2024e", { header: "2024E" });

    table.addRow("rev", {
      cells: { label: "Revenue", "2023a": 100.0, stub: 25.0, "2024e": 120.0 },
    });
    table.addRow("kpi", {
      shade: "alt",
      cells: {
        label: "KPI Band",
        "2023a": new Cell({ value: 10.0, shade: "none" }),
        stub: { value: 12.0, shade: "muted" },
        "2024e": 15.0,
      },
    });
    table.setCell("rev", "2024e", { shade: "muted" });

    const { tables } = model.toSpec();
    expect(tables[0]).toEqual({
      id: "T1",
      title: "Shaded Table",
      collapsible: false,
      columns: [
        { kind: "data", id: "label", role: "descriptor", headers: ["Item"] },
        { kind: "data", id: "2023a", role: "measure", headers: ["2023A"], shade: "muted" },
        { kind: "data", id: "stub", role: "measure", headers: ["Stub"], shade: "alt" },
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
    });

    const compiled = model.compile();
    const tableInsp = compiled.inspectTable("T1");
    expect(tableInsp.columns[0]?.shade).toBeNull();
    expect(tableInsp.columns[1]?.shade).toBe("muted");
    expect(tableInsp.columns[2]?.shade).toBe("alt");
    expect(tableInsp.rows[0]?.shade).toBeNull();
    expect(tableInsp.rows[1]?.shade).toBe("alt");

    expect(compiled.inspectCell("T1", "rev", "2023a").shade).toBe("muted");
    expect(compiled.inspectCell("T1", "rev", "stub").shade).toBe("alt");
    expect(compiled.inspectCell("T1", "rev", "2024e").shade).toBe("muted");
    expect(compiled.inspectCell("T1", "kpi", "label").shade).toBe("alt");
    expect(compiled.inspectCell("T1", "kpi", "2023a").shade).toBeNull();
    expect(compiled.inspectCell("T1", "kpi", "stub").shade).toBe("muted");
    expect(compiled.inspectCell("T1", "kpi", "2024e").shade).toBe("alt");

    // Symmetrical vector shade conflict raises SpecificationError
    const conflictModel = new Model();
    const cs = conflictModel.addSheet("S");
    const ct = cs.addTable("Conflict", "Conflict");
    const shadeVal: Shade = "alt";
    ct.addColumn("c1", { header: "C1", shade: shadeVal });
    ct.addRow("r1", { shade: "muted", cells: { c1: 1.0 } });
    try {
      conflictModel.compile();
      expect.unreachable("Should have thrown SpecificationError on shade conflict");
    } catch (err) {
      expect(err).toBeInstanceOf(SpecificationError);
      expect((err as SpecificationError).table).toBe("Conflict");
    }
  });

  it("Scenario 2: table placement DSL and dashboard layout", () => {
    const model = new Model({ name: "DashboardLayoutModel" });
    const sheet = model.addSheet("Dashboard");

    // Authoring validation: mutually exclusive rightOf and below, or negative gap
    expect(() => sheet.addTable("Bad1", "Bad", { rightOf: "T1", below: "T1" })).toThrow(
      "Cannot specify both 'rightOf' and 'below'",
    );
    expect(() => new Table("Bad2", "Bad", { rightOf: "T1", below: "T1" })).toThrow(
      "Cannot specify both 'rightOf' and 'below'",
    );
    expect(() => sheet.addTable("Bad3", "Bad", { gap: -1 })).toThrow(
      "Table 'gap' must be non-negative",
    );
    expect(() => new Table("Bad4", "Bad", { rightOf: "T1", gap: -2 })).toThrow(
      "Table 'gap' must be non-negative",
    );
    expect(() => new Table("Bad5", "Bad", { gap: 1.5 })).toThrow(
      "Table 'gap' must be non-negative integer",
    );

    // Dashboard authoring with Table object and string targets + custom gaps
    const t1 = sheet.addTable("T1", "Tall Left Table");
    t1.addColumn("item", { role: "descriptor", header: "Item" });
    t1.addColumn("c1", { header: "2023A" });
    t1.addColumn("c2", { header: "2024E" });
    for (let i = 1; i <= 9; i++) {
      t1.addRow(`r${i}`, {
        cells: { item: `Row ${i}`, c1: 10.0 * i, c2: 20.0 * i },
      });
    }

    // T2 placed rightOf T1 (passing Table object)
    const t2 = sheet.addTable("T2", "Top Right", { rightOf: t1 });
    t2.addColumn("metric", { role: "descriptor", header: "Metric" });
    t2.addColumn("val", { header: "Value" });
    t2.addRow("m1", { cells: { metric: "R1 2024E" }, expr: 'T1["r1", "c2"]' });

    // T3 placed below T2 (passing string ID and custom gap)
    const t3 = sheet.addTable("T3", "Bottom Right", { below: "T2", gap: 2 });
    t3.addColumn("metric", { role: "descriptor", header: "Metric" });
    t3.addColumn("val", { header: "Value" });
    t3.addRow("m2", { cells: { metric: "Double M1" }, expr: 'T2["m1", "val"] * 2' });

    // T4 stacked at sheet bottom with custom gap=3
    const t4 = sheet.addTable("T4", "Full Width Bottom", { gap: 3 });
    t4.addColumn("item", { role: "descriptor", header: "Summary" });
    t4.addColumn("total", { header: "Total" });
    t4.addRow("sum_row", {
      cells: { item: "Combined" },
      expr: 'T2["m1", "val"] + T3["m2", "val"]',
    });

    const { tables: tablesSpec } = model.toSpec();
    expect(tablesSpec[0]?.placement).toBeUndefined();
    expect(tablesSpec[1]?.placement).toEqual({ kind: "right_of", target: "T1" });
    expect(tablesSpec[2]?.placement).toEqual({ kind: "below", target: "T2", gap: 2 });
    expect(tablesSpec[3]?.placement).toEqual({ kind: "stack", gap: 3 });

    const compiled = model.compile();
    expect(compiled.num("T2", "m1", "val")).toBeCloseTo(20.0);
    expect(compiled.num("T3", "m2", "val")).toBeCloseTo(40.0);
    expect(compiled.num("T4", "sum_row", "total")).toBeCloseTo(60.0);

    // Verify physical coordinates and lowered formulas
    const t2Cell = compiled.inspectCell("T2", "m1", "val");
    expect(t2Cell.physicalAddress).toBe("Dashboard!G5");
    expect(t2Cell.excelFormula).toBe("=$D$5");

    const t3Cell = compiled.inspectCell("T3", "m2", "val");
    expect(t3Cell.physicalAddress).toBe("Dashboard!G11");
    expect(t3Cell.excelFormula).toBe("=$G$5*2");

    const t4Cell = compiled.inspectCell("T4", "sum_row", "total");
    expect(t4Cell.physicalAddress).toBe("Dashboard!C20");
    expect(t4Cell.excelFormula).toBe("=$G$5+$G$11");

    const sheetInsp = compiled.inspectSheet("Dashboard");
    expect(sheetInsp.minCol).toBe(1);
    expect(sheetInsp.maxCol).toBe(6);
    expect(sheetInsp.minRow).toBe(1);
    expect(sheetInsp.maxRow).toBe(19);
    expect(sheetInsp.colWidths[4]).toBeCloseTo(3.0);

    const xlsxBytes = compiled.emitWorkbook();
    expect(xlsxBytes[0]).toBe(0x50);
    expect(xlsxBytes[1]).toBe(0x4b);

    // Invalid placement target raises SpecificationError with err.table
    const badTargetModel = new Model();
    const bs = badTargetModel.addSheet("S1");
    const bt = bs.addTable("OrphanTable", "Orphan", { rightOf: "MissingTarget" });
    bt.addColumn("c1", { header: "C1" });
    bt.addRow("r1", { cells: { c1: 1.0 } });
    try {
      badTargetModel.compile();
      expect.unreachable("Should have thrown SpecificationError on invalid placement target");
    } catch (err) {
      expect(err).toBeInstanceOf(SpecificationError);
      expect((err as SpecificationError).table).toBe("OrphanTable");
    }

    // Overlapping tables raise SpecificationError with err.table === secondTable.id
    const overlapModel = new Model();
    const osSheet = overlapModel.addSheet("S1");
    const ot1 = osSheet.addTable("Base", "Base");
    ot1.addColumn("c1", { header: "C1" });
    ot1.addRow("r1", { cells: { c1: 1.0 } });

    const ot2 = osSheet.addTable("FirstRight", "First", { rightOf: ot1 });
    ot2.addColumn("c1", { header: "C1" });
    ot2.addRow("r1", { cells: { c1: 2.0 } });

    const ot3 = osSheet.addTable("CollidingRight", "Second", { rightOf: ot1 });
    ot3.addColumn("c1", { header: "C1" });
    ot3.addRow("r1", { cells: { c1: 3.0 } });

    try {
      overlapModel.compile();
      expect.unreachable("Should have thrown SpecificationError on table overlap");
    } catch (err) {
      expect(err).toBeInstanceOf(SpecificationError);
      expect((err as SpecificationError).table).toBe("CollidingRight");
    }
  });

  it("Scenario 3a: setCells multi-regime timeline row authoring", () => {
    const model = new Model();
    const sheet = model.addSheet("Schedule");
    const table = sheet.addTable("T", "Multi-Regime Schedule");
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
      .addColumn("fy2", { role: "measure", header: "FY2" });

    table.addRow("amort", {
      cells: { line_item: "Amortization" },
      expr: 'T["amort", col.prev] * 2',
    });
    const nullOverride: CellInputValue = null;
    const sliceSelector: AxisSlice = { start: "q1", stop: "q2" };
    table.setCells("amort", ["standalone", "adj", "pf"], nullOverride);
    table.setCells("amort", sliceSelector, { expr: "10.0" });

    const compiled = model.compile();
    expect(compiled.value("T", "amort", "standalone")).toBeNull();
    expect(compiled.value("T", "amort", "adj")).toBeNull();
    expect(compiled.value("T", "amort", "pf")).toBeNull();
    expect(compiled.inspectCell("T", "amort", "standalone").evaluatedValue).toBe("Empty");
    expect(compiled.inspectCell("T", "amort", "adj").evaluatedValue).toBe("Empty");
    expect(compiled.inspectCell("T", "amort", "pf").evaluatedValue).toBe("Empty");
    expect(compiled.num("T", "amort", "q1")).toBeCloseTo(10.0);
    expect(compiled.num("T", "amort", "q2")).toBeCloseTo(10.0);
    expect(compiled.num("T", "amort", "fy1")).toBeCloseTo(20.0);
    expect(compiled.num("T", "amort", "fy2")).toBeCloseTo(40.0);
  });

  it("Scenario 3b: setCells 2D rectangular block and metadata patching", () => {
    const model = new Model();
    const sheet = model.addSheet("Matrix");
    const table = sheet.addTable("T1", "Block Matrix");
    table
      .addColumn("c1", { header: "C1" })
      .addColumnGutter()
      .addColumn("c2", { header: "C2" })
      .addColumn("c3", { header: "C3" });

    table
      .addRow("r1", { cells: { c3: 1.0 } })
      .addSectionHeader("Section Divider")
      .addRowGutter()
      .addRow("r2", { cells: { c3: 2.0 } })
      .addRow("r3", { cells: { c1: 3.0, c2: 3.0, c3: 3.0 } });

    const rowSelector: AxisSelector = new Set(["r1", "r2"]);
    table.setCells({ start: "r1", stop: "r2" }, { start: "c1", stop: "c2" }, 5.0);
    table.setCells(rowSelector, "c2", { shade: "alt", note: "patched" });

    const { tables } = model.toSpec();
    const rowsSpec = tables[0]?.rows.filter((r) => r.kind === "data");
    expect(rowsSpec?.[0]).toEqual({
      kind: "data",
      id: "r1",
      cells: {
        c1: 5.0,
        c2: { value: 5.0, shade: "alt", note: "patched" },
        c3: 1.0,
      },
    });
    expect(rowsSpec?.[1]).toEqual({
      kind: "data",
      id: "r2",
      cells: {
        c1: 5.0,
        c2: { value: 5.0, shade: "alt", note: "patched" },
        c3: 2.0,
      },
    });
    expect(rowsSpec?.[2]).toEqual({
      kind: "data",
      id: "r3",
      cells: { c1: 3.0, c2: 3.0, c3: 3.0 },
    });

    const compiled = model.compile();
    expect(compiled.num("T1", "r1", "c1")).toBeCloseTo(5.0);
    expect(compiled.num("T1", "r2", "c1")).toBeCloseTo(5.0);
    expect(compiled.inspectCell("T1", "r1", "c2").shade).toBe("alt");
    expect(compiled.inspectCell("T1", "r1", "c2").note).toBe("patched");
    expect(compiled.inspectCell("T1", "r2", "c2").shade).toBe("alt");
    expect(compiled.inspectCell("T1", "r2", "c2").note).toBe("patched");
  });

  it("Scenario 3c & 4f: setCells selector validation and atomic failure", () => {
    const table = new Table("T1", "Validation Table");
    table.addColumn("c1", { header: "C1" });
    table.addColumn("c2", { header: "C2" });
    table.addRow("r1", { cells: { c1: 10.0 } });
    table.addRow("r2", { cells: { c1: 20.0, c2: 30.0 } });

    // Unknown row or column ID (via string, Iterable, or AxisSlice endpoint) throws Error
    expect(() => table.setCells("missing_row", "c1", 1.0)).toThrow(
      "Row 'missing_row' does not exist",
    );
    expect(() => table.setCells(["r1", "missing_row"], "c1", 1.0)).toThrow(
      "Row 'missing_row' does not exist",
    );
    expect(() => table.setCells({ start: "missing_row", stop: "r2" }, "c1", 1.0)).toThrow(
      "Row 'missing_row' does not exist",
    );
    expect(() => table.setCells({ start: "r1", stop: "missing_row" }, "c1", 1.0)).toThrow(
      "Row 'missing_row' does not exist",
    );

    expect(() => table.setCells("r1", "missing_col", 1.0)).toThrow(
      "Column 'missing_col' does not exist",
    );
    expect(() => table.setCells("r1", ["c1", "missing_col"], 1.0)).toThrow(
      "Column 'missing_col' does not exist",
    );
    expect(() => table.setCells("r1", { start: "missing_col", stop: "c2" }, 1.0)).toThrow(
      "Column 'missing_col' does not exist",
    );
    expect(() => table.setCells("r1", { start: "c1", stop: "missing_col" }, 1.0)).toThrow(
      "Column 'missing_col' does not exist",
    );

    // Inverted slice throws Error
    expect(() => table.setCells("r1", { start: "c2", stop: "c1" }, 1.0)).toThrow(
      "Inverted column slice",
    );
    expect(() => table.setCells({ start: "r2", stop: "r1" }, "c1", 1.0)).toThrow(
      "Inverted row slice",
    );

    // Empty iterable throws Error
    expect(() => table.setCells([], "c1", 1.0)).toThrow("Row selector iterable cannot be empty");
    expect(() => table.setCells("r1", [], 1.0)).toThrow("Column selector iterable cannot be empty");

    // Atomic failure: ("r1", "c1") has 10.0 and ("r1", "c2") is unpopulated
    expect(() => table.setCells("r1", ["c1", "c2"], { note: "missing payload" })).toThrow(
      "must specify either 'value' or 'expr'",
    );

    // Verify ("r1", "c1") was left completely unmutated without the note
    const spec = table.toSpec();
    const rowsSpec = spec.rows.filter((r) => r.kind === "data");
    expect(rowsSpec[0]).toEqual({
      kind: "data",
      id: "r1",
      cells: { c1: 10.0 },
    });
  });

  it("active table selector DSL (@table)", () => {
    const model = new Model({ name: "ActiveTableDSL" });
    const sheet = model.addSheet("Model");
    const table = sheet.addTable("IncomeStatement", "Income Statement");
    table.addColumn("y1", { role: "measure", header: "Y1" });
    table.addColumn("y2", { role: "measure", header: "Y2" });

    table.addRow("rev", { cells: { y1: 100.0, y2: 200.0 } });
    table.addRow("cogs", { cells: { y1: 40.0, y2: 70.0 } });
    table.addRow("gp", { expr: '@table["rev", @col] - @table["cogs", @col]' });

    const compiled = model.compile();
    const inspY1 = compiled.inspectCell("IncomeStatement", "gp", "y1");
    expect(inspY1.formulaExpr).toBe('@table["rev", @col] - @table["cogs", @col]');
    expect(inspY1.evaluatedValue).toEqual({ Number: 60.0 });

    const inspY2 = compiled.inspectCell("IncomeStatement", "gp", "y2");
    expect(inspY2.formulaExpr).toBe('@table["rev", @col] - @table["cogs", @col]');
    expect(inspY2.evaluatedValue).toEqual({ Number: 130.0 });
  });
});
