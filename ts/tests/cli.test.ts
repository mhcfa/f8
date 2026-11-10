import { execFileSync, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { afterEach, describe, expect, it } from "vitest";

import { getPackageRoot, getSkillResourceDir, GUIDE_TOPICS, main } from "../src/cli.js";
import { defaultTheme, ensureInitSync, Model, VERSION } from "../src/index.js";

function createCaptureIo(cwd?: string): {
  readonly out: () => string;
  readonly err: () => string;
  readonly io: {
    readonly stdout: (text: string) => void;
    readonly stderr: (text: string) => void;
    readonly cwd?: string;
  };
} {
  let outBuf = "";
  let errBuf = "";
  return {
    out: () => outBuf,
    err: () => errBuf,
    io: {
      stdout: (text: string): void => {
        outBuf += text;
      },
      stderr: (text: string): void => {
        errBuf += text;
      },
      ...(cwd !== undefined ? { cwd } : {}),
    },
  };
}

function collectFilesRecursive(dir: string): string[] {
  const results: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) {
      results.push(...collectFilesRecursive(full));
    } else {
      results.push(full);
    }
  }
  return results;
}

describe("TypeScript CLI and module runner entrypoints", () => {
  const tempDirs: string[] = [];

  function makeTempDir(): string {
    const dir = mkdtempSync(join(tmpdir(), "f8-cli-test-"));
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

  it("prints usage when invoked with no arguments", () => {
    const cap = createCaptureIo();
    const exitCode = main([], cap.io);
    expect(exitCode).toBe(0);
    expect(cap.out()).toContain("usage: f8");
    expect(cap.out()).toContain("guide");
    expect(cap.out()).toContain("agent");
    expect(cap.out()).toContain("theme");
    expect(cap.err()).toBe("");
  });

  it("prints help when agent subcommand is invoked without args", () => {
    const cap = createCaptureIo();
    const exitCode = main(["agent"], cap.io);
    expect(exitCode).toBe(0);
    expect(cap.out()).toContain("usage: f8 agent");
    expect(cap.out()).toContain("init");
  });

  it("prints help when theme subcommand is invoked without args", () => {
    const cap = createCaptureIo();
    const exitCode = main(["theme"], cap.io);
    expect(exitCode).toBe(0);
    expect(cap.out()).toContain("usage: f8 theme");
    expect(cap.out()).toContain("init");
  });

  it("Scenario 1: outputs modeling guide overview (SKILL.md) to stdout", () => {
    const cap = createCaptureIo();
    const exitCode = main(["guide"], cap.io);
    expect(exitCode).toBe(0);
    expect(cap.out()).toContain("name: f8-modeling");
    expect(cap.out()).toContain("# f8 Financial Modeling Guide");
    expect(cap.out()).toContain("Mental Model");
    expect(cap.err()).toBe("");
  });

  it.each(["syntax", "functions", "audit"])(
    "Scenario 2: outputs specific topic guide '%s' to stdout",
    (topic) => {
      const cap = createCaptureIo();
      const exitCode = main(["guide", topic], cap.io);
      expect(exitCode).toBe(0);
      const entry = GUIDE_TOPICS[topic];
      const relPath = entry ? entry[0] : "";
      const targetPath = join(getSkillResourceDir(), ...relPath.split("/"));
      const expectedContent = readFileSync(targetPath, "utf-8");
      expect(cap.out()).toBe(expectedContent);
      expect(cap.err()).toBe("");
    },
  );

  it("Scenario 3: lists available guide topics with aligned summaries", () => {
    const cap = createCaptureIo();
    const exitCode = main(["guide", "--list"], cap.io);
    expect(exitCode).toBe(0);
    const expectedLines = [
      "Available topics:",
      "  syntax     2D coordinates, vector expressions, column roles, gutters, indentation",
      "  functions  44 built-in Excel functions, signatures, omission syntax, errors",
      "  audit      TableAuditor, inspection, unboxing, structured exception recovery",
      "  all        Complete concatenated guide",
    ];
    expect(cap.out().trim()).toBe(expectedLines.join("\n"));
    expect(cap.err()).toBe("");
  });

  it.each([[["guide", "--all"]], [["guide", "all"]]])(
    "Scenario 4: outputs complete concatenated guide separated by delimiters (%j)",
    (args) => {
      const cap = createCaptureIo();
      const exitCode = main(args, cap.io);
      expect(exitCode).toBe(0);

      const root = getSkillResourceDir();
      const sections = [
        readFileSync(join(root, "SKILL.md"), "utf-8").trim(),
        readFileSync(join(root, "references", "syntax-and-grid.md"), "utf-8").trim(),
        readFileSync(join(root, "references", "functions.md"), "utf-8").trim(),
        readFileSync(join(root, "references", "auditing-and-repair.md"), "utf-8").trim(),
      ];
      const expected = `${sections.join("\n\n---\n\n")}\n`;
      expect(cap.out()).toBe(expected);
      expect(cap.err()).toBe("");
    },
  );

  it("Scenario 5: rejects invalid guide topic and exits with code 2", () => {
    const cap = createCaptureIo();
    const exitCode = main(["guide", "unknown_topic"], cap.io);
    expect(exitCode).toBe(2);
    expect(cap.err()).toContain("invalid choice: 'unknown_topic'");
  });

  it("Scenario 6: initializes agent skill directory tree in empty workspace", () => {
    const tmpDir = makeTempDir();
    const workspace = join(tmpDir, "my_project");
    const cap = createCaptureIo();
    const exitCode = main(["agent", "init", "--dest", workspace], cap.io);
    expect(exitCode).toBe(0);
    expect(cap.out()).toContain("Initialized f8-modeling skill in");

    const skillDir = join(workspace, ".agents", "skills", "f8-modeling");
    expect(statSync(skillDir).isDirectory()).toBe(true);
    expect(statSync(join(skillDir, "SKILL.md")).isFile()).toBe(true);
    expect(statSync(join(skillDir, "references", "syntax-and-grid.md")).isFile()).toBe(true);
    expect(statSync(join(skillDir, "references", "functions.md")).isFile()).toBe(true);
    expect(statSync(join(skillDir, "references", "auditing-and-repair.md")).isFile()).toBe(true);

    const allFiles = collectFilesRecursive(skillDir);
    const codeFiles = allFiles.filter(
      (f) => f.endsWith(".py") || f.endsWith(".ts") || f.endsWith(".js"),
    );
    expect(codeFiles).toHaveLength(0);
  });

  it("Scenario 7: rejects overwrite on collision without force flag", () => {
    const tmpDir = makeTempDir();
    const workspace = join(tmpDir, "colliding_project");
    const cap1 = createCaptureIo();
    expect(main(["agent", "init", "--dest", workspace], cap1.io)).toBe(0);

    const skillFile = join(workspace, ".agents", "skills", "f8-modeling", "SKILL.md");
    const originalMtime = statSync(skillFile).mtimeMs;

    const cap2 = createCaptureIo();
    const collisionCode = main(["agent", "init", "--dest", workspace], cap2.io);
    expect(collisionCode).toBe(1);
    expect(cap2.err()).toContain("already exists and contains files");
    expect(cap2.err()).toContain("Use --force to overwrite");
    expect(statSync(skillFile).mtimeMs).toBe(originalMtime);
  });

  it.each(["--force", "-f"])(
    "Scenario 8: overwrites existing skill directory with %s flag",
    (flag) => {
      const tmpDir = makeTempDir();
      const workspace = join(tmpDir, "force_project");
      const cap1 = createCaptureIo();
      expect(main(["agent", "init", "--dest", workspace], cap1.io)).toBe(0);

      const skillFile = join(workspace, ".agents", "skills", "f8-modeling", "SKILL.md");
      writeFileSync(skillFile, "Modified content", "utf-8");
      expect(readFileSync(skillFile, "utf-8")).toBe("Modified content");

      const cap2 = createCaptureIo();
      const forceCode = main(["agent", "init", "--dest", workspace, flag], cap2.io);
      expect(forceCode).toBe(0);
      expect(cap2.out()).toContain("Initialized f8-modeling skill in");

      const expectedSkill = readFileSync(join(getSkillResourceDir(), "SKILL.md"), "utf-8");
      expect(readFileSync(skillFile, "utf-8")).toBe(expectedSkill);
    },
  );

  it("Scenario 9: prevents self-copy into source asset tree", () => {
    const pkgDir = getPackageRoot();
    const cap1 = createCaptureIo();
    const exitCode1 = main(["agent", "init", "--dest", pkgDir], cap1.io);
    expect(exitCode1).toBe(1);
    expect(cap1.err()).toContain("Destination path resolves to package resource directory");

    const skillSrcDir = getSkillResourceDir();
    const cap2 = createCaptureIo();
    const exitCode2 = main(["agent", "init", "--dest", skillSrcDir], cap2.io);
    expect(exitCode2).toBe(1);
    expect(cap2.err()).toContain("Destination path resolves to package resource directory");
  });

  it("executes built dist/src/cli.js via node subprocess", () => {
    const repoRoot = resolve(fileURLToPath(new URL("../..", import.meta.url)));
    const distCli = join(repoRoot, "dist", "src", "cli.js");
    const distSkill = join(repoRoot, "dist", "skills", "f8-modeling", "SKILL.md");
    if (!existsSync(distCli) || !existsSync(distSkill)) {
      execFileSync("pnpm", ["build:ts"], { cwd: repoRoot, stdio: "pipe" });
    }

    const result = spawnSync(process.execPath, [distCli, "guide", "--list"], {
      cwd: repoRoot,
      encoding: "utf-8",
    });
    expect(result.status).toBe(0);
    expect(result.stdout).toContain("Available topics:");
    expect(result.stdout).toContain("syntax");
    expect(result.stdout).toContain("functions");
    expect(result.stdout).toContain("audit");
    expect(result.stdout).toContain("all");
  });

  it.each(["--version", "-V"])("outputs package version for %s and exits with 0", (flag) => {
    ensureInitSync();
    const cap = createCaptureIo();
    const exitCode = main([flag], cap.io);
    expect(exitCode).toBe(0);
    expect(cap.out().trim()).toBe(`f8 ${VERSION}`);
    expect(VERSION.split(".").length).toBeGreaterThanOrEqual(3);
  });

  it("generates theme.json via f8 theme init and compiles model", () => {
    const tmpDir = makeTempDir();
    const cap = createCaptureIo(tmpDir);
    const exitCode = main(["theme", "init"], cap.io);
    expect(exitCode).toBe(0);
    expect(cap.out()).toContain("Initialized theme specification in theme.json");
    expect(cap.err()).toBe("");

    const themeFile = join(tmpDir, "theme.json");
    expect(statSync(themeFile).isFile()).toBe(true);

    const parsed: unknown = JSON.parse(readFileSync(themeFile, "utf-8"));
    expect(parsed).toEqual(defaultTheme());

    const model = new Model({ name: "ThemeInitModel", theme: pathToFileURL(themeFile) });
    const sheet = model.addSheet("Summary");
    const table = sheet.addTable("T1", "Test");
    table.addColumn("c1", { header: "Col 1", format: "currency" });
    table.addRow("r1", { cells: { c1: 42.0 } });
    const compiled = model.compile();
    expect(compiled.summary().themeName).toBe("corporate_navy");
  });

  it("refuses to overwrite existing theme.json without --force", () => {
    const tmpDir = makeTempDir();
    const themeFile = join(tmpDir, "theme.json");
    writeFileSync(themeFile, '{"custom": true}\n', "utf-8");

    const cap = createCaptureIo(tmpDir);
    const exitCode = main(["theme", "init"], cap.io);
    expect(exitCode).toBe(1);
    expect(cap.err()).toContain("already exists");
    expect(cap.err()).toContain("Use --force to overwrite");
    expect(readFileSync(themeFile, "utf-8")).toBe('{"custom": true}\n');
  });

  it.each(["--force", "-f"])("overwrites existing theme.json when %s is passed", (flag) => {
    const tmpDir = makeTempDir();
    const themeFile = join(tmpDir, "theme.json");
    writeFileSync(themeFile, '{"custom": true}\n', "utf-8");

    const cap = createCaptureIo(tmpDir);
    const exitCode = main(["theme", "init", flag], cap.io);
    expect(exitCode).toBe(0);
    expect(cap.out()).toContain("Initialized theme specification in theme.json");
    const parsed: unknown = JSON.parse(readFileSync(themeFile, "utf-8"));
    expect(parsed).toEqual(defaultTheme());
  });
});
