import { mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { inflateRawSync } from "node:zlib";

import { afterEach, describe, expect, it } from "vitest";

import { main as cliMain } from "../src/cli.js";
import { Cell, Model } from "../src/index.js";

function buildInstitutionalLboModel(): Model {
  const model = new Model({ name: "AcmeCorpLBO", theme: "corporate_navy" });

  // Sheet 1: Operating Assumptions
  const assumptionsSheet = model.addSheet("Assumptions", {
    tabColor: "#1F4E79",
    showGridlines: true,
    freezePanes: { row: 3, col: 2 },
  });
  const assumptionsTbl = assumptionsSheet.addTable("assumptions", "Macro & Operating Assumptions", {
    subtitle: "Audited Forecast Drivers",
    currency: "USD",
  });
  assumptionsTbl.addColumn("driver", { role: "descriptor", header: "Operating Driver" });
  assumptionsTbl.addColumn("y2023a", {
    role: "measure",
    group: "Historical",
    header: "2023A",
    format: "percentage",
    decimals: 1,
  });
  assumptionsTbl.addColumn("y2024e", {
    role: "measure",
    group: "Forecast",
    header: "2024E",
    format: "percentage",
    decimals: 1,
  });
  assumptionsTbl.addColumn("y2025e", {
    role: "measure",
    group: "Forecast",
    header: "2025E",
    format: "percentage",
    decimals: 1,
  });
  assumptionsTbl.addColumn("y2026e", {
    role: "measure",
    group: "Forecast",
    header: "2026E",
    format: "percentage",
    decimals: 1,
  });

  assumptionsTbl.addSectionHeader("Top-Line Revenue Drivers");
  assumptionsTbl.addRow("rev_growth", {
    cells: {
      driver: "Revenue Growth Rate (%)",
      y2023a: null,
      y2024e: 0.15,
      y2025e: 0.12,
      y2026e: 0.1,
    },
    format: "percentage",
    decimals: 1,
  });

  assumptionsTbl.addRowGutter();
  assumptionsTbl.addSectionHeader("Margin & Rate Benchmarks");
  assumptionsTbl.addRow("cogs_margin", {
    cells: {
      driver: "COGS Margin (% of Revenue)",
      y2023a: 0.45,
      y2024e: 0.42,
      y2025e: 0.4,
      y2026e: 0.39,
    },
    format: "percentage",
    decimals: 1,
  });
  assumptionsTbl.addRow("sga_margin", {
    cells: {
      driver: "SG&A Margin (% of Revenue)",
      y2023a: 0.2,
      y2024e: 0.18,
      y2025e: 0.17,
      y2026e: 0.16,
    },
    format: "percentage",
    decimals: 1,
  });
  assumptionsTbl.addRow("tax_rate", {
    cells: {
      driver: "Effective Income Tax Rate",
      y2023a: 0.25,
      y2024e: 0.25,
      y2025e: 0.25,
      y2026e: 0.25,
    },
    format: "percentage",
    decimals: 1,
  });

  // Sheet 2: Income Statement
  const isSheet = model.addSheet("Income Statement", {
    tabColor: "#2E75B6",
    showGridlines: true,
    freezePanes: { row: 3, col: 2 },
  });
  const isTbl = isSheet.addTable("income_statement", "Consolidated Statement of Operations", {
    subtitle: "USD Thousands ($)",
    currency: "USD",
  });
  isTbl.addColumn("line_item", { role: "descriptor", header: "Financial Line Item" });
  isTbl.addColumn("y2023a", {
    role: "measure",
    group: "Historical",
    header: "2023A",
    format: "currency",
    decimals: 0,
  });
  isTbl.addColumn("y2024e", {
    role: "measure",
    group: "Forecast",
    header: "2024E",
    format: "currency",
    decimals: 0,
  });
  isTbl.addColumn("y2025e", {
    role: "measure",
    group: "Forecast",
    header: "2025E",
    format: "currency",
    decimals: 0,
  });
  isTbl.addColumn("y2026e", {
    role: "measure",
    group: "Forecast",
    header: "2026E",
    format: "currency",
    decimals: 0,
  });

  isTbl.addSectionHeader("Gross Revenue & Cost of Sales");
  isTbl.addRow("rev", {
    cells: { line_item: "Total Gross Revenue", y2023a: 125000.0 },
    expr: 'income_statement["rev", col.prev] * (1 + assumptions["rev_growth", @col])',
    format: "currency",
    decimals: 0,
  });
  isTbl.addRow("cogs", {
    cells: {
      line_item: new Cell({ value: "Cost of Goods Sold (COGS)", indent: 1 }),
      y2023a: 56250.0,
    },
    expr: 'income_statement["rev", @col] * assumptions["cogs_margin", @col]',
    format: "currency",
    decimals: 0,
  });
  isTbl.addRow("gp", {
    cells: { line_item: new Cell({ value: "Gross Operating Profit", style: "subtotal" }) },
    expr: 'income_statement["rev", @col] - income_statement["cogs", @col]',
    style: "subtotal",
    format: "currency",
    decimals: 0,
  });

  isTbl.addRowGutter();
  isTbl.addSectionHeader("Operating Expenses & Pre-Tax Income");
  isTbl.addRow("sga", {
    cells: {
      line_item: new Cell({ value: "Selling, General & Administrative", indent: 1 }),
      y2023a: 25000.0,
    },
    expr: 'income_statement["rev", @col] * assumptions["sga_margin", @col]',
    format: "currency",
    decimals: 0,
  });
  isTbl.addRow("ebitda", {
    cells: { line_item: new Cell({ value: "Operating EBITDA", style: "subtotal" }) },
    expr: 'income_statement["gp", @col] - income_statement["sga", @col]',
    style: "subtotal",
    format: "currency",
    decimals: 0,
  });
  isTbl.addRow("tax", {
    cells: { line_item: new Cell({ value: "Provision for Income Taxes", indent: 1 }) },
    expr: 'income_statement["ebitda", @col] * assumptions["tax_rate", @col]',
    format: "currency",
    decimals: 0,
  });
  isTbl.addRow("net_income", {
    cells: { line_item: new Cell({ value: "Net Consolidated Income", style: "total" }) },
    expr: 'income_statement["ebitda", @col] - income_statement["tax", @col]',
    style: "total",
    format: "currency",
    decimals: 0,
  });

  // Attach audit note to net income exit year milestone
  isTbl.setCell("net_income", "y2026e", {
    expr: 'income_statement["ebitda", @col] - income_statement["tax", @col]',
    note: "5-Year Strategic Exit Target Milestone",
  });

  return model;
}

function readZipEntries(bytes: Uint8Array): Map<string, Uint8Array> {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const decoder = new TextDecoder("utf-8");

  let eocdOffset = -1;
  for (let i = bytes.byteLength - 22; i >= Math.max(0, bytes.byteLength - 65557); i--) {
    if (view.getUint32(i, true) === 0x06054b50) {
      eocdOffset = i;
      break;
    }
  }
  if (eocdOffset === -1) {
    throw new Error("Invalid ZIP archive: End of Central Directory not found");
  }

  const totalEntries = view.getUint16(eocdOffset + 10, true);
  let cdOffset = view.getUint32(eocdOffset + 16, true);
  const entries = new Map<string, Uint8Array>();

  for (let i = 0; i < totalEntries; i++) {
    const sig = view.getUint32(cdOffset, true);
    if (sig !== 0x02014b50) {
      throw new Error(`Invalid Central Directory header signature at offset ${cdOffset}`);
    }
    const compressionMethod = view.getUint16(cdOffset + 10, true);
    const compressedSize = view.getUint32(cdOffset + 20, true);
    const fileNameLen = view.getUint16(cdOffset + 28, true);
    const extraLen = view.getUint16(cdOffset + 30, true);
    const commentLen = view.getUint16(cdOffset + 32, true);
    const localHeaderOffset = view.getUint32(cdOffset + 42, true);

    const fileName = decoder.decode(bytes.subarray(cdOffset + 46, cdOffset + 46 + fileNameLen));

    const localSig = view.getUint32(localHeaderOffset, true);
    if (localSig !== 0x04034b50) {
      throw new Error(`Invalid Local File Header signature at offset ${localHeaderOffset}`);
    }
    const localNameLen = view.getUint16(localHeaderOffset + 26, true);
    const localExtraLen = view.getUint16(localHeaderOffset + 28, true);
    const dataStart = localHeaderOffset + 30 + localNameLen + localExtraLen;
    const rawSlice = bytes.subarray(dataStart, dataStart + compressedSize);

    if (compressionMethod === 0) {
      entries.set(fileName, rawSlice);
    } else if (compressionMethod === 8) {
      entries.set(fileName, inflateRawSync(rawSlice));
    } else {
      throw new Error(`Unsupported ZIP compression method ${compressionMethod} for ${fileName}`);
    }

    cdOffset += 46 + fileNameLen + extraLen + commentLen;
  }

  return entries;
}

describe("TypeScript End-to-End Integration Suite", () => {
  const tempDirs: string[] = [];

  function makeTempDir(): string {
    const dir = mkdtempSync(join(tmpdir(), "f8-e2e-test-"));
    tempDirs.push(dir);
    return dir;
  }

  afterEach(() => {
    while (tempDirs.length > 0) {
      const dir = tempDirs.pop();
      if (dir !== undefined) {
        rmSync(dir, { recursive: true, force: true });
      }
    }
  });

  it("Scenario 1: E2E CLI agent scaffolding and guide listing", () => {
    let outBuf = "";
    const exitCode1 = cliMain(["guide", "--list"], {
      stdout: (text: string): void => {
        outBuf += text;
      },
    });
    expect(exitCode1).toBe(0);
    expect(outBuf).toContain("Available topics:");
    expect(outBuf).toContain("syntax");
    expect(outBuf).toContain("functions");
    expect(outBuf).toContain("audit");
    expect(outBuf).toContain("all");

    const tmpDir = makeTempDir();
    const destWorkspace = join(tmpDir, "agent_workspace");
    const exitCode2 = cliMain(["agent", "init", "--dest", destWorkspace], {
      stdout: (): void => {},
    });
    expect(exitCode2).toBe(0);

    const skillRoot = join(destWorkspace, ".agents", "skills", "f8-modeling");
    expect(statSync(join(skillRoot, "SKILL.md")).isFile()).toBe(true);
    expect(statSync(join(skillRoot, "references", "syntax-and-grid.md")).isFile()).toBe(true);
    expect(statSync(join(skillRoot, "references", "functions.md")).isFile()).toBe(true);
    expect(statSync(join(skillRoot, "references", "auditing-and-repair.md")).isFile()).toBe(true);
  });

  it("Scenario 2: E2E model compilation and table auditing", () => {
    const model = buildInstitutionalLboModel();
    const compiled = model.compile();

    const summary = compiled.summary();
    expect(summary.modelName).toBe("AcmeCorpLBO");
    expect(summary.totalSheets).toBe(2);
    expect(summary.totalTables).toBe(2);
    expect(summary.totalCells).toBe(55);
    expect(summary.totalDrivers).toBe(30);
    expect(summary.totalFormulas).toBe(25);

    const isAuditor = compiled.table("income_statement");
    expect(isAuditor.id).toBe("income_statement");
    expect(isAuditor.title).toBe("Consolidated Statement of Operations");
    expect(isAuditor.subtitle).toBe("USD Thousands ($)");
    expect(isAuditor.columns).toEqual(["line_item", "y2023a", "y2024e", "y2025e", "y2026e"]);
    expect(isAuditor.rows).toEqual(["rev", "cogs", "gp", "sga", "ebitda", "tax", "net_income"]);
    expect(isAuditor.shape).toEqual([7, 5]);
    expect(isAuditor.length).toBe(7);

    // 2023A (Inputs)
    expect(isAuditor.value("rev", "y2023a")).toBe(125000.0);
    expect(isAuditor.value("cogs", "y2023a")).toBe(56250.0);
    expect(isAuditor.value("gp", "y2023a")).toBe(68750.0);
    expect(isAuditor.value("sga", "y2023a")).toBe(25000.0);
    expect(isAuditor.value("ebitda", "y2023a")).toBe(43750.0);
    expect(isAuditor.value("tax", "y2023a")).toBe(10937.5);
    expect(isAuditor.value("net_income", "y2023a")).toBe(32812.5);

    // 2024E (Compound Growth + Margins from Assumptions table)
    expect(isAuditor.value("rev", "y2024e")).toBe(143750.0);
    expect(isAuditor.value("cogs", "y2024e")).toBe(60375.0);
    expect(isAuditor.value("gp", "y2024e")).toBe(83375.0);
    expect(isAuditor.value("sga", "y2024e")).toBe(25875.0);
    expect(isAuditor.value("ebitda", "y2024e")).toBe(57500.0);
    expect(isAuditor.value("tax", "y2024e")).toBe(14375.0);
    expect(isAuditor.value("net_income", "y2024e")).toBe(43125.0);

    // 2026E Exit Year
    expect(isAuditor.num("rev", "y2026e")).toBeCloseTo(177100.0);
    expect(isAuditor.num("net_income", "y2026e")).toBeCloseTo(59771.25);

    // Physical coordinates and lowered Excel formulas
    expect(isAuditor.address("rev", "y2024e")).toBe("D8");
    expect(isAuditor.formula("rev", "y2024e")).toBe("=C8*(1+'Assumptions'!D8)");
    expect(isAuditor.address("net_income", "y2026e")).toBe("F16");
    expect(isAuditor.formula("net_income", "y2026e")).toBe("=F14-F15");

    // Cell audit note
    const cellInfo = compiled.inspectCell("income_statement", "net_income", "y2026e");
    expect(cellInfo.note).toBe("5-Year Strategic Exit Target Milestone");

    // Fast-path compiled accessors
    expect(compiled.num("income_statement", "rev", "y2024e")).toBeCloseTo(143750.0);
    expect(compiled.formatted("income_statement", "rev", "y2024e", { unit: "K" })).toBe(
      "$143,750K",
    );
    expect(isAuditor.measureColumns).toEqual(["y2023a", "y2024e", "y2025e", "y2026e"]);
    const revRowNums = isAuditor.rowNums("rev", ["y2023a", "y2024e"]);
    expect(revRowNums).toHaveLength(2);
    expect(revRowNums[0]).toBeCloseTo(125000.0);
    expect(revRowNums[1]).toBeCloseTo(143750.0);

    // Markdown rendering across views
    const mdValues = isAuditor.toMarkdown("values");
    expect(mdValues).toContain("$125,000");
    expect(mdValues).toContain("$143,750");
    expect(mdValues).toContain("**Gross Revenue & Cost of Sales**");
    expect(mdValues).toContain("**Gross Operating Profit**");
    expect(mdValues).toContain("**Net Consolidated Income**");
    expect(mdValues).toContain("Historical: 2023A");

    const mdFormulas = isAuditor.toMarkdown("formulas");
    expect(mdFormulas).toContain("=C8*(1+'Assumptions'!D8)");
    expect(mdFormulas).toContain("=F14-F15");

    const mdAddresses = isAuditor.toMarkdown("addresses");
    expect(mdAddresses).toContain("C8");
    expect(mdAddresses).toContain("F16");

    // Structured record export
    const records = isAuditor.toDict("records");
    expect(records).toHaveLength(7);
    expect(records[0]?.line_item).toBe("Total Gross Revenue");
    expect(records[0]?.y2023a).toBe(125000.0);
  });

  it("Scenario 3: E2E OpenXML workbook emission and inspection", () => {
    const model = buildInstitutionalLboModel();
    const compiled = model.compile();

    const tmpDir = makeTempDir();
    const xlsxPath = join(tmpDir, "acme_lbo_model.xlsx");
    compiled.save(xlsxPath);

    expect(statSync(xlsxPath).isFile()).toBe(true);
    expect(statSync(xlsxPath).size).toBeGreaterThan(0);

    const bytes = new Uint8Array(readFileSync(xlsxPath));
    const entries = readZipEntries(bytes);

    expect(entries.has("[Content_Types].xml")).toBe(true);
    expect(entries.has("xl/workbook.xml")).toBe(true);
    expect(entries.has("xl/styles.xml")).toBe(true);
    expect(entries.has("xl/worksheets/sheet1.xml")).toBe(true);
    expect(entries.has("xl/worksheets/sheet2.xml")).toBe(true);

    const sheet2Bytes = entries.get("xl/worksheets/sheet2.xml");
    expect(sheet2Bytes).toBeDefined();
    const sheet2Xml = new TextDecoder("utf-8").decode(sheet2Bytes);
    expect(sheet2Xml).toContain("<f>");
    expect(sheet2Xml).toContain("<v>");
    expect(sheet2Xml).toContain("Assumptions");
  });
});
