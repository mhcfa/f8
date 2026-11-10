#!/usr/bin/env node

import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  realpathSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { isAbsolute, join, relative, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { defaultTheme, ensureInitSync, VERSION } from "./index.js";

export interface CliIo {
  readonly stdout?: (text: string) => void;
  readonly stderr?: (text: string) => void;
  readonly cwd?: string;
}

export const GUIDE_TOPICS: Readonly<Record<string, readonly [string, string]>> = {
  syntax: [
    "references/syntax-and-grid.md",
    "2D coordinates, vector expressions, column roles, gutters, indentation",
  ],
  functions: [
    "references/functions.md",
    "44 built-in Excel functions, signatures, omission syntax, errors",
  ],
  audit: [
    "references/auditing-and-repair.md",
    "TableAuditor, inspection, unboxing, structured exception recovery",
  ],
  all: ["", "Complete concatenated guide"],
};

const TOP_HELP = `usage: f8 [-h] [-V] {guide,agent,theme} ...

F8 Financial Modeling SDK runner and agent onboarding helper.

positional arguments:
  {guide,agent,theme}
    guide              Stream financial modeling guide and reference documentation.
    agent              Agent skill management commands.
    theme              Workbook theme configuration commands.

options:
  -h, --help           show this help message and exit
  -V, --version        show program's version number and exit
`;

const GUIDE_HELP = `usage: f8 guide [-h] [--all] [--list] [{syntax,functions,audit,all}]

Stream financial modeling guide and reference documentation to stdout.

positional arguments:
  {syntax,functions,audit,all}
                        Specific topic guide to display.

options:
  -h, --help            show this help message and exit
  --all                 Output complete concatenated documentation.
  --list                List available topics with summaries.
`;

const AGENT_HELP = `usage: f8 agent [-h] {init} ...

Agent skill management commands.

positional arguments:
  {init}
    init      Initialize agent modeling skill in target workspace.

options:
  -h, --help  show this help message and exit
`;

const AGENT_INIT_HELP = `usage: f8 agent init [-h] [--dest DEST] [-f]

Scaffold the .agents/skills/f8-modeling directory tree into the target workspace.

options:
  -h, --help   show this help message and exit
  --dest DEST  Target root directory (default: current working directory).
  -f, --force  Overwrite existing skill files if present.
`;

const THEME_HELP = `usage: f8 theme [-h] {init} ...

Workbook theme configuration commands.

positional arguments:
  {init}
    init      Generate default theme.json in current directory.

options:
  -h, --help  show this help message and exit
`;

const THEME_INIT_HELP = `usage: f8 theme init [-h] [-f]

Generate a corporate_navy theme.json file in the current directory.

options:
  -h, --help   show this help message and exit
  -f, --force  Overwrite existing theme.json if present.
`;

export function getPackageRoot(): string {
  return fileURLToPath(new URL("..", import.meta.url));
}

export function getSkillResourceDir(): string {
  return fileURLToPath(new URL("../skills/f8-modeling", import.meta.url));
}

function formatTopicList(): string {
  const lines = ["Available topics:"];
  for (const [topic, [, desc]] of Object.entries(GUIDE_TOPICS)) {
    lines.push(`  ${topic.padEnd(11, " ")}${desc}`);
  }
  return `${lines.join("\n")}\n`;
}

function copyDirectoryRecursive(src: string, dst: string): void {
  const stat = statSync(src);
  if (stat.isDirectory()) {
    mkdirSync(dst, { recursive: true });
    for (const entry of readdirSync(src)) {
      copyDirectoryRecursive(join(src, entry), join(dst, entry));
    }
  } else if (stat.isFile()) {
    copyFileSync(src, dst);
  }
}

function resolveCanonical(p: string): string {
  const abs = resolve(p);
  if (existsSync(abs)) {
    try {
      return realpathSync(abs);
    } catch {
      return abs;
    }
  }
  return abs;
}

function isEqualOrSubpath(child: string, parent: string): boolean {
  const rel = relative(parent, child);
  return rel === "" || (!rel.startsWith("..") && !isAbsolute(rel));
}

function isSelfCopy(dest: string, targetDir: string, skillDir: string, pkgRoot: string): boolean {
  const resolvedDest = resolveCanonical(dest);
  const resolvedTarget = resolveCanonical(targetDir);
  const resolvedSkill = resolveCanonical(skillDir);
  const resolvedPkg = resolveCanonical(pkgRoot);

  return (
    isEqualOrSubpath(resolvedDest, resolvedSkill) ||
    isEqualOrSubpath(resolvedTarget, resolvedSkill) ||
    isEqualOrSubpath(resolvedDest, resolvedPkg) ||
    isEqualOrSubpath(resolvedTarget, resolvedPkg)
  );
}

function hasExistingFiles(targetDir: string): boolean {
  if (!existsSync(targetDir)) {
    return false;
  }
  try {
    const stat = statSync(targetDir);
    if (stat.isFile()) {
      return true;
    }
    if (stat.isDirectory()) {
      return readdirSync(targetDir).length > 0;
    }
    return true;
  } catch {
    return true;
  }
}

function runGuide(
  args: readonly string[],
  writeOut: (text: string) => void,
  writeErr: (text: string) => void,
): number {
  let showAll = false;
  let showList = false;
  let topic: string | undefined;

  for (const arg of args) {
    if (arg === "-h" || arg === "--help") {
      writeOut(GUIDE_HELP);
      return 0;
    }
    if (arg === "--all") {
      showAll = true;
    } else if (arg === "--list") {
      showList = true;
    } else if (arg.startsWith("-")) {
      writeErr(`f8 guide: error: unrecognized arguments: ${arg}\n`);
      return 2;
    } else if (topic === undefined) {
      if (!Object.prototype.hasOwnProperty.call(GUIDE_TOPICS, arg)) {
        const valid = Object.keys(GUIDE_TOPICS)
          .map((k) => `'${k}'`)
          .join(", ");
        writeErr(
          `f8 guide: error: argument topic: invalid choice: '${arg}' (choose from ${valid})\n`,
        );
        return 2;
      }
      topic = arg;
    } else {
      writeErr(`f8 guide: error: unrecognized arguments: ${arg}\n`);
      return 2;
    }
  }

  if (showList) {
    writeOut(formatTopicList());
    return 0;
  }

  const resourceRoot = getSkillResourceDir();
  if (!existsSync(resourceRoot) || !statSync(resourceRoot).isDirectory()) {
    writeErr("Error: Packaged f8-modeling skill resources not found.\n");
    return 1;
  }

  if (showAll || topic === "all") {
    const skillMdPath = join(resourceRoot, "SKILL.md");
    const sections = [readFileSync(skillMdPath, "utf-8").trim()];
    for (const [relPath] of Object.values(GUIDE_TOPICS)) {
      if (relPath) {
        const targetPath = join(resourceRoot, ...relPath.split("/"));
        sections.push(readFileSync(targetPath, "utf-8").trim());
      }
    }
    writeOut(`${sections.join("\n\n---\n\n")}\n`);
    return 0;
  }

  let targetPath: string;
  if (topic === undefined) {
    targetPath = join(resourceRoot, "SKILL.md");
  } else {
    const entry = GUIDE_TOPICS[topic];
    const relPath = entry ? entry[0] : "";
    targetPath = join(resourceRoot, ...relPath.split("/"));
  }

  if (!existsSync(targetPath) || !statSync(targetPath).isFile()) {
    writeErr(`Error: Guide resource '${targetPath}' not found.\n`);
    return 1;
  }

  const content = readFileSync(targetPath, "utf-8");
  writeOut(content.endsWith("\n") ? content : `${content}\n`);
  return 0;
}

function runAgentInit(
  args: readonly string[],
  cwd: string | undefined,
  writeOut: (text: string) => void,
  writeErr: (text: string) => void,
): number {
  let destArg = ".";
  let force = false;

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === undefined) {
      continue;
    }
    if (arg === "-h" || arg === "--help") {
      writeOut(AGENT_INIT_HELP);
      return 0;
    }
    if (arg === "-f" || arg === "--force") {
      force = true;
    } else if (arg === "--dest") {
      const next = args[i + 1];
      if (next === undefined) {
        writeErr("f8 agent init: error: argument --dest: expected one argument\n");
        return 2;
      }
      destArg = next;
      i += 1;
    } else if (arg.startsWith("--dest=")) {
      destArg = arg.slice("--dest=".length);
    } else {
      writeErr(`f8 agent init: error: unrecognized arguments: ${arg}\n`);
      return 2;
    }
  }

  const dest = cwd !== undefined ? resolve(cwd, destArg) : destArg;
  const targetDir = join(dest, ".agents", "skills", "f8-modeling");
  const skillResource = getSkillResourceDir();
  const pkgRoot = getPackageRoot();

  if (!existsSync(skillResource) || !statSync(skillResource).isDirectory()) {
    writeErr("Error: Packaged f8-modeling skill resources not found.\n");
    return 1;
  }

  if (isSelfCopy(dest, targetDir, skillResource, pkgRoot)) {
    writeErr("Error: Destination path resolves to package resource directory.\n");
    return 1;
  }

  if (hasExistingFiles(targetDir) && !force) {
    writeErr(
      `Error: Target directory '${targetDir}' already exists and contains files. Use --force to overwrite.\n`,
    );
    return 1;
  }

  if (existsSync(targetDir) && statSync(targetDir).isFile()) {
    unlinkSync(targetDir);
  }

  mkdirSync(targetDir, { recursive: true });
  copyDirectoryRecursive(skillResource, targetDir);
  writeOut(`Initialized f8-modeling skill in ${targetDir}\n`);
  return 0;
}

