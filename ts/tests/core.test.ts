import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  Cell,
  compile,
  CycleDetectedError,
  defaultTheme,
  EmissionError,
  F8Error,
  FormulaParseError,
  InspectionError,
  Model,
  ReferenceResolutionError,
  SpecificationError,
  VERSION,
} from "../src/index.js";
import type { ProjectManifestInput, TableSpecInput } from "../src/types.js";

describe("WebAssembly Core Bindings and TypeScript SDK", () => {
  it("exposes package version and default corporate_navy theme", () => {
    const pkg = JSON.parse(
      readFileSync(new URL("../../package.json", import.meta.url), "utf-8"),
    ) as { version: string; engines: { node: string } };
    expect(VERSION).toBe(pkg.version);
    expect(pkg.engines.node).toBe(">=22.3.0");
    const theme = defaultTheme();
    expect(theme).toBeTypeOf("object");
    expect(theme["name"]).toBe("corporate_navy");
  });

  it("supports deterministic WASM disposal via dispose() and [Symbol.dispose]()", () => {
    const m = new Model({ name: "DisposableModel" });
    const t = m.addSheet("Main").addTable("T", "Table");
    t.addColumn("c1", { role: "measure", header: "Val" });
    t.addRow("r1", { cells: { c1: 100.0 } });

    const compiled = m.compile();
    expect(compiled.num("T", "r1", "c1")).toBe(100.0);

    compiled.dispose();
    // Idempotent second call
    compiled.dispose();
    compiled[Symbol.dispose]();

    expect(() => compiled.summary()).toThrow(F8Error);
    expect(() => compiled.value("T", "r1", "c1")).toThrow(F8Error);
    expect(() => compiled.num("T", "r1", "c1")).toThrow(F8Error);
    expect(() => compiled.formatted("T", "r1", "c1")).toThrow(F8Error);
    expect(() => compiled.inspectCell("T", "r1", "c1")).toThrow(F8Error);
    expect(() => compiled.inspectTable("T")).toThrow(F8Error);
    expect(() => compiled.inspectSheet("Main")).toThrow(F8Error);
    expect(() => compiled.precedentsOf("T", "r1", "c1")).toThrow(F8Error);
    expect(() => compiled.dependentsOf("T", "r1", "c1")).toThrow(F8Error);
    expect(() => compiled.emitWorkbook()).toThrow(F8Error);
  });

  describe("Scenario 1: Nominal compilation, scalar lookup, and inspection", () => {
    const manifest: ProjectManifestInput = {
      modelName: "FinancialModel",
      theme: "corporate_navy",
      sheets: [
        {
          name: "BalanceSheet",
          tables: ["PPE"],
        },
      ],
    };

    const ppeTable: TableSpecInput = {
      id: "PPE",
      title: "Property, Plant & Equipment",
      columns: [
        {
          kind: "data",
          id: "line_item",
          role: "descriptor",
          headers: ["Line Item"],
        },
        {
          kind: "data",
          id: "2024A",
          role: "measure",
          headers: ["2024A"],
          format: "currency",
          decimals: 1,
        },
        {
          kind: "data",
          id: "2025A",
          role: "measure",
          headers: ["2025A"],
          format: "currency",
          decimals: 1,
        },
      ],
      rows: [
        {
          kind: "data",
          id: "beg_balance",
          format: "currency",
          decimals: 1,
          cells: {
            line_item: "Beginning Balance",
            "2024A": 100.0,
          },
          expr: 'PPE["end_balance", col.prev(1)]',
        },
        {
          kind: "data",
          id: "capex",
          format: "currency",
          decimals: 1,
          cells: {
            line_item: "Capital Expenditures",
            "2024A": 20.0,
            "2025A": 20.0,
          },
        },
        {
          kind: "data",
          id: "depreciation",
          format: "currency",
          decimals: 1,
          cells: {
            line_item: "Depreciation",
            "2024A": 20.0,
            "2025A": 10.0,
          },
        },
        {
          kind: "data",
          id: "end_balance",
          format: "currency",
          decimals: 1,
          expr: 'PPE["beg_balance", @col] + PPE["capex", @col] - PPE["depreciation", @col]',
          cells: {
            line_item: "Ending Balance",
          },
        },
        {
          kind: "data",
          id: "net_loss",
          format: "currency",
          decimals: 1,
          cells: {
            line_item: "Net Loss",
            "2024A": -50.0,
            "2025A": -50.0,
          },
        },
      ],
    };

    it("evaluates corkscrew table and verifies summary counts", () => {
      const model = compile(manifest, [ppeTable]);
      const summary = model.summary();

      expect(summary.modelName).toBe("FinancialModel");
      expect(summary.themeName).toBe("corporate_navy");
      expect(summary.totalSheets).toBe(1);
      expect(summary.totalTables).toBe(1);
      expect(summary.totalCells).toBe(15);
      expect(summary.totalDrivers).toBe(12);
      expect(summary.totalFormulas).toBe(3);

      expect(summary.sheets).toHaveLength(1);
      expect(summary.sheets[0]?.name).toBe("BalanceSheet");
      expect(summary.sheets[0]?.tables[0]?.id).toBe("PPE");
      expect(summary.sheets[0]?.tables[0]?.rowCount).toBe(5);
      expect(summary.sheets[0]?.tables[0]?.colCount).toBe(3);
    });

    it("verifies scalar value and num lookups", () => {
      const model = compile(manifest, [ppeTable]);

      // 2024A: 100 + 20 - 20 = 100.0
      // 2025A: beg_balance = 100.0, capex = 20.0, depreciation = 10.0 -> end_balance = 110.0
      expect(model.value('PPE["end_balance", "2025A"]')).toBe(110.0);
      expect(model.value("PPE", "end_balance", "2025A")).toBe(110.0);
      expect(model.num("PPE", "end_balance", "2025A")).toBe(110.0);
      expect(model.num('PPE["end_balance", "2025A"]')).toBe(110.0);
      expect(model.value("PPE", "beg_balance", "line_item")).toBe("Beginning Balance");
    });

    it("formats cell values with and without units and negative parenthesis handling", () => {
      const model = compile(manifest, [ppeTable]);

      expect(model.formatted('PPE["end_balance", "2025A"]')).toBe("$110.0");
      expect(model.formatted('PPE["end_balance", "2025A"]', { unit: "M" })).toBe("$110.0M");
      expect(model.formatted("PPE", "end_balance", "2025A", { unit: "M" })).toBe("$110.0M");

      // Negative accounting string should splice unit inside closing parenthesis
      expect(model.formatted('PPE["net_loss", "2025A"]')).toBe("($50.0)");
      expect(model.formatted('PPE["net_loss", "2025A"]', { unit: "M" })).toBe("($50.0M)");
    });

    it("inspects cell, table, and sheet with camelCase contracts", () => {
      const model = compile(manifest, [ppeTable]);

      const cell = model.inspectCell("PPE", "end_balance", "2025A");
      expect(cell.coordinate).toEqual({ table: "PPE", row: "end_balance", col: "2025A" });
      expect(cell.sheet).toBe("BalanceSheet");
      expect(cell.physicalAddress).toBe("'BalanceSheet'!D8");
      expect(cell.evaluatedValue).toEqual({ Number: 110.0 });
      expect(cell.formattedValue).toBe("$110.0");
      expect(cell.isFormula).toBe(true);
      expect(cell.format).toBe("currency");
      expect(cell.decimals).toBe(1);
      expect(cell.precedents.length).toBeGreaterThan(0);
      expect(cell.precedents[0]?.cellId).toBeDefined();

      const table = model.inspectTable("PPE");
      expect(table.tableId).toBe("PPE");
      expect(table.sheet).toBe("BalanceSheet");
      expect(table.columns).toHaveLength(3);
      expect(table.columns[0]?.isDescriptor).toBe(true);
      expect(table.columns[0]?.style).toBe("data");
      expect(table.rows).toHaveLength(5);
      expect(table.cells['PPE["end_balance", "2025A"]']?.value).toEqual({ Number: 110.0 });
      expect(Object.keys(table.cells).slice(0, 3)).toEqual([
        'PPE["beg_balance", "line_item"]',
        'PPE["beg_balance", "2024A"]',
        'PPE["beg_balance", "2025A"]',
      ]);

      const sheet = model.inspectSheet("BalanceSheet");
      expect(sheet.sheetId).toBe("BalanceSheet");
      expect(sheet.tables).toEqual(["PPE"]);
      expect(sheet.rows.length).toBeGreaterThan(0);
    });

    it("traces precedents and dependents trees", () => {
      const model = compile(manifest, [ppeTable]);

      const precedents = model.precedentsOf("PPE", "end_balance", "2025A", { recursive: true });
      expect(precedents.root).toEqual({ table: "PPE", row: "end_balance", col: "2025A" });
      expect(precedents.directCount).toBe(3);
      expect(precedents.transitiveCount).toBeGreaterThanOrEqual(3);
      expect(precedents.nodes.length).toBeGreaterThan(0);

      const dependents = model.dependentsOf("PPE", "beg_balance", "2024A", { recursive: true });
      expect(dependents.root).toEqual({ table: "PPE", row: "beg_balance", col: "2024A" });
      expect(dependents.directCount).toBe(1);
    });

    it("normalizes uppercase ISO currency codes across table, column, row, and cell inputs", () => {
      const currencyManifest: ProjectManifestInput = {
        modelName: "CurrencyNormalizationModel",
        theme: "corporate_navy",
        sheets: [{ name: "Currencies", tables: ["FX"] }],
      };

      const fxTable: TableSpecInput = {
        id: "FX",
        title: "FX Rates",
        currency: "USD",
        columns: [
          { kind: "data", id: "usd_col", currency: "USD" },
          { kind: "data", id: "eur_col", currency: "EUR" },
          { kind: "data", id: "gbp_col", currency: "GBP" },
        ],
        rows: [
          {
            kind: "data",
            id: "r1",
            currency: "EUR",
            cells: {
              usd_col: { value: 100, currency: "USD", format: "currency" },
              eur_col: { value: 200, currency: "EUR", format: "currency" },
              gbp_col: { value: 300, currency: "GBP", format: "currency" },
            },
          },
          {
            kind: "data",
            id: "r2",
            currency: "GBP",
            cells: {
              usd_col: { expr: 'FX["r1", "usd_col"] * 2' },
              eur_col: { expr: 'FX["r1", "eur_col"] * 2' },
              gbp_col: { expr: 'FX["r1", "gbp_col"] * 2' },
            },
          },
        ],
      };

      const model = compile(currencyManifest, [fxTable]);
      expect(model.num("FX", "r1", "usd_col")).toBe(100);
      expect(model.num("FX", "r1", "eur_col")).toBe(200);
      expect(model.num("FX", "r1", "gbp_col")).toBe(300);
      expect(model.num("FX", "r2", "usd_col")).toBe(200);
      expect(model.num("FX", "r2", "eur_col")).toBe(400);
      expect(model.num("FX", "r2", "gbp_col")).toBe(600);

      const cell = model.inspectCell("FX", "r1", "usd_col");
      expect(cell.currency).toBe("usd");
    });
  });

  describe("Scenario 2: In-memory .xlsx emission and disk save()", () => {
    const manifest: ProjectManifestInput = {
      modelName: "EmissionModel",
      theme: "corporate_navy",
      sheets: [{ name: "Sheet1", tables: ["T1"] }],
    };

    const table: TableSpecInput = {
      id: "T1",
      title: "Table 1",
      columns: [
        { kind: "data", id: "desc", role: "descriptor", headers: ["Description"] },
        { kind: "data", id: "col1", headers: ["2024"] },
      ],
      rows: [
        {
          kind: "data",
          id: "r1",
          cells: { desc: "Item", col1: 42.0 },
        },
      ],
    };

    it("emits in-memory workbook buffer starting with PK zip magic bytes", () => {
      const model = compile(manifest, [table]);
      const bytes = model.emitWorkbook();

      expect(bytes).toBeInstanceOf(Uint8Array);
      expect(bytes.length).toBeGreaterThan(100);
      // PK\x03\x04
      expect(bytes[0]).toBe(0x50);
      expect(bytes[1]).toBe(0x4b);
      expect(bytes[2]).toBe(0x03);
      expect(bytes[3]).toBe(0x04);
    });

    it("saves workbook to disk creating parent directories and verifies byte parity", () => {
      const model = compile(manifest, [table]);
      const tempDir = mkdtempSync(join(tmpdir(), "f8-test-"));
      const targetFile = join(tempDir, "nested", "deeply", "workbook.xlsx");

      try {
        expect(existsSync(targetFile)).toBe(false);
        model.save(targetFile);
        expect(existsSync(targetFile)).toBe(true);

        const savedBytes = readFileSync(targetFile);
        const inMemoryBytes = model.emitWorkbook();
        expect(savedBytes).toEqual(Buffer.from(inMemoryBytes));
      } finally {
        rmSync(tempDir, { recursive: true, force: true });
      }
    });

    it("throws EmissionError when save fails due to invalid path", () => {
      const model = compile(manifest, [table]);
      const tempDir = mkdtempSync(join(tmpdir(), "f8-test-"));

      try {
        // Trying to save directly to an existing directory path throws EmissionError
        let caught: unknown;
        try {
          model.save(tempDir);
        } catch (err) {
          caught = err;
        }
        expect(caught).toBeInstanceOf(EmissionError);
        expect(caught).toBeInstanceOf(F8Error);
        expect((caught as EmissionError).cause).toBeInstanceOf(Error);
      } finally {
        rmSync(tempDir, { recursive: true, force: true });
      }
    });
  });

  describe("Scenario 3: Structured compiler and inspection exceptions", () => {
    it("(a) throws CycleDetectedError with .cycle property on circular references", () => {
      const manifest: ProjectManifestInput = {
        modelName: "CycleModel",
        theme: "corporate_navy",
        sheets: [{ name: "S", tables: ["T"] }],
      };
      const table: TableSpecInput = {
        id: "T",
        title: "Cycle Table",
        columns: [
          { kind: "data", id: "desc", role: "descriptor", headers: ["Desc"] },
          { kind: "data", id: "val", headers: ["Val"] },
        ],
        rows: [
          {
            kind: "data",
            id: "a",
            expr: 'T["b", @col]',
            cells: { desc: "Row A" },
          },
          {
            kind: "data",
            id: "b",
            expr: 'T["a", @col]',
            cells: { desc: "Row B" },
          },
        ],
      };

      try {
        compile(manifest, [table]);
        expect.unreachable("Should have thrown CycleDetectedError");
      } catch (err) {
        expect(err).toBeInstanceOf(CycleDetectedError);
        expect(err).toBeInstanceOf(F8Error);
        const cycleErr = err as CycleDetectedError;
        expect(cycleErr.cycle.length).toBeGreaterThanOrEqual(2);
      }
    });

    it("(b) throws ReferenceResolutionError on unknown column reference", () => {
      const manifest: ProjectManifestInput = {
        modelName: "RefModel",
        theme: "corporate_navy",
        sheets: [{ name: "S", tables: ["T"] }],
      };
      const table: TableSpecInput = {
        id: "T",
        title: "Ref Table",
        columns: [
          { kind: "data", id: "desc", role: "descriptor", headers: ["Desc"] },
          { kind: "data", id: "val", headers: ["Val"] },
        ],
        rows: [
          {
            kind: "data",
            id: "a",
            expr: 'T["a", "unknown_col"]',
            cells: { desc: "Row A" },
          },
        ],
      };

      try {
        compile(manifest, [table]);
        expect.unreachable("Should have thrown ReferenceResolutionError");
      } catch (err) {
        expect(err).toBeInstanceOf(ReferenceResolutionError);
        expect(err).toBeInstanceOf(F8Error);
        const refErr = err as ReferenceResolutionError;
        expect(refErr.targetTable).toBe("T");
        expect(refErr.row).toBeNull();
        expect(refErr.col).toBe("unknown_col");
        expect(refErr.coordinate).toBe('T["a", "val"]');
      }
    });

    it("(c) throws SpecificationError on unassigned cell and duplicate table ID", () => {
      // 1. Unassigned measure cell throws SpecificationError with table and reason
      const unassignedManifest: ProjectManifestInput = {
        modelName: "UnassignedCellModel",
        theme: "corporate_navy",
        sheets: [{ name: "S", tables: ["T_Unassigned"] }],
      };
      const unassignedTable: TableSpecInput = {
        id: "T_Unassigned",
        title: "Unassigned Cell Table",
        columns: [{ kind: "data", id: "c1", headers: ["Col 1"] }],
        rows: [{ kind: "data", id: "r1" }],
      };

      try {
        compile(unassignedManifest, [unassignedTable]);
        expect.unreachable("Should have thrown SpecificationError on unassigned cell");
      } catch (err) {
        expect(err).toBeInstanceOf(SpecificationError);
        expect(err).toBeInstanceOf(F8Error);
        const specErr = err as SpecificationError;
        expect(specErr.table).toBe("T_Unassigned");
        expect(specErr.reason.toLowerCase()).toContain("unassigned");
      }

      // 2. Duplicate table ID throws SpecificationError with table and reason
      const dupManifest: ProjectManifestInput = {
        modelName: "DupTableModel",
        theme: "corporate_navy",
        sheets: [{ name: "S", tables: ["T_Dup"] }],
      };
      const dupTable1: TableSpecInput = {
        id: "T_Dup",
        title: "Table 1",
        columns: [{ kind: "data", id: "c1", headers: ["Col 1"] }],
        rows: [{ kind: "data", id: "r1", cells: { c1: 10 } }],
      };
      const dupTable2: TableSpecInput = {
        id: "T_Dup",
        title: "Table 2",
        columns: [{ kind: "data", id: "c1", headers: ["Col 1"] }],
        rows: [{ kind: "data", id: "r1", cells: { c1: 20 } }],
      };

      try {
        compile(dupManifest, [dupTable1, dupTable2]);
        expect.unreachable("Should have thrown SpecificationError on duplicate table ID");
      } catch (err) {
        expect(err).toBeInstanceOf(SpecificationError);
        expect(err).toBeInstanceOf(F8Error);
        const specErr = err as SpecificationError;
        expect(specErr.table).toBe("T_Dup");
        expect(specErr.reason.toLowerCase()).toContain("duplicate");
      }
    });

    it("(d) throws FormulaParseError on cell-level token syntax error", () => {
      const manifest: ProjectManifestInput = {
        modelName: "ParseErrorModel",
        theme: "corporate_navy",
        sheets: [{ name: "S", tables: ["T"] }],
      };
      const table: TableSpecInput = {
        id: "T",
        title: "Parse Table",
        columns: [
          { kind: "data", id: "desc", role: "descriptor", headers: ["Desc"] },
          { kind: "data", id: "val", headers: ["Val"] },
        ],
        rows: [
          {
            kind: "data",
            id: "a",
            cells: {
              desc: "Row A",
              val: { expr: "100 + * 2" },
            },
          },
        ],
      };

      try {
        compile(manifest, [table]);
        expect.unreachable("Should have thrown FormulaParseError");
      } catch (err) {
        expect(err).toBeInstanceOf(FormulaParseError);
        expect(err).toBeInstanceOf(F8Error);
        const parseErr = err as FormulaParseError;
        expect(parseErr.table).toBe("T");
        expect(parseErr.row).toBe("a");
        expect(parseErr.col).toBe("val");
        expect(typeof parseErr.tokenOffset).toBe("number");
        expect(parseErr.tokenOffset).toBe(6);
      }
    });

    it("(e) throws InspectionError on unknown coordinate or entity", () => {
      const manifest: ProjectManifestInput = {
        modelName: "InspectErrorModel",
        theme: "corporate_navy",
        sheets: [{ name: "S", tables: ["T"] }],
      };
      const table: TableSpecInput = {
        id: "T",
        title: "Table",
        columns: [
          { kind: "data", id: "desc", role: "descriptor", headers: ["Desc"] },
          { kind: "data", id: "val", headers: ["Val"] },
        ],
        rows: [
          {
            kind: "data",
            id: "r1",
            cells: { desc: "Desc", val: 10.0 },
          },
        ],
      };
      const model = compile(manifest, [table]);

      try {
        model.inspectCell("NONEXISTENT", "r", "c");
        expect.unreachable("Should have thrown InspectionError");
      } catch (err) {
        expect(err).toBeInstanceOf(InspectionError);
        expect(err).toBeInstanceOf(F8Error);
        const inspErr = err as InspectionError;
        expect(inspErr.entity).toContain("NONEXISTENT");
      }

      try {
        model.inspectTable("UNKNOWN_TABLE");
        expect.unreachable("Should have thrown InspectionError");
      } catch (err) {
        expect(err).toBeInstanceOf(InspectionError);
        expect(err).toBeInstanceOf(F8Error);
        const inspErr = err as InspectionError;
        expect(inspErr.entity).toBe("UNKNOWN_TABLE");
      }
    });

    it("(f) throws standard JavaScript TypeError when num() is queried on a non-numeric cell", () => {
      const manifest: ProjectManifestInput = {
        modelName: "TypeErrorModel",
        theme: "corporate_navy",
        sheets: [{ name: "S", tables: ["T"] }],
      };
      const table: TableSpecInput = {
        id: "T",
        title: "Table",
        columns: [
          { kind: "data", id: "desc", role: "descriptor", headers: ["Desc"] },
          { kind: "data", id: "val", headers: ["Val"] },
        ],
        rows: [
          {
            kind: "data",
            id: "r1",
            cells: { desc: "Text Only", val: 10.0 },
          },
        ],
      };
      const model = compile(manifest, [table]);

      try {
        model.num("T", "r1", "desc");
        expect.unreachable("Should have thrown TypeError");
      } catch (err) {
        expect(err).toBeInstanceOf(TypeError);
        // TypeError should NOT be an instance of F8Error
        expect(err instanceof F8Error).toBe(false);
      }
    });

    it("(g) validates range slice contiguity and populates ReferenceResolutionError properties", () => {
      // 1. Contiguous sub-block and vertical single-descriptor slice succeed
      const okModel = new Model({ name: "ContiguousOk", theme: "corporate_navy" });
      const okTable = okModel.addSheet("Main").addTable("T", "Summary");
      okTable
        .addColumn("line_item", { role: "descriptor", header: "Line Item" })
        .addColumnGutter()
        .addColumn("2024E", { role: "measure", header: "2024E" })
        .addColumn("2025E", { role: "measure", header: "2025E" });
      okTable.addSectionHeader("Revenue");
      okTable.addRow("r1", { cells: { line_item: "Product", "2024E": 10, "2025E": 20 } });
      okTable.addRow("r2", { cells: { line_item: "Rev", "2024E": 30, "2025E": 40 } });
      okTable.addRowGutter();
      okTable.addRow("tot", {
        cells: { line_item: new Cell({ expr: 'INDEX(@table["r1".."r2", @col], 2)' }) },
        expr: 'SUM(@table["r1".."r2", "2024E".."2025E"]) + XLOOKUP("Rev", @table["r1".."r2", "line_item"], @table["r1".."r2", @col])',
      });
      const compiled = okModel.compile();
      expect(compiled.num("T", "tot", "2024E")).toBe(130);
      expect(compiled.num("T", "tot", "2025E")).toBe(140);
      expect(compiled.value("T", "tot", "line_item")).toBe("Rev");

      // 2. Row range spanning RowGutter -> ReferenceResolutionError (col === null, row === null)
      const rgModel = new Model({ name: "RowGutterFail" });
      const rgTable = rgModel.addSheet("Main").addTable("T", "T");
      rgTable.addColumn("2024E", { role: "measure", header: "2024E" });
      rgTable.addRow("r1", { cells: { "2024E": 10 } });
      rgTable.addRowGutter();
      rgTable.addRow("r2", { cells: { "2024E": 20 } });
      rgTable.addRow("tot", { expr: 'SUM(T["r1".."r2", @col])' });
      try {
        rgModel.compile();
        expect.unreachable("Should have thrown ReferenceResolutionError");
      } catch (err) {
        if (!(err instanceof ReferenceResolutionError)) {
          expect.unreachable("Expected ReferenceResolutionError");
        }
        expect(err).toBeInstanceOf(F8Error);
        expect(err.coordinate).toBe('T["tot", "2024E"]');
        expect(err.targetTable).toBe("T");
        expect(err.row).toBeNull();
        expect(err.col).toBeNull();
        expect(err.message).toContain("a gutter row");
      }

      // 3. Row range spanning SectionHeader -> ReferenceResolutionError (col === null, row === null)
      const shModel = new Model({ name: "SectionHeaderFail" });
      const shTable = shModel.addSheet("Main").addTable("T", "T");
      shTable.addColumn("2024E", { role: "measure", header: "2024E" });
      shTable.addRow("r1", { cells: { "2024E": 10 } });
      shTable.addSectionHeader("OpEx");
      shTable.addRow("r2", { cells: { "2024E": 20 } });
      shTable.addRow("tot", { expr: 'SUM(T["r1".."r2", @col])' });
      try {
        shModel.compile();
        expect.unreachable("Should have thrown ReferenceResolutionError");
      } catch (err) {
        if (!(err instanceof ReferenceResolutionError)) {
          expect.unreachable("Expected ReferenceResolutionError");
        }
        expect(err.coordinate).toBe('T["tot", "2024E"]');
        expect(err.targetTable).toBe("T");
        expect(err.row).toBeNull();
        expect(err.col).toBeNull();
        expect(err.message).toContain("section header 'OpEx'");
      }

      // 4. Column range spanning ColumnGutter -> ReferenceResolutionError (col === null, row === null)
      const cgModel = new Model({ name: "ColGutterFail" });
      const cgTable = cgModel.addSheet("Main").addTable("T", "T");
      cgTable
        .addColumn("2024E", { role: "measure", header: "2024E" })
        .addColumnGutter()
        .addColumn("2025E", { role: "measure", header: "2025E" });
      cgTable.addRow("r1", {
        cells: {
          "2024E": 10,
          "2025E": new Cell({ expr: 'SUM(T["r1", col.prev..@col])' }),
        },
      });
      try {
        cgModel.compile();
        expect.unreachable("Should have thrown ReferenceResolutionError");
      } catch (err) {
        if (!(err instanceof ReferenceResolutionError)) {
          expect.unreachable("Expected ReferenceResolutionError");
        }
        expect(err.coordinate).toBe('T["r1", "2025E"]');
        expect(err.targetTable).toBe("T");
        expect(err.row).toBeNull();
        expect(err.col).toBeNull();
        expect(err.message).toContain("a gutter column");
      }

      // 5. Column range spanning DescriptorColumn -> ReferenceResolutionError with col populated
      const dcModel = new Model({ name: "DescriptorColFail" });
      const dcTable = dcModel.addSheet("Main").addTable("T", "T");
      dcTable
        .addColumn("m1", { role: "measure", header: "M1" })
        .addColumn("notes", { role: "descriptor", header: "Notes" })
        .addColumn("m2", { role: "measure", header: "M2" });
      dcTable.addRow("r1", {
        cells: {
          m1: 10,
          notes: "memo",
          m2: new Cell({ expr: 'SUM(T["r1", col.prev..@col])' }),
        },
      });
      try {
        dcModel.compile();
        expect.unreachable("Should have thrown ReferenceResolutionError");
      } catch (err) {
        if (!(err instanceof ReferenceResolutionError)) {
          expect.unreachable("Expected ReferenceResolutionError");
        }
        expect(err.coordinate).toBe('T["r1", "m2"]');
        expect(err.targetTable).toBe("T");
        expect(err.row).toBeNull();
        expect(err.col).toBe("notes");
        expect(err.message).toContain("descriptor column 'notes'");
      }
    });

    it("(h) resolves cross-table relative offsets and populates mismatch/out-of-bounds ReferenceResolutionError", () => {
      // 1. Valid cross-table column & row offsets and range slices (Scenarios 1, 2, 3)
      const okModel = new Model({ name: "CrossTableOffsets", theme: "corporate_navy" });
      const sheet = okModel.addSheet("Main");

      const history = sheet.addTable("History", "History");
      history
        .addColumn("2022A", { role: "measure", header: "2022A" })
        .addColumn("2023A", { role: "measure", header: "2023A" })
        .addColumn("2024E", { role: "measure", header: "2024E" });
      history.addRow("rev", {
        cells: {
          "2022A": 100,
          "2023A": 150,
          "2024E": new Cell({ expr: 'Forecast["rev", col.next]' }),
        },
      });

      const detail = sheet.addTable("Detail", "Detail");
      detail
        .addColumn("2022A", { role: "measure", header: "2022A" })
        .addColumn("notes", { role: "descriptor", header: "Notes" })
        .addColumnGutter()
        .addColumn("2023A", { role: "measure", header: "2023A" })
        .addColumn("2024E", { role: "measure", header: "2024E" });
      detail.addRow("rev", {
        cells: { "2022A": 80, notes: "audited", "2023A": 120, "2024E": 160 },
      });

      const forecast = sheet.addTable("Forecast", "Forecast");
      forecast
        .addColumn("2024E", { role: "measure", header: "2024E" })
        .addColumn("2025E", { role: "measure", header: "2025E" });
      forecast.addRow("rev", {
        cells: {
          "2024E": new Cell({ expr: 'History["rev", col.prev]' }),
          "2025E": 300,
        },
      });
      forecast.addRow("cum_hist", {
        cells: {
          "2024E": new Cell({ expr: 'SUM(History["rev", "2022A"..col.prev])' }),
          "2025E": 0,
        },
      });
      forecast.addRow("detail_lag", {
        cells: {
          "2024E": new Cell({ expr: 'Detail["rev", col.prev]' }),
          "2025E": 0,
        },
      });
      forecast.addRow("detail_lag2", {
        cells: {
          "2024E": new Cell({ expr: 'Detail["rev", col.prev(2)]' }),
          "2025E": 0,
        },
      });

      const schedule = sheet.addTable("Schedule", "Schedule");
      schedule.addColumn("2024E", { role: "measure", header: "2024E" });
      schedule.addRow("r0", { cells: { "2024E": 10 } });
      schedule.addSectionHeader("Sec");
      schedule.addRow("r1", { cells: { "2024E": 20 } });
      schedule.addRowGutter();
      schedule.addRow("r2", { cells: { "2024E": 30 } });
      schedule.addRow("r3", { cells: { "2024E": 40 } });

      const driver = sheet.addTable("DriverTable", "Drivers");
      driver.addColumn("2024E", { role: "measure", header: "2024E" });
      driver.addRow("r1", { expr: "Schedule[row.prev, @col]" });
      driver.addRow("r3", {
        expr: 'Schedule[row.prev, @col] + SUM(Schedule["r2"..row.prev, @col])',
      });

      const compiled = okModel.compile();
      expect(compiled.num("Forecast", "rev", "2024E")).toBe(150);
      expect(compiled.num("Forecast", "cum_hist", "2024E")).toBe(250);
      expect(compiled.num("History", "rev", "2024E")).toBe(300);
      expect(compiled.num("Forecast", "detail_lag", "2024E")).toBe(120);
      expect(compiled.num("Forecast", "detail_lag2", "2024E")).toBe(80);
      expect(compiled.num("DriverTable", "r1", "2024E")).toBe(10);
      expect(compiled.num("DriverTable", "r3", "2024E")).toBe(60);
      expect(compiled.inspectCell("Forecast", "cum_hist", "2024E").excelFormula).toBe(
        "=SUM($B$5:C$5)",
      );

      // 2. Cross-table active column mismatch (scalar and range endpoint)
      for (const exprStr of [
        'Assumptions["tax", col.prev]',
        'SUM(Assumptions["tax", "val"..col.prev])',
      ]) {
        const colMm = new Model({ name: "ColMismatch" });
        const mmSheet = colMm.addSheet("Main");
        const assump = mmSheet.addTable("Assumptions", "Assumptions");
        assump.addColumn("val", { role: "measure", header: "Val" });
        assump.addRow("tax", { cells: { val: 0.21 } });
        const fcst = mmSheet.addTable("Forecast", "Forecast");
        fcst.addColumn("2024E", { role: "measure", header: "2024E" });
        fcst.addRow("ebitda", { expr: exprStr });
        try {
          colMm.compile();
          expect.unreachable("Should have thrown ReferenceResolutionError");
        } catch (err) {
          if (!(err instanceof ReferenceResolutionError)) {
            expect.unreachable("Expected ReferenceResolutionError");
          }
          expect(err.coordinate).toBe('Forecast["ebitda", "2024E"]');
          expect(err.targetTable).toBe("Assumptions");
          expect(err.col).toBe("2024E");
          expect(err.row).toBeNull();
        }
      }

      // 3. Cross-table active row mismatch (scalar and range endpoint)
      for (const exprStr of [
        'Assumptions[row.prev, "val"]',
        'SUM(Assumptions["tax"..row.prev, "val"])',
      ]) {
        const rowMm = new Model({ name: "RowMismatch" });
        const mmSheet = rowMm.addSheet("Main");
        const assump = mmSheet.addTable("Assumptions", "Assumptions");
        assump.addColumn("val", { role: "measure", header: "Val" });
        assump.addRow("tax", { cells: { val: 0.21 } });
        const fcst = mmSheet.addTable("Forecast", "Forecast");
        fcst.addColumn("2024E", { role: "measure", header: "2024E" });
        fcst.addRow("ebitda", { expr: exprStr });
        try {
          rowMm.compile();
          expect.unreachable("Should have thrown ReferenceResolutionError");
        } catch (err) {
          if (!(err instanceof ReferenceResolutionError)) {
            expect.unreachable("Expected ReferenceResolutionError");
          }
          expect(err.coordinate).toBe('Forecast["ebitda", "2024E"]');
          expect(err.targetTable).toBe("Assumptions");
          expect(err.row).toBe("ebitda");
          expect(err.col).toBeNull();
        }
      }

      // 4. Same-table and cross-table target out-of-bounds, target descriptor anchor,
      // multi-table disambiguation, and inverted range (Scenarios 3, 4, 5)
      const sameOobModel = new Model({ name: "SameTableOob" });
      const sameOobTbl = sameOobModel.addSheet("Main").addTable("Origin", "Origin");
      sameOobTbl.addColumn("2024E", { role: "measure", header: "2024E" });
      sameOobTbl.addRow("r1", {
        cells: { "2024E": new Cell({ expr: '@table["r1", col.prev]' }) },
      });
      try {
        sameOobModel.compile();
        expect.unreachable("Should have thrown ReferenceResolutionError");
      } catch (err) {
        if (!(err instanceof ReferenceResolutionError)) {
          expect.unreachable("Expected ReferenceResolutionError");
        }
        const sameTargetTable: string = err.targetTable;
        expect(sameTargetTable.toUpperCase()).toBe("ORIGIN");
        expect(sameTargetTable).toBe("Origin");
        expect(err.row).toBeNull();
        expect(err.col).toBeNull();
        expect(err.message).toContain(
          "Formula in cell 'Origin[\"r1\", \"2024E\"]' has temporal offset -1 which exceeds axis boundaries in table 'Origin'",
        );
      }

      for (const [originCol, exprStr, expectedOffsetStr] of [
        ["2024E", 'Target["r1", col.prev]', "-1"],
        ["2024E", 'Target[row.prev, "2024E"]', "-1"],
        ["label", 'Target["r1", col.prev]', "-1"],
        ["2024E", 'HistoryA["r1", col.prev] + Target["r1", col.prev]', "-1"],
        ["2025E", 'HistoryA["r1", col.next] + Target["r1", col.next]', "+1"],
        ["2024E", 'HistoryA[row.prev, "2024E"] + Target[row.prev, "2024E"]', "-1"],
        ["2024E", 'HistoryA[row.next, "2024E"] + Target[row.next, "2024E"]', "+1"],
        ["label", 'HistoryA["r1", col.prev] + Target["r1", col.prev]', "-1"],
      ] as const) {
        const oobModel = new Model({ name: "TargetOob" });
        const oobSheet = oobModel.addSheet("Main");

        const historyA = oobSheet.addTable("HistoryA", "History A");
        for (const colId of ["2023A", "label", "2024E", "2025E", "2026E"]) {
          historyA.addColumn(colId, { role: "measure", header: colId });
        }
        for (const [rowId, base] of [
          ["r0", 1],
          ["r1", 10],
          ["r2", 20],
        ] as const) {
          historyA.addRow(rowId, {
            cells: {
              "2023A": base,
              label: base + 1,
              "2024E": base + 2,
              "2025E": base + 3,
              "2026E": base + 4,
            },
          });
        }

        const targetTbl = oobSheet.addTable("Target", "Target");
        targetTbl
          .addColumn("label", { role: "descriptor", header: "Label" })
          .addColumn("2024E", { role: "measure", header: "2024E" })
          .addColumn("2025E", { role: "measure", header: "2025E" });
        targetTbl.addRow("r1", { cells: { label: "Item", "2024E": 10, "2025E": 20 } });

        const originTbl = oobSheet.addTable("Origin", "Origin");
        originTbl
          .addColumn("2023A", { role: "measure", header: "2023A" })
          .addColumn("label", { role: "descriptor", header: "Label" })
          .addColumn("2024E", { role: "measure", header: "2024E" })
          .addColumn("2025E", { role: "measure", header: "2025E" });
        originTbl.addRow("r0", {
          cells: { "2023A": 1, label: "Zero", "2024E": 2, "2025E": 3 },
        });
        const originCells: Record<string, number | string | Cell> = {
          "2023A": 4,
          label: "One",
          "2024E": 5,
          "2025E": 6,
        };
        originCells[originCol] = new Cell({ expr: exprStr });
        originTbl.addRow("r1", { cells: originCells });

        try {
          oobModel.compile();
          expect.unreachable("Should have thrown ReferenceResolutionError");
        } catch (err) {
          if (!(err instanceof ReferenceResolutionError)) {
            expect.unreachable("Expected ReferenceResolutionError");
          }
          const targetTableId: string = err.targetTable;
          expect(targetTableId.toUpperCase()).toBe("TARGET");
          expect(targetTableId).toBe("Target");
          expect(err.coordinate).toBe(`Origin["r1", "${originCol}"]`);
          expect(err.row).toBeNull();
          expect(err.col).toBeNull();
          expect(err.message).toContain(
            `Formula in cell 'Origin["r1", "${originCol}"]' has temporal offset ${expectedOffsetStr} which exceeds axis boundaries in table 'Target'`,
          );
        }
      }

      const invModel = new Model({ name: "TargetInverted" });
      const invSheet = invModel.addSheet("Main");
      const invTarget = invSheet.addTable("Target", "Target");
      invTarget
        .addColumn("label", { role: "descriptor", header: "Label" })
        .addColumn("2024E", { role: "measure", header: "2024E" })
        .addColumn("2025E", { role: "measure", header: "2025E" });
      invTarget.addRow("r1", { cells: { label: "Item", "2024E": 10, "2025E": 20 } });
      const invOrigin = invSheet.addTable("Origin", "Origin");
      invOrigin
        .addColumn("label", { role: "descriptor", header: "Label" })
        .addColumn("2024E", { role: "measure", header: "2024E" })
        .addColumn("2025E", { role: "measure", header: "2025E" });
      invOrigin.addRow("r1", {
        cells: {
          label: "Item",
          "2024E": 10,
          "2025E": new Cell({ expr: 'SUM(Target["r1", col.prev.."label"])' }),
        },
      });
      try {
        invModel.compile();
        expect.unreachable("Should have thrown ReferenceResolutionError");
      } catch (err) {
        if (!(err instanceof ReferenceResolutionError)) {
          expect.unreachable("Expected ReferenceResolutionError");
        }
        expect(err.coordinate).toBe('Origin["r1", "2025E"]');
        expect(err.targetTable).toBe("Target");
        expect(err.row).toBeNull();
        expect(err.col).toBeNull();
        expect(err.message.toLowerCase()).toContain("inverted range");
      }
    });

    it("(i) statically validates dormant lazy branches, propagates IFERROR FunctionError, and confines CallArg omission", () => {
      const invalidCallCases: readonly (readonly [string, string])[] = [
        ["IF(TRUE, 10, UNKNOWN_FN())", "unknown function 'UNKNOWN_FN'"],
        ["IF(FALSE, SUM(), 10)", "too few arguments for 'SUM'"],
        ["IF(TRUE, 10, ABS(1, 2))", "too many arguments for 'ABS'"],
        ["IF(TRUE, 10, IFS(TRUE, 1, FALSE, 2))", "unknown function 'IFS'"],
        ["IF(TRUE, 10, ABS(_))", "argument 'number' at position 1 of 'ABS' cannot be omitted"],
        ["IFS(TRUE, 1, FALSE, 2)", "unknown function 'IFS'"],
        ["IFERROR(10, UNKNOWN_FN())", "unknown function 'UNKNOWN_FN'"],
        ["IFERROR(ABS(1, 2, 3), 0)", "too many arguments for 'ABS'"],
        ["IFNA(10, 0)", "unknown function 'IFNA'"],
        ['COUNTA(@table["k1".."k2", @col])', "unknown function 'COUNTA'"],
        ["DAYS(45474, 45292)", "unknown function 'DAYS'"],
        ['ISBLANK(@table["k1", @col])', "unknown function 'ISBLANK'"],
        ['ISNA(@table["k1", @col])', "unknown function 'ISNA'"],
        ["CHOOSE(1, 10, SUM())", "too few arguments for 'SUM'"],
        ["SWITCH(1, 1, 10, 20)", "unknown function 'SWITCH'"],
        ["IF(FALSE, SWITCH(1, 1, 10, 20), 0)", "unknown function 'SWITCH'"],
        [
          'SUMIFS(@table["v1".."v2", @col], @table["k1".."k2", @col], 1)',
          "unknown function 'SUMIFS'",
        ],
        ['COUNTIFS(@table["k1".."k2", @col], 1)', "unknown function 'COUNTIFS'"],
        [
          'AVERAGEIFS(@table["v1".."v2", @col], @table["k1".."k2", @col], 1)',
          "unknown function 'AVERAGEIFS'",
        ],
        [
          'MINIFS(@table["v1".."v2", @col], @table["k1".."k2", @col], 1)',
          "unknown function 'MINIFS'",
        ],
        [
          'MAXIFS(@table["v1".."v2", @col], @table["k1".."k2", @col], 1)',
          "unknown function 'MAXIFS'",
        ],
        [
          'XLOOKUP(1, @table["k1".."k2", @col], @table["v1".."v2", @col], UNKNOWN_FN())',
          "unknown function 'UNKNOWN_FN'",
        ],
        ["IF((1 / 0) > 0, UNKNOWN_FN(), 10)", "unknown function 'UNKNOWN_FN'"],
        ["IF(TRUE, 10, -(1 + UNKNOWN_FN()))", "unknown function 'UNKNOWN_FN'"],
        ["IF(FALSE, 10)", "too few arguments for 'IF'"],
        ["IF(FALSE, 10, _)", "argument 'value_if_false' at position 3 of 'IF' cannot be omitted"],
        ["IF(TRUE, 10, _)", "argument 'value_if_false' at position 3 of 'IF' cannot be omitted"],
        [
          "IF(TRUE, 10, IF(FALSE, 1, _))",
          "argument 'value_if_false' at position 3 of 'IF' cannot be omitted",
        ],
        [
          'INDEX(@table["k1".."k2", @col], 2, _)',
          "trailing argument 'col_num' at position 3 of 'INDEX' cannot use '_'",
        ],
        [
          "YEARFRAC(45292, 45474, _)",
          "trailing argument 'basis' at position 3 of 'YEARFRAC' cannot use '_'",
        ],
        [
          'IRR(@table["k1".."k2", @col], _)',
          "trailing argument 'guess' at position 2 of 'IRR' cannot use '_'",
        ],
        [
          'XIRR(@table["v1".."v2", @col], @table["k1".."k2", @col], _)',
          "trailing argument 'guess' at position 3 of 'XIRR' cannot use '_'",
        ],
        ["PMT(0.05, 10, -1000, _)", "trailing argument 'fv' at position 4 of 'PMT' cannot use '_'"],
        [
          "PMT(0.05, 10, -1000, _, _)",
          "trailing argument 'fv' at position 4 of 'PMT' cannot use '_'",
        ],
        [
          'XMATCH(1, @table["k1".."k2", @col], _)',
          "trailing argument 'match_mode' at position 3 of 'XMATCH' cannot use '_'",
        ],
        [
          'XLOOKUP(1, @table["k1".."k2", @col], @table["v1".."v2", @col], _)',
          "trailing argument 'if_not_found' at position 4 of 'XLOOKUP' cannot use '_'",
        ],
        [
          'XLOOKUP(1, @table["k1".."k2", @col], @table["v1".."v2", @col], _, 0, _)',
          "trailing argument 'search_mode' at position 6 of 'XLOOKUP' cannot use '_'",
        ],
        ["ROUND(1.5, _)", "argument 'num_digits' at position 2 of 'ROUND' cannot be omitted"],
        ["IF(_, 10, _)", "argument 'logical_test' at position 1 of 'IF' cannot be omitted"],
        [
          'INDEX(@table["k1".."k2", @col], _, 2)',
          "argument 'row_num' at position 2 of 'INDEX' cannot be omitted",
        ],
      ];

      for (const [formula, expectedSubstr] of invalidCallCases) {
        const errModel = new Model({ name: "InvalidCallModel" });
        const sheet = errModel.addSheet("Main");
        const tbl = sheet.addTable("T", "Table");
        tbl.addColumn("c1", { role: "measure", header: "C1" });
        tbl.addRow("k1", { cells: { c1: 1 } });
        tbl.addRow("k2", { cells: { c1: 2 } });
        tbl.addRow("v1", { cells: { c1: 100 } });
        tbl.addRow("v2", { cells: { c1: 200 } });
        tbl.addRow("target", { expr: formula });

        try {
          errModel.compile();
          expect.unreachable(`Expected F8Error for formula: ${formula}`);
        } catch (err) {
          if (!(err instanceof F8Error)) {
            expect.unreachable("Expected F8Error");
          }
          expect(err.constructor).toBe(F8Error);
          expect(err.message).toContain(expectedSubstr);
        }
      }

      // Valid IFERROR domain error trapping, 2-arg XMATCH, and intermediate CallArg::Omitted lowering
      const okModel = new Model({ name: "ValidCallArgModel" });
      const okSheet = okModel.addSheet("Main");
      const okTbl = okSheet.addTable("T", "Table");
      okTbl.addColumn("c1", { role: "measure", header: "C1" });
      okTbl.addRow("k1", { cells: { c1: 100 } });
      okTbl.addRow("k2", { cells: { c1: 200 } });
      okTbl.addRow("v1", { cells: { c1: 10 } });
      okTbl.addRow("v2", { cells: { c1: 20 } });
      okTbl.addRow("iferror_ok", { expr: "IFERROR(10 / 0, 42)" });
      okTbl.addRow("xmatch_2arg", { expr: 'XMATCH(200, @table["k1".."k2", @col])' });
      okTbl.addRow("pmt_skip", { expr: "PMT(0.05, 10, -1000, _, 1)" });
      okTbl.addRow("pmt_explicit", { expr: "PMT(0.05, 10, -1000, 0, 1)" });
      okTbl.addRow("xlookup_omitted", {
        expr: 'XLOOKUP(100, @table["k1".."k2", @col], @table["v1".."v2", @col], _, 0, 1)',
      });
      okTbl.addRow("xlookup_2skip", {
        expr: 'XLOOKUP(100, @table["k1".."k2", @col], @table["v1".."v2", @col], _, _, -1)',
      });

      const compiled = okModel.compile();
      expect(compiled.num("T", "iferror_ok", "c1")).toBe(42);
      expect(compiled.num("T", "xmatch_2arg", "c1")).toBe(2);
      expect(compiled.num("T", "pmt_skip", "c1")).toBe(compiled.num("T", "pmt_explicit", "c1"));
      expect(compiled.num("T", "xlookup_omitted", "c1")).toBe(10);
      expect(compiled.num("T", "xlookup_2skip", "c1")).toBe(10);

      const xmatchCell = compiled.inspectCell("T", "xmatch_2arg", "c1");
      expect(xmatchCell.evaluatedValue).toEqual({ Number: 2 });
      expect(xmatchCell.excelFormula).not.toBeNull();
      expect(xmatchCell.excelFormula?.startsWith("=XMATCH(200,")).toBe(true);

      const pmtCell = compiled.inspectCell("T", "pmt_skip", "c1");
      expect(pmtCell.excelFormula).toBe("=PMT(0.05,10,-1000,,1)");

      const xlookupCell = compiled.inspectCell("T", "xlookup_omitted", "c1");
      expect(xlookupCell.excelFormula).not.toBeNull();
      expect(xlookupCell.excelFormula?.startsWith("=XLOOKUP(100,")).toBe(true);
      expect(xlookupCell.excelFormula?.endsWith(",,0,1)")).toBe(true);

      const xlookup2SkipCell = compiled.inspectCell("T", "xlookup_2skip", "c1");
      expect(xlookup2SkipCell.excelFormula).not.toBeNull();
      expect(xlookup2SkipCell.excelFormula?.startsWith("=XLOOKUP(100,")).toBe(true);
      expect(xlookup2SkipCell.excelFormula?.endsWith(",,,-1)")).toBe(true);

      // Invalid bare `_` outside direct function argument positions throws FormulaParseError
      for (const badSyntax of ["_", "1 + _", "IF(TRUE, 10, _ + 1)", "IF(TRUE, 10, (_))"]) {
        const syntaxModel = new Model({ name: "InvalidUnderscoreSyntax" });
        const syntaxSheet = syntaxModel.addSheet("Main");
        const syntaxTbl = syntaxSheet.addTable("T", "Table");
        syntaxTbl.addColumn("c1", { role: "measure", header: "C1" });
        syntaxTbl.addRow("r1", { expr: badSyntax });

        expect(() => syntaxModel.compile()).toThrow(FormulaParseError);
      }
    });

    it("(j) enforces Cluster A1 strict operator, scalar, slice, ISBLANK, and NA contracts", () => {
      // 1. Grammar strictness: unary `+`, chained comparisons, and invalid prev/next offsets -> FormulaParseError
      for (const badParse of [
        "+10",
        "+TRUE",
        "1 = 1 = TRUE",
        "1 < 2 < 3",
        "@table[row.prev(0), @col]",
        "@table[@row, col.next(0)]",
        "@table[@row, col.prev(5000000000)]",
      ]) {
        const parseModel = new Model({ name: "ParseStrictModel" });
        const parseTbl = parseModel.addSheet("Main").addTable("T", "Parse Strict");
        parseTbl.addColumn("c1", { role: "measure", header: "C1" });
        parseTbl.addRow("r1", { expr: badParse });

        let caught: unknown;
        try {
          parseModel.compile();
        } catch (err: unknown) {
          caught = err;
        }
        expect(caught).toBeInstanceOf(FormulaParseError);
      }

      // 2. Type, EmptyFormulaResult, EmptyArgument, SliceInScalarContext, and ParamKind::Range -> exact F8Error (never TypeError)
      for (const [badF8, expectedSubstr] of [
        ['@table["a", @col]', "formula cannot evaluate to an empty cell"],
        ['INDEX(@table["a".."a", @col], 1)', "evaluated to an empty cell"],
        [
          'XLOOKUP("match", @table["k".."k", @col], @table["a".."a", @col])',
          "evaluated to an empty cell",
        ],
        [
          'XLOOKUP("miss", @table["k".."k", @col], @table["a".."a", @col], @table["a", @col])',
          "evaluated to an empty cell",
        ],
        ['IF(TRUE, @table["a", @col], 1)', "evaluated to an empty cell"],
        ['CHOOSE(1, @table["a", @col], 1)', "evaluated to an empty cell"],
        ['IFERROR(@table["a", @col], 1)', "evaluated to an empty cell"],
        ['ISERROR(@table["a", @col])', "evaluated to an empty cell"],
        ['"5" + 1', "expected number, found text"],
        ["TRUE * 10", "expected number, found boolean"],
        ['@table["a", @col] + 1', "evaluated to an empty cell"],
        ["IF(1, 10, 20)", "expected boolean, found number"],
        ['AND(@table["a", @col], TRUE)', "evaluated to an empty cell"],
        ['100 < "1"', "cannot compare '<' operands of types number and text"],
        ['"" < FALSE', "cannot compare '<' operands of types text and boolean"],
        ['@table["a", @col] = ""', "evaluated to an empty cell"],
        ['@table["a", @col] = FALSE', "evaluated to an empty cell"],
        ['IFERROR(@table["a", @col] + 1, 0)', "evaluated to an empty cell"],
        ['@table["r1".."r2", @col]', "slice"],
        ['@table["r1".."r1", @col]', "slice"],
        ['ISERROR(@table["r1".."r1", @col])', "slice"],
        ["COUNT(10)", "expected range"],
        ['COUNT(@table["a", @col])', "evaluated to an empty cell"],
        ['SUMPRODUCT("abc")', "expected range"],
        ["SWITCH(1, 1, 10, 20)", "unknown function 'SWITCH'"],
      ] as const) {
        const errModel = new Model({ name: "StrictErrorModel" });
        const errTbl = errModel.addSheet("Main").addTable("T", "Strict Error");
        errTbl.addColumn("c1", { role: "measure", header: "C1" });
        errTbl.addRow("a", { cells: { c1: null } });
        errTbl.addRow("k", { cells: { c1: "match" } });
        errTbl.addRow("r1", { cells: { c1: 10 } });
        errTbl.addRow("r2", { cells: { c1: 20 } });
        errTbl.addRow("target", { expr: badF8 });

        let caught: unknown;
        try {
          errModel.compile();
        } catch (err: unknown) {
          caught = err;
        }
        expect(caught).toBeInstanceOf(F8Error);
        expect(caught instanceof F8Error && caught.constructor === F8Error).toBe(true);
        expect(caught instanceof TypeError).toBe(false);
        if (caught instanceof F8Error) {
          expect(caught.message).toContain(expectedSubstr);
        }
      }

      // 3. Valid ISERROR, NA(), dormant empty branches, parenthesized comparisons, and sparse Slice AND
      const model = new Model({ name: "IserrorAndNaModel", theme: "corporate_navy" });
      const tbl = model
        .addSheet("Main")
        .addTable("T", "Blank Guard & Information", { currency: "USD" });
      tbl.addColumn("c1", { role: "measure", header: "C1" });

      tbl.addRow("a", { cells: { c1: null }, format: "currency", decimals: 1 });
      tbl.addRow("k", { cells: { c1: "match" } });
      tbl.addRow("f1", { cells: { c1: null } });
      tbl.addRow("f2", { cells: { c1: true } });
      tbl.addRow("b", {
        expr: 'IF(FALSE, @table["a", @col], 0)',
        format: "currency",
        decimals: 1,
      });
      tbl.addRow("if_guard_a", {
        expr: 'IF(FALSE, @table["a", @col], "")',
        format: "currency",
        decimals: 1,
      });
      tbl.addRow("ie_guarded", { expr: 'ISERROR(@table["b", @col])' });
      tbl.addRow("ie_empty_str", { expr: 'ISERROR("")' });
      tbl.addRow("ie_div0", { expr: "ISERROR(1 / 0)" });
      tbl.addRow("na_cell", { expr: "NA()" });
      tbl.addRow("na_trapped", { expr: "IFERROR(IF(FALSE, 10, NA()), 0)" });
      tbl.addRow("paren_cmp", { expr: "(1 = 1) = TRUE" });
      tbl.addRow("and_sparse", { expr: 'AND(@table["f1".."f2", @col])' });
      tbl.addRow("b_eq_zero", { expr: '(@table["b", @col] = 0)' });
      tbl.addRow("count_a", { expr: 'COUNT(@table["a".."a", @col])' });
      tbl.addRow("count_b", { expr: 'COUNT(@table["b", @col])' });

      const compiled = model.compile();

      // Literal null cell remains Empty; .num() still throws host TypeError
      expect(compiled.value("T", "a", "c1")).toBeNull();
      expect(() => compiled.num("T", "a", "c1")).toThrow(TypeError);
      expect(compiled.formatted("T", "a", "c1")).toBe("");
      expect(compiled.inspectCell("T", "a", "c1").evaluatedValue).toBe("Empty");

      // Guarded formula cell evaluates to +0
      expect(Object.is(compiled.value("T", "b", "c1"), 0)).toBe(true);
      expect(Object.is(compiled.num("T", "b", "c1"), 0)).toBe(true);
      expect(compiled.formatted("T", "b", "c1")).toBe("$0.0");

      expect(compiled.value("T", "if_guard_a", "c1")).toBe("");
      expect(compiled.formatted("T", "if_guard_a", "c1")).toBe("");
      expect(compiled.value("T", "ie_guarded", "c1")).toBe(false);
      expect(compiled.value("T", "ie_empty_str", "c1")).toBe(false);
      expect(compiled.value("T", "ie_div0", "c1")).toBe(true);

      const naInfo = compiled.inspectCell("T", "na_cell", "c1");
      expect(compiled.value("T", "na_cell", "c1")).toBe("#N/A");
      expect(naInfo.excelFormula).toBe("=NA()");
      expect(() => compiled.num("T", "na_cell", "c1")).toThrow(TypeError);
      expect(compiled.num("T", "na_trapped", "c1")).toBe(0);

      expect(compiled.value("T", "paren_cmp", "c1")).toBe(true);
      expect(compiled.value("T", "and_sparse", "c1")).toBe(true);
      expect(compiled.value("T", "b_eq_zero", "c1")).toBe(true);
      expect(compiled.num("T", "count_a", "c1")).toBe(0);
      expect(compiled.num("T", "count_b", "c1")).toBe(1);
    });

    it("(k) normalizes negative zero to +0", () => {
      const model = new Model({ name: "NegZeroModel" });
      const tbl = model.addSheet("Main").addTable("T", "Neg Zero");
      tbl.addColumn("num", { role: "measure", header: "Number" });

      tbl.addRow("r1", { cells: { num: 42 } });
      tbl.addRow("r2", { cells: { num: -0 } });

      const compiled = model.compile();
      expect(Object.is(compiled.value("T", "r2", "num"), 0)).toBe(true);
    });

    it("(l) enforces Cluster A2 monomorphic branches, dormant memoized validation, and lookup/index strictness", () => {
      // 1. Stage A & Stage B rejections throw exact F8Error (never host TypeError or 'dormant')
      for (const badA2 of [
        "IF(TRUE, 10)",
        "SWITCH(1, 1, 10, 20)",
        "IF(FALSE, SWITCH(1, 1, 10, 20), 0)",
        'IF(TRUE, 10, "NM")',
        'IFERROR(10 / 0, "N/A")',
        'CHOOSE(1, 10, "NM")',
        'IF(FALSE, IF(TRUE, "a", "b") = IF(FALSE, 1, 2), TRUE)',
        'ROUND(10, IF(TRUE, "a", "b"))',
        'ROUND(10, IF(TRUE, 2, "bad"))',
        '1 + INDEX(@table["txt1".."txt2", @col], @table["dyn_oob_idx", @col])',
        '1 = XLOOKUP("Missing", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col])',
        'ABS(XLOOKUP("Missing", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col]))',
        'IF(XLOOKUP("Missing", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col]), 10, 20)',
        'XMATCH(IF(FALSE, "Alpha", NA()), @table["num1".."num2", @col])',
        'XMATCH(INDEX(@table["txt1".."txt2", @col], @table["dyn_oob_idx", @col]), @table["num1".."num2", @col])',
        'XLOOKUP(XLOOKUP("Missing", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col]), @table["num1".."num2", @col], @table["num1".."num2", @col])',
        '1 + XLOOKUP("Beta", @table["cat1".."cat2", @col], @table["num1".."num2", @col], "NM")',
        'IFERROR(@table["null_cell", @col], 1)',
        'IFERROR(1 / 0, @table["null_cell", @col])',
        'ISERROR(IFERROR(@table["null_cell", @col], 1))',
        'IF(TRUE, @table["num1", @col], @table["txt1", @col])',
        'IF(@table["flag_false", @col], @table["txt1", @col] + 5, 0)',
        'IF(@table["flag_false", @col], @table["num1", @col] < @table["txt1", @col], FALSE)',
        'IF(TRUE, 10, XLOOKUP("Beta", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col]))',
        'IF(TRUE, 10, INDEX(@table["txt1".."txt2", @col], @table["dyn_idx", @col]))',
        'IFERROR(INDEX(@table["txt1".."txt2", @col], @table["dyn_oob_idx", @col]), 10)',
        'IFERROR(XLOOKUP("Missing", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col]), 10)',
        'IFERROR(@table["err_txt", @col], 10)',
        'IFERROR(@table["err_oob_txt", @col], 10)',
        '1 + @table["err_txt", @col]',
        '@table["err_txt", @col] = 10',
        'IFERROR(@table["err_div0", @col], "fallback")',
        'AND(@table["c1".."err_txt", @col])',
        'AND(@table["err_txt".."err_txt", @col])',
        'IF(FALSE, AND(@table["c1".."err_txt", @col]), FALSE)',
        'XLOOKUP(10, @table["num_adj".."err_txt", @col], @table["txt1".."txt2", @col])',
        'XMATCH(10, @table["err_txt".."err_txt", @col])',
        'XMATCH(10, @table["err_txt".."err_txt", @col], 0, 2)',
        'INDEX(@table["num_adj".."err_txt", @col], 1)',
        'XLOOKUP("Alpha", @table["cat1".."cat2", @col], @table["num_adj".."err_txt", @col])',
        'IFERROR(INDEX(@table["err_div0".."err_div0_2", @col], 1), "fallback")',
        'XMATCH("Alpha", @table["txt1".."txt2", @col], "abc")',
        'INDEX(@table["num1".."txt1", @col], 1)',
      ]) {
        const errModel = new Model({ name: "ClusterA2ErrorModel" });
        const errTbl = errModel.addSheet("Main").addTable("T", "Cluster A2 Error");
        errTbl.addColumn("c1", { role: "measure", header: "C1" });
        errTbl.addRow("null_cell", { cells: { c1: null } });
        errTbl.addRow("num1", { cells: { c1: 10 } });
        errTbl.addRow("num2", { cells: { c1: 20 } });
        errTbl.addRow("txt1", { cells: { c1: "Alpha" } });
        errTbl.addRow("txt2", { cells: { c1: "Beta" } });
        errTbl.addRow("cat1", { cells: { c1: "Alpha" } });
        errTbl.addRow("cat2", { cells: { c1: "Beta" } });
        errTbl.addRow("flag_false", { cells: { c1: false } });
        errTbl.addRow("flt_cell", { cells: { c1: 2.5 } });
        errTbl.addRow("dyn_idx", { expr: "1 + 0" });
        errTbl.addRow("dyn_oob_idx", { expr: "5 + 0" });
        if (badA2.includes('"c1".."err_txt"')) {
          errTbl.addRow("num_adj", { cells: { c1: 10 } });
          errTbl.addRow("c1", { cells: { c1: true } });
        } else {
          errTbl.addRow("c1", { cells: { c1: true } });
          errTbl.addRow("num_adj", { cells: { c1: 10 } });
        }
        errTbl.addRow("err_txt", {
          expr: 'XLOOKUP("Missing", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col])',
        });
        errTbl.addRow("err_oob_txt", {
          expr: 'INDEX(@table["txt1".."txt2", @col], @table["dyn_oob_idx", @col])',
        });
        errTbl.addRow("err_div0", { expr: "1 / 0" });
        errTbl.addRow("err_div0_2", { expr: "2 / 0" });
        errTbl.addRow("unsort1", { cells: { c1: 10 } });
        errTbl.addRow("unsort2", { cells: { c1: 30 } });
        errTbl.addRow("unsort3", { cells: { c1: 20 } });
        errTbl.addRow("errk1", { cells: { c1: 10 } });
        errTbl.addRow("errk2", { expr: "1 / 0" });
        errTbl.addRow("target", { expr: badA2 });

        let caught: unknown;
        try {
          errModel.compile();
        } catch (err: unknown) {
          caught = err;
        }
        expect(caught).toBeInstanceOf(F8Error);
        expect(caught instanceof F8Error && caught.constructor === F8Error).toBe(true);
        expect(caught instanceof TypeError).toBe(false);
        expect(caught instanceof F8Error && !caught.message.includes("dormant")).toBe(true);
      }

      // 2. Valid monomorphic branches, taken integer arm, error ValueType provenance, and binary search
      const okModel = new Model({ name: "ClusterA2ValidModel" });
      const okTbl = okModel.addSheet("Main").addTable("T", "Cluster A2 Valid");
      okTbl.addColumn("c1", { role: "measure", header: "C1" });
      okTbl.addRow("null_cell", { cells: { c1: null } });
      okTbl.addRow("flt_cell", { cells: { c1: 2.5 } });
      okTbl.addRow("cat1", { cells: { c1: "Alpha" } });
      okTbl.addRow("cat2", { cells: { c1: "Beta" } });
      okTbl.addRow("txt1", { cells: { c1: "one" } });
      okTbl.addRow("txt2", { cells: { c1: "two" } });
      okTbl.addRow("sp1", { cells: { c1: 10 } });
      okTbl.addRow("sp2", { cells: { c1: null } });
      okTbl.addRow("sp3", { cells: { c1: 30 } });
      okTbl.addRow("asc1", { cells: { c1: 10 } });
      okTbl.addRow("asc2", { cells: { c1: 20 } });
      okTbl.addRow("asc3", { cells: { c1: 30 } });
      okTbl.addRow("asc4", { cells: { c1: 40 } });
      okTbl.addRow("desc1", { cells: { c1: 40 } });
      okTbl.addRow("desc2", { cells: { c1: 30 } });
      okTbl.addRow("desc3", { cells: { c1: 20 } });
      okTbl.addRow("desc4", { cells: { c1: 10 } });
      okTbl.addRow("if_text", { expr: 'IF(TRUE, "PASS", "FAIL")' });
      okTbl.addRow("if_na_bottom", { expr: "IF(10 > 0, 100 / 10, NA())" });
      okTbl.addRow("round_taken_lit", { expr: "ROUND(10, IF(TRUE, 2, 2.5))" });
      okTbl.addRow("round_taken_cell", {
        expr: 'ROUND(10, IF(TRUE, 2, @table["flt_cell", @col]))',
      });
      okTbl.addRow("round_guard_cell", {
        expr: 'IF(FALSE, ROUND(10, @table["flt_cell", @col]), 0)',
      });
      okTbl.addRow("helper_guard", {
        expr: 'IF(TRUE, 0, @table["null_cell", @col] + 5)',
      });
      okTbl.addRow("index_sparse", { expr: 'INDEX(@table["sp1".."sp3", @col], 3)' });
      okTbl.addRow("xmatch_bin_asc", { expr: 'XMATCH(30, @table["asc1".."asc4", @col], 0, 2)' });
      okTbl.addRow("xlookup_bin_desc", {
        expr: 'XLOOKUP(25, @table["desc1".."desc4", @col], @table["asc1".."asc4", @col], -1, -1, -2)',
      });
      okTbl.addRow("add_na", { expr: "1 + IF(FALSE, 10, NA())" });
      okTbl.addRow("xmatch_na", { expr: 'XMATCH(NA(), @table["asc1".."asc2", @col])' });
      okTbl.addRow("err_txt", {
        expr: 'XLOOKUP("Missing", @table["cat1".."cat2", @col], @table["txt1".."txt2", @col])',
      });
      okTbl.addRow("err_div0", { expr: "1 / 0" });
      okTbl.addRow("err_div0_2", { expr: "2 / 0" });
      okTbl.addRow("c1", { cells: { c1: true } });
      okTbl.addRow("err_na", { expr: "NA()" });
      okTbl.addRow("ok_iferr_txt", { expr: 'IFERROR(@table["err_txt", @col], "fallback")' });
      okTbl.addRow("ok_iferr_div0", { expr: 'IFERROR(@table["err_div0", @col], 0)' });
      okTbl.addRow("ok_iferr_idx_div0", {
        expr: 'IFERROR(INDEX(@table["err_div0".."err_div0_2", @col], 1), 0)',
      });
      okTbl.addRow("ok_add_div0", { expr: '1 + @table["err_div0", @col]' });
      okTbl.addRow("ok_iferr_na_num", { expr: 'IFERROR(@table["err_na", @col], 10)' });
      okTbl.addRow("ok_iferr_na_txt", { expr: 'IFERROR(@table["err_na", @col], "fallback")' });
      okTbl.addRow("ok_and_na_slice", { expr: 'AND(@table["c1".."err_na", @col])' });

      const compiled = okModel.compile();
      expect(compiled.value("T", "if_text", "c1")).toBe("PASS");
      expect(compiled.num("T", "if_na_bottom", "c1")).toBe(10);
      expect(compiled.num("T", "round_taken_lit", "c1")).toBe(10);
      expect(compiled.num("T", "round_taken_cell", "c1")).toBe(10);
      expect(compiled.num("T", "round_guard_cell", "c1")).toBe(0);
      expect(compiled.num("T", "helper_guard", "c1")).toBe(0);
      expect(compiled.num("T", "index_sparse", "c1")).toBe(30);
      expect(compiled.num("T", "xmatch_bin_asc", "c1")).toBe(3);
      expect(compiled.num("T", "xlookup_bin_desc", "c1")).toBe(30);
      expect(compiled.value("T", "add_na", "c1")).toBe("#N/A");
      expect(compiled.value("T", "xmatch_na", "c1")).toBe("#N/A");
      expect(compiled.value("T", "ok_iferr_txt", "c1")).toBe("fallback");
      expect(compiled.num("T", "ok_iferr_div0", "c1")).toBe(0);
      expect(compiled.num("T", "ok_iferr_idx_div0", "c1")).toBe(0);
      expect(compiled.value("T", "ok_add_div0", "c1")).toBe("#DIV/0!");
      expect(compiled.num("T", "ok_iferr_na_num", "c1")).toBe(10);
      expect(compiled.value("T", "ok_iferr_na_txt", "c1")).toBe("fallback");
      expect(compiled.value("T", "ok_and_na_slice", "c1")).toBe("#N/A");
    });

    it("(m) enforces Cluster B descriptor roles, sparse slice aggregation, and strict date/annuity/basis domains", () => {
      // 1. Stage A & Stage B rejections throw exact F8Error (never host TypeError or 'dormant')
      const badClusterB: readonly (readonly [string, string])[] = [
        // Scenario 1: Descriptor references in numeric/financial aggregations
        ['SUM(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"],
        ['SUM(@table["r1", "line_item"])', "cannot reference descriptor column"],
        ['AVERAGE(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"],
        ['MIN(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"],
        ['MAX(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"],
        ['MEDIAN(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"],
        ['COUNT(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"],
        [
          'SUMPRODUCT(@table["r1".."r3", "line_item"], @table["r1".."r3", "c1"])',
          "cannot reference descriptor column",
        ],
        ['NPV(0.1, @table["r1".."r3", "line_item"])', "cannot reference descriptor column"],
        ['IRR(@table["r1".."r3", "line_item"])', "cannot reference descriptor column"],
        [
          'XNPV(0.1, @table["r1".."r3", "line_item"], @table["d1".."d3", "c1"])',
          "cannot reference descriptor column",
        ],
        [
          'XIRR(@table["cf1".."cf3", "c1"], @table["r1".."r3", "line_item"])',
          "cannot reference descriptor column",
        ],
        [
          'IF(FALSE, SUM(@table["r1".."r3", "line_item"]), 0)',
          "cannot reference descriptor column",
        ],
        // Scenario 2: Non-numeric cells, SingleCell Empty refs, and NPV/IRR/XNPV/XIRR Empty holes
        ['SUM(@table["txt1".."txt2", "c1"])', "expected number, found text"],
        ['SUM(@table["bool1".."bool2", "c1"])', "expected number, found boolean"],
        ["SUM(TRUE, 5)", "expected number, found boolean"],
        ['SUM(@table["null1", "c1"])', "evaluated to an empty cell"],
        ['AVERAGE(@table["r1", "c1"], @table["null1", "c1"])', "evaluated to an empty cell"],
        ['MIN(@table["txt1".."txt2", "c1"])', "expected number, found text"],
        ['MAX(@table["null1", "c1"])', "evaluated to an empty cell"],
        ['MEDIAN(@table["bool1".."bool2", "c1"])', "expected number, found boolean"],
        ['COUNT(@table["txt1".."txt2", "c1"])', "expected number, found text"],
        ['COUNT(@table["null1", "c1"])', "evaluated to an empty cell"],
        [
          'SUMPRODUCT(@table["txt1".."txt2", "c1"], @table["r1".."r2", "c1"])',
          "expected number, found text",
        ],
        [
          'SUMPRODUCT(@table["bool1".."bool2", "c1"], @table["r1".."r2", "c1"])',
          "expected number, found boolean",
        ],
        ['IFERROR(SUM(@table["null1", "c1"]), 0)', "evaluated to an empty cell"],
        ['NPV(0.1, @table["cf_hole1".."cf_hole3", "c1"])', "evaluated to an empty cell"],
        ['IRR(@table["cf_hole1".."cf_hole3", "c1"])', "evaluated to an empty cell"],
        [
          'XNPV(0.1, @table["cf_hole1".."cf_hole3", "c1"], @table["d1".."d3", "c1"])',
          "evaluated to an empty cell",
        ],
        [
          'XIRR(@table["cf1".."cf3", "c1"], @table["d_hole1".."d_hole3", "c1"])',
          "evaluated to an empty cell",
        ],
        // Scenario 5: ShapePolicy precedence
        [
          'XNPV(-2.0, @table["r1".."r2", "c1".."c2"], @table["r1".."r2", "c1".."c2"])',
          "requires a 1D slice, found 2x2",
        ],
        [
          'XNPV(0.1, @table["cf1".."cf3", "c1"], @table["zero_serial".."zero_serial", "c1"])',
          "length mismatch in 'XNPV'",
        ],
      ];

      for (const [formula, expectedSubstr] of badClusterB) {
        const errModel = new Model({ name: "ClusterBErrorModel" });
        const errTbl = errModel.addSheet("Main").addTable("T", "Cluster B Error");
        errTbl
          .addColumn("line_item", { role: "descriptor", header: "Item" })
          .addColumn("c1", { role: "measure", header: "C1" })
          .addColumn("c2", { role: "measure", header: "C2" });
        errTbl.addRow("r1", { cells: { line_item: "Rev", c1: 10, c2: 1 } });
        errTbl.addRow("r2", { cells: { line_item: "Cost", c1: 20, c2: 2 } });
        errTbl.addRow("r3", { cells: { line_item: "OpEx", c1: 30, c2: 3 } });
        errTbl.addRow("txt1", { cells: { line_item: "T1", c1: "Alpha", c2: 0 } });
        errTbl.addRow("txt2", { cells: { line_item: "T2", c1: "Beta", c2: 0 } });
        errTbl.addRow("bool1", { cells: { line_item: "B1", c1: true, c2: 0 } });
        errTbl.addRow("bool2", { cells: { line_item: "B2", c1: false, c2: 0 } });
        errTbl.addRow("null1", { cells: { line_item: "N1", c1: null, c2: 0 } });
        errTbl.addRow("cf1", { cells: { line_item: "CF1", c1: -1000, c2: 0 } });
        errTbl.addRow("cf2", { cells: { line_item: "CF2", c1: 600, c2: 0 } });
        errTbl.addRow("cf3", { cells: { line_item: "CF3", c1: 600, c2: 0 } });
        errTbl.addRow("cf_hole1", { cells: { line_item: "CFH1", c1: -1000, c2: 0 } });
        errTbl.addRow("cf_hole2", { cells: { line_item: "CFH2", c1: null, c2: 0 } });
        errTbl.addRow("cf_hole3", { cells: { line_item: "CFH3", c1: 1200, c2: 0 } });
        errTbl.addRow("d1", { cells: { line_item: "D1", c1: 45292, c2: 0 } });
        errTbl.addRow("d2", { cells: { line_item: "D2", c1: 45658, c2: 0 } });
        errTbl.addRow("d3", { cells: { line_item: "D3", c1: 46023, c2: 0 } });
        errTbl.addRow("d_hole1", { cells: { line_item: "DH1", c1: 45292, c2: 0 } });
        errTbl.addRow("d_hole2", { cells: { line_item: "DH2", c1: null, c2: 0 } });
        errTbl.addRow("d_hole3", { cells: { line_item: "DH3", c1: 46023, c2: 0 } });
        errTbl.addRow("yr_ok", { cells: { line_item: "YOK", c1: 2024, c2: 0 } });
        errTbl.addRow("yr_24", { cells: { line_item: "Y24", c1: 24, c2: 0 } });
        errTbl.addRow("day_29", { cells: { line_item: "D29", c1: 29, c2: 0 } });
        errTbl.addRow("zero_serial", { cells: { line_item: "ZS", c1: 0, c2: 0 } });
        errTbl.addRow("target", {
          cells: { line_item: "Target", c1: new Cell({ expr: formula }), c2: 0 },
        });

        let caught: unknown;
        try {
          errModel.compile();
        } catch (err: unknown) {
          caught = err;
        }
        expect(caught).toBeInstanceOf(F8Error);
        expect(caught instanceof F8Error && caught.constructor === F8Error).toBe(true);
        expect(caught instanceof TypeError).toBe(false);
        if (caught instanceof F8Error) {
          expect(caught.message.includes("dormant")).toBe(false);
          expect(caught.message).toContain(expectedSubstr);
        }
      }

      // 2. Valid descriptor lookups, sparse slices, COUNT error skipping, overflow #NUM!, and date parity
      const okModel = new Model({ name: "ClusterBValidModel", theme: "corporate_navy" });
      const okTbl = okModel.addSheet("Main").addTable("T", "Cluster B Valid");
      okTbl
        .addColumn("line_item", { role: "descriptor", header: "Item" })
        .addColumn("c1", { role: "measure", header: "C1" })
        .addColumn("w", { role: "measure", header: "Weight" });

      okTbl.addRow("r1", { cells: { line_item: "Rev", c1: 10, w: 1 } });
      okTbl.addRow("r2", { cells: { line_item: "Cost", c1: 20, w: 2 } });
      okTbl.addRow("r3", { cells: { line_item: "OpEx", c1: 30, w: 3 } });

      okTbl.addRow("sp1", { cells: { line_item: "S1", c1: null, w: 5 } });
      okTbl.addRow("sp2", { cells: { line_item: "S2", c1: 0, w: 2 } });
      okTbl.addRow("sp3", { cells: { line_item: "S3", c1: 20, w: 4 } });
      okTbl.addRow("sp4", { cells: { line_item: "S4", c1: null, w: 9 } });

      okTbl.addRow("an1", { cells: { line_item: "AN1", c1: null, w: 0 } });
      okTbl.addRow("an2", { cells: { line_item: "AN2", c1: null, w: 0 } });

      okTbl.addRow("er1", { cells: { line_item: "E1", c1: 10, w: 0 } });
      okTbl.addRow("er2", { cells: { line_item: "E2", c1: new Cell({ expr: "1 / 0" }), w: 0 } });
      okTbl.addRow("er3", { cells: { line_item: "E3", c1: 20, w: 0 } });

      okTbl.addRow("big1", { cells: { line_item: "BG1", c1: 1e308, w: 1e200 } });
      okTbl.addRow("big2", { cells: { line_item: "BG2", c1: 1e308, w: 1e200 } });

      okTbl.addRow("yr_ok", { cells: { line_item: "Y", c1: 2024, w: 0 } });
      okTbl.addRow("mo_feb", { cells: { line_item: "M", c1: 2, w: 0 } });
      okTbl.addRow("day_29", { cells: { line_item: "D", c1: 29, w: 0 } });
      okTbl.addRow("type_1", { cells: { line_item: "T", c1: 1, w: 0 } });
      okTbl.addRow("basis_3", { cells: { line_item: "B", c1: 3, w: 0 } });
      okTbl.addRow("zero_date", { cells: { line_item: "ZD", c1: 0, w: 0 }, format: "date" });

      const addCheck = (rowId: string, formula: string): void => {
        okTbl.addRow(rowId, {
          cells: { line_item: rowId, c1: new Cell({ expr: formula }), w: 0 },
        });
      };

      addCheck("desc_xmatch", 'XMATCH("OpEx", @table["r1".."r3", "line_item"])');
      addCheck(
        "desc_xlookup",
        'XLOOKUP("Rev", @table["r1".."r3", "line_item"], @table["r1".."r3", "c1"])',
      );
      addCheck("desc_index", 'INDEX(@table["r1".."r3", "line_item"], 2)');
      addCheck("guard_sum", 'IF(TRUE, 0, SUM(@table["sp1", "c1"]))');

      addCheck("sum_sparse", 'SUM(@table["sp1".."sp4", "c1"])');
      addCheck("avg_sparse", 'AVERAGE(@table["sp1".."sp4", "c1"])');
      addCheck("cnt_sparse", 'COUNT(@table["sp1".."sp4", "c1"])');
      addCheck("sp_sparse", 'SUMPRODUCT(@table["sp1".."sp4", "c1"], @table["sp1".."sp4", "w"])');

      addCheck("sum_all_null", 'SUM(@table["an1".."an2", "c1"])');
      addCheck("avg_all_null", 'AVERAGE(@table["an1".."an2", "c1"])');
      addCheck("med_all_null", 'MEDIAN(@table["an1".."an2", "c1"])');
      addCheck("min_all_null", 'MIN(@table["an1".."an2", "c1"])');
      addCheck("max_all_null", 'MAX(@table["an1".."an2", "c1"])');
      addCheck("cnt_all_null", 'COUNT(@table["an1".."an2", "c1"])');

      addCheck("cnt_err_slice", 'COUNT(@table["er1".."er3", "c1"])');
      addCheck("sum_err_slice", 'SUM(@table["er1".."er3", "c1"])');

      addCheck("sum_ovf", 'SUM(@table["big1".."big2", "c1"])');
      addCheck("sp_ovf", 'SUMPRODUCT(@table["big1".."big1", "w"], @table["big2".."big2", "w"])');
      addCheck("max_big", 'MAX(@table["big1".."big2", "c1"])');

      addCheck("date_1900_leap", "DATE(1900, 2, 29)");
      addCheck(
        "date_dyn",
        'DATE(@table["yr_ok", "c1"], @table["mo_feb", "c1"], @table["day_29", "c1"])',
      );
      addCheck("pmt_dyn", 'PMT(0.05, 10, -1000, 0, @table["type_1", "c1"])');
      addCheck("yf_dyn", 'YEARFRAC(45292, 45658, @table["basis_3", "c1"])');
      addCheck("yr_min", "YEAR(1)");
      addCheck("yr_max", "YEAR(2958465.9)");
      addCheck(
        "yr_guard",
        'IF(@table["zero_date", "c1"] >= 1, YEAR(@table["zero_date", "c1"]), 2024)',
      );
      addCheck("edate_udf", "EDATE(DATE(1900, 1, 15), -2)");
      addCheck("eomonth_ovf", "EOMONTH(DATE(9999, 12, 1), 2)");
      addCheck("yr_edate_bubble", "YEAR(EDATE(DATE(1900, 1, 15), -2))");
      addCheck("round_float", "ROUND(10, 2.5)");
      addCheck("date_yr24", "DATE(24, 1, 1)");
      addCheck("idx_oob", 'INDEX(@table["r1".."r3", "c1"], 5)');
      addCheck("choose_zero", "CHOOSE(0, 10, 20)");
      addCheck("xmatch_bad_mode", 'XMATCH(10, @table["r1".."r3", "c1"], 9)');

      const compiled = okModel.compile();
      expect(compiled.num("T", "desc_xmatch", "c1")).toBe(3);
      expect(compiled.num("T", "desc_xlookup", "c1")).toBe(10);
      expect(compiled.value("T", "desc_index", "c1")).toBe("Cost");
      expect(compiled.num("T", "guard_sum", "c1")).toBe(0);

      expect(compiled.num("T", "sum_sparse", "c1")).toBe(20);
      expect(compiled.num("T", "avg_sparse", "c1")).toBe(10);
      expect(compiled.num("T", "cnt_sparse", "c1")).toBe(2);
      expect(compiled.num("T", "sp_sparse", "c1")).toBe(80);

      expect(compiled.num("T", "sum_all_null", "c1")).toBe(0);
      expect(compiled.value("T", "avg_all_null", "c1")).toBe("#DIV/0!");
      expect(compiled.value("T", "med_all_null", "c1")).toBe("#NUM!");
      expect(compiled.num("T", "min_all_null", "c1")).toBe(0);
      expect(compiled.num("T", "max_all_null", "c1")).toBe(0);
      expect(compiled.num("T", "cnt_all_null", "c1")).toBe(0);

      expect(compiled.num("T", "cnt_err_slice", "c1")).toBe(2);
      expect(compiled.value("T", "sum_err_slice", "c1")).toBe("#DIV/0!");

      expect(compiled.value("T", "sum_ovf", "c1")).toBe("#NUM!");
      expect(compiled.value("T", "sp_ovf", "c1")).toBe("#NUM!");
      expect(compiled.num("T", "max_big", "c1")).toBe(1e308);

      expect(compiled.num("T", "date_1900_leap", "c1")).toBe(60);
      expect(compiled.num("T", "date_dyn", "c1")).toBe(45351);
      expect(compiled.num("T", "pmt_dyn", "c1")).toBeGreaterThan(120);
      expect(compiled.num("T", "yf_dyn", "c1")).toBeCloseTo(366 / 365, 10);
      expect(compiled.num("T", "yr_min", "c1")).toBe(1900);
      expect(compiled.num("T", "yr_max", "c1")).toBe(9999);
      expect(compiled.num("T", "yr_guard", "c1")).toBe(2024);
      expect(compiled.formatted("T", "zero_date", "c1")).toBe("1900-01-00");
      expect(compiled.value("T", "edate_udf", "c1")).toBe("#NUM!");
      expect(compiled.value("T", "eomonth_ovf", "c1")).toBe("#NUM!");
      expect(compiled.value("T", "yr_edate_bubble", "c1")).toBe("#NUM!");
      expect(compiled.value("T", "round_float", "c1")).toBe("#NUM!");
      expect(compiled.value("T", "date_yr24", "c1")).toBe("#NUM!");
      expect(compiled.value("T", "idx_oob", "c1")).toBe("#REF!");
      expect(compiled.value("T", "choose_zero", "c1")).toBe("#VALUE!");
      expect(compiled.value("T", "xmatch_bad_mode", "c1")).toBe("#VALUE!");
    });

    it("(n) rejects dotted function names and evaluates ISERROR error introspection", () => {
      for (const badParse of ["ERROR.TYPE(1 / 0)", "ERROR . TYPE(1 / 0)", "error.type(1 / 0)"]) {
        const m = new Model({ name: "ParseCheck" });
        const t = m.addSheet("Main").addTable("T", "Parse");
        t.addColumn("c1", { role: "measure", header: "Val" });
        t.addRow("bad", { expr: badParse });
        expect(() => m.compile()).toThrow(FormulaParseError);
      }

      for (const badIserror of ['ISERROR("5" + 1)', 'ISERROR(@table["n", "c1"])']) {
        const mErr = new Model({ name: "IserrorTypeCheck" });
        const tErr = mErr.addSheet("Main").addTable("T", "Type Check");
        tErr.addColumn("c1", { role: "measure", header: "Val" });
        tErr.addRow("n", { cells: { c1: null } });
        tErr.addRow("target", { expr: badIserror });
        expect(() => mErr.compile()).toThrow(F8Error);
      }

      const okModel = new Model({ name: "ErrorIntrospectionOk" });
      const okTbl = okModel.addSheet("Main").addTable("T", "Error Introspection");
      for (const colId of ["cat", "res"]) {
        okTbl.addColumn(colId, { role: "measure", header: colId });
      }

      okTbl.addRow("r1", { cells: { cat: "Alpha", res: 0 } });
      okTbl.addRow("r2", { cells: { cat: "Beta", res: 0 } });
      okTbl.addRow("r3", { cells: { cat: "Alpha", res: 0 } });
      okTbl.addRow("r4", { cells: { cat: "May", res: 0 } });

      const addRes = (rowId: string, formula: string): void => {
        okTbl.addRow(rowId, {
          cells: {
            cat: null,
            res: new Cell({ expr: formula }),
          },
        });
      };

      addRes("iserror_div0", "ISERROR(1 / 0)");
      addRes("iserror_na", 'ISERROR(XMATCH("Missing", @table["r1".."r4", "cat"]))');
      addRes("iserror_non_err", "ISERROR(42)");

      const compiled = okModel.compile();
      expect(compiled.value("T", "iserror_div0", "res")).toBe(true);
      expect(compiled.value("T", "iserror_na", "res")).toBe(true);
      expect(compiled.value("T", "iserror_non_err", "res")).toBe(false);
      expect(compiled.inspectCell("T", "iserror_div0", "res").excelFormula).toContain("ISERROR(");
    });
  });
});
