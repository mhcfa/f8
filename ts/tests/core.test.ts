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
        cells: { line_item: new Cell({ expr: 'COUNTA(@table["r1".."r2", @col])' }) },
        expr: 'SUM(@table["r1".."r2", "2024E".."2025E"]) + XLOOKUP("Rev", @table["r1".."r2", "line_item"], @table["r1".."r2", @col])',
      });
      const compiled = okModel.compile();
      expect(compiled.num("T", "tot", "2024E")).toBe(130);
      expect(compiled.num("T", "tot", "2025E")).toBe(140);
      expect(compiled.num("T", "tot", "line_item")).toBe(2);

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
        ["IF(TRUE, 10, IFS(TRUE, 1, FALSE))", "variadic pairs in 'IFS'"],
        ["IF(TRUE, 10, ABS(_))", "argument 'number' at position 1 of 'ABS' cannot be omitted"],
        ["IFS(TRUE, 1, FALSE, UNKNOWN_FN())", "unknown function 'UNKNOWN_FN'"],
        ["IFERROR(10, UNKNOWN_FN())", "unknown function 'UNKNOWN_FN'"],
        ["IFERROR(ABS(1, 2, 3), 0)", "too many arguments for 'ABS'"],
        ["IFNA(10, UNKNOWN_FN())", "unknown function 'UNKNOWN_FN'"],
        ["CHOOSE(1, 10, SUM())", "too few arguments for 'SUM'"],
        ["SWITCH(1, 1, 10, ABS())", "too few arguments for 'ABS'"],
        [
          'XLOOKUP(1, @table["k1".."k2", @col], @table["v1".."v2", @col], UNKNOWN_FN())',
          "unknown function 'UNKNOWN_FN'",
        ],
        ["IF(1 / 0, UNKNOWN_FN(), 10)", "unknown function 'UNKNOWN_FN'"],
        ["IF(TRUE, 10, -(1 + UNKNOWN_FN()))", "unknown function 'UNKNOWN_FN'"],
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

      // Valid IFERROR domain error trapping and valid CallArg::Omitted lowering
      const okModel = new Model({ name: "ValidCallArgModel" });
      const okSheet = okModel.addSheet("Main");
      const okTbl = okSheet.addTable("T", "Table");
      okTbl.addColumn("c1", { role: "measure", header: "C1" });
      okTbl.addRow("k1", { cells: { c1: 100 } });
      okTbl.addRow("k2", { cells: { c1: 200 } });
      okTbl.addRow("v1", { cells: { c1: 10 } });
      okTbl.addRow("v2", { cells: { c1: 20 } });
      okTbl.addRow("iferror_ok", { expr: "IFERROR(10 / 0, 42)" });
      okTbl.addRow("if_omitted", { expr: "IF(TRUE, 10, _)" });
      okTbl.addRow("xlookup_omitted", {
        expr: 'XLOOKUP(100, @table["k1".."k2", @col], @table["v1".."v2", @col], _, 0, 1)',
      });

      const compiled = okModel.compile();
      expect(compiled.num("T", "iferror_ok", "c1")).toBe(42);
      expect(compiled.num("T", "if_omitted", "c1")).toBe(10);
      expect(compiled.num("T", "xlookup_omitted", "c1")).toBe(10);

      const ifCell = compiled.inspectCell("T", "if_omitted", "c1");
      expect(ifCell.excelFormula).toBe("=IF(TRUE,10,)");

      const xlookupCell = compiled.inspectCell("T", "xlookup_omitted", "c1");
      expect(xlookupCell.excelFormula).not.toBeNull();
      expect(xlookupCell.excelFormula?.startsWith("=XLOOKUP(100,")).toBe(true);
      expect(xlookupCell.excelFormula?.endsWith(",,0,1)")).toBe(true);

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
  });
});
