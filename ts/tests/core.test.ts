import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  compile,
  CycleDetectedError,
  defaultTheme,
  EmissionError,
  F8Error,
  FormulaParseError,
  InspectionError,
  ReferenceResolutionError,
  SpecificationError,
  VERSION,
} from "../src/index.js";
import type { ProjectManifestInput, TableSpecInput } from "../src/types.js";

describe("WebAssembly Core Bindings and TypeScript SDK", () => {
  it("exposes package version and default corporate_navy theme", () => {
    const pkg = JSON.parse(
      readFileSync(new URL("../../package.json", import.meta.url), "utf-8"),
    ) as { version: string };
    expect(VERSION).toBe(pkg.version);
    const theme = defaultTheme();
    expect(theme).toBeTypeOf("object");
    expect(theme["name"]).toBe("corporate_navy");
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
      expect(cell.physicalAddress).toBe("BalanceSheet!D8");
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
        expect(() => model.save(tempDir)).toThrow(EmissionError);
        expect(() => model.save(tempDir)).toThrow(F8Error);
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
  });
});