function runThemeInit(
  args: readonly string[],
  cwd: string | undefined,
  writeOut: (text: string) => void,
  writeErr: (text: string) => void,
): number {
  let force = false;

  for (const arg of args) {
    if (arg === "-h" || arg === "--help") {
      writeOut(THEME_INIT_HELP);
      return 0;
    }
    if (arg === "-f" || arg === "--force") {
      force = true;
    } else {
      writeErr(`f8 theme init: error: unrecognized arguments: ${arg}\n`);
      return 2;
    }
  }

  const baseCwd = cwd ?? process.cwd();
  const targetFile = join(baseCwd, "theme.json");

  if (existsSync(targetFile) && statSync(targetFile).isDirectory()) {
    writeErr("Error: Target path 'theme.json' is a directory.\n");
    return 1;
  }

  if (existsSync(targetFile) && !force) {
    writeErr("Error: Target file 'theme.json' already exists. Use --force to overwrite.\n");
    return 1;
  }

  const themeData = defaultTheme();
  writeFileSync(targetFile, `${JSON.stringify(themeData, null, 2)}\n`, "utf-8");
  writeOut("Initialized theme specification in theme.json\n");
  return 0;
}

export function main(argv?: readonly string[], io?: CliIo): number {
  const args = argv ?? process.argv.slice(2);
  const writeOut = io?.stdout ?? ((text: string): void => void process.stdout.write(text));
  const writeErr = io?.stderr ?? ((text: string): void => void process.stderr.write(text));

  if (args.length === 0) {
    writeOut(TOP_HELP);
    return 0;
  }

  const first = args[0];
  if (first === "-h" || first === "--help") {
    writeOut(TOP_HELP);
    return 0;
  }

  if (first === "-V" || first === "--version") {
    ensureInitSync();
    writeOut(`f8 ${VERSION}\n`);
    return 0;
  }

  if (first === "guide") {
    return runGuide(args.slice(1), writeOut, writeErr);
  }

  if (first === "agent") {
    const sub = args[1];
    if (sub === undefined || sub === "-h" || sub === "--help") {
      writeOut(AGENT_HELP);
      return 0;
    }
    if (sub === "init") {
      return runAgentInit(args.slice(2), io?.cwd, writeOut, writeErr);
    }
    writeErr(
      `f8 agent: error: argument agent_command: invalid choice: '${sub}' (choose from 'init')\n`,
    );
    return 2;
  }

  if (first === "theme") {
    const sub = args[1];
    if (sub === undefined || sub === "-h" || sub === "--help") {
      writeOut(THEME_HELP);
      return 0;
    }
    if (sub === "init") {
      return runThemeInit(args.slice(2), io?.cwd, writeOut, writeErr);
    }
    writeErr(
      `f8 theme: error: argument theme_command: invalid choice: '${sub}' (choose from 'init')\n`,
    );
    return 2;
  }

  writeErr(
    `f8: error: argument command: invalid choice: '${first ?? ""}' (choose from 'guide', 'agent', 'theme')\n`,
  );
  return 2;
}

function isMainModule(): boolean {
  const entry = process.argv[1];
  if (!entry) {
    return false;
  }
  try {
    const currentFile = realpathSync(fileURLToPath(import.meta.url));
    const entryFile = realpathSync(resolve(entry));
    return currentFile === entryFile;
  } catch {
    return false;
  }
}

if (isMainModule()) {
  process.exitCode = main();
}
