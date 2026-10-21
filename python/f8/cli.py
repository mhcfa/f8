"""Command-line interface and module runner for the f8 Python SDK.

Provides entrypoints for streaming financial modeling documentation and
scaffolding agent skill directories into target workspaces.
"""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from importlib.resources import files
from importlib.resources.abc import Traversable
from pathlib import Path

from f8._core import __version__

GUIDE_TOPICS: dict[str, tuple[str, str]] = {
    "syntax": (
        "references/syntax-and-grid.md",
        "2D coordinates, vector expressions, column roles, gutters, indentation",
    ),
    "functions": (
        "references/functions.md",
        "44 built-in Excel functions, signatures, omission syntax, errors",
    ),
    "audit": (
        "references/auditing-and-repair.md",
        "TableAuditor, inspection, unboxing, structured exception recovery",
    ),
    "all": (
        "",
        "Complete concatenated guide",
    ),
}


def _get_skill_resource() -> Traversable:
    """Resolve the packaged f8-modeling skill resource root."""
    return files("f8").joinpath("skills", "f8-modeling")


def _format_topic_list() -> str:
    """Format available guide topics into an aligned summary."""
    lines = ["Available topics:"]
    for topic, (_, desc) in GUIDE_TOPICS.items():
        lines.append(f"  {topic:<11}{desc}")
    return "\n".join(lines) + "\n"


def _copy_traversable(src: Traversable, dst: Path) -> None:
    """Recursively copy a Traversable directory hierarchy to a destination path."""
    if src.is_dir():
        dst.mkdir(parents=True, exist_ok=True)
        for child in src.iterdir():
            _copy_traversable(child, dst / child.name)
    elif src.is_file():
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_bytes(src.read_bytes())


def _is_self_copy(dest: Path, target_dir: Path, src: Traversable) -> bool:
    """Check if destination or target directory resolves into the package source tree."""
    try:
        resolved_dest = dest.resolve()
        resolved_target = target_dir.resolve()

        if isinstance(src, Path):
            resolved_src = src.resolve()
        elif hasattr(src, "resolve"):
            resolved_src = Path(str(src)).resolve()
        else:
            resolved_src = None

        if resolved_src is not None and (
            resolved_dest == resolved_src or resolved_target == resolved_src
        ):
            return True

        pkg_root = files("f8")
        if isinstance(pkg_root, Path):
            resolved_pkg_root = pkg_root.resolve()
        elif hasattr(pkg_root, "resolve"):
            resolved_pkg_root = Path(str(pkg_root)).resolve()
        else:
            resolved_pkg_root = None

        if resolved_pkg_root is not None and (
            resolved_dest == resolved_pkg_root
            or resolved_dest.is_relative_to(resolved_pkg_root)
            or resolved_target == resolved_pkg_root
            or resolved_target.is_relative_to(resolved_pkg_root)
        ):
            return True
    except (OSError, RuntimeError, ValueError):
        pass
    return False


def _has_existing_files(target_dir: Path) -> bool:
    """Check if target directory exists and contains any files or directories."""
    if not target_dir.exists():
        return False
    if target_dir.is_file():
        return True
    try:
        return any(target_dir.iterdir())
    except OSError:
        return True


def _run_guide(args: argparse.Namespace) -> int:
    """Handle the 'guide' subcommand."""
    if args.list:
        sys.stdout.write(_format_topic_list())
        return 0

    resource_root = _get_skill_resource()
    if not resource_root.is_dir():
        sys.stderr.write("Error: Packaged f8-modeling skill resources not found.\n")
        return 1

    if args.all or args.topic == "all":
        sections = [resource_root.joinpath("SKILL.md").read_text(encoding="utf-8").strip()]
        for rel_path, _ in GUIDE_TOPICS.values():
            if rel_path:
                sections.append(
                    resource_root.joinpath(*rel_path.split("/")).read_text(encoding="utf-8").strip()
                )
        sys.stdout.write("\n\n---\n\n".join(sections) + "\n")
        return 0

    if args.topic is None:
        target = resource_root.joinpath("SKILL.md")
    else:
        rel_path, _ = GUIDE_TOPICS[args.topic]
        parts = rel_path.split("/")
        target = resource_root.joinpath(*parts)

    if not target.is_file():
        sys.stderr.write(f"Error: Guide resource '{target.name}' not found.\n")
        return 1

    content = target.read_text(encoding="utf-8")
    sys.stdout.write(content)
    if not content.endswith("\n"):
        sys.stdout.write("\n")
    return 0


def _run_agent_init(args: argparse.Namespace) -> int:
    """Handle the 'agent init' subcommand."""
    dest = Path(args.dest)
    target_dir = dest / ".agents" / "skills" / "f8-modeling"
    skill_resource = _get_skill_resource()

    if not skill_resource.is_dir():
        sys.stderr.write("Error: Packaged f8-modeling skill resources not found.\n")
        return 1

    if _is_self_copy(dest, target_dir, skill_resource):
        sys.stderr.write("Error: Destination path resolves to package resource directory.\n")
        return 1

    if _has_existing_files(target_dir) and not args.force:
        sys.stderr.write(
            f"Error: Target directory '{target_dir}' already exists and contains files. "
            "Use --force to overwrite.\n"
        )
        return 1

    if target_dir.is_file():
        target_dir.unlink()

    target_dir.mkdir(parents=True, exist_ok=True)
    _copy_traversable(skill_resource, target_dir)
    sys.stdout.write(f"Initialized f8-modeling skill in {target_dir}\n")
    return 0


def _build_parser() -> tuple[argparse.ArgumentParser, argparse.ArgumentParser]:
    """Construct top-level and subcommand argument parsers."""
    parser = argparse.ArgumentParser(
        prog="f8",
        description="F8 Financial Modeling SDK runner and agent onboarding helper.",
    )
    parser.add_argument(
        "-V",
        "--version",
        action="version",
        version=f"%(prog)s {__version__}",
    )
    subparsers = parser.add_subparsers(dest="command")

    # guide subcommand
    guide_parser = subparsers.add_parser(
        "guide",
        help="Stream financial modeling guide and reference documentation.",
        description="Stream financial modeling guide and reference documentation to stdout.",
    )
    guide_parser.add_argument(
        "topic",
        nargs="?",
        choices=list(GUIDE_TOPICS.keys()),
        default=None,
        help="Specific topic guide to display.",
    )
    guide_parser.add_argument(
        "--all",
        action="store_true",
        help="Output complete concatenated documentation.",
    )
    guide_parser.add_argument(
        "--list",
        action="store_true",
        help="List available topics with summaries.",
    )

    # agent subcommand
    agent_parser = subparsers.add_parser(
        "agent",
        help="Agent skill management commands.",
        description="Agent skill management commands.",
    )
    agent_subparsers = agent_parser.add_subparsers(dest="agent_command")

    init_parser = agent_subparsers.add_parser(
        "init",
        help="Initialize agent modeling skill in target workspace.",
        description=(
            "Scaffold the .agents/skills/f8-modeling directory tree into the target workspace."
        ),
    )
    init_parser.add_argument(
        "--dest",
        default=".",
        help="Target root directory (default: current working directory).",
    )
    init_parser.add_argument(
        "-f",
        "--force",
        action="store_true",
        help="Overwrite existing skill files if present.",
    )

    return parser, agent_parser


def main(argv: Sequence[str] | None = None) -> int:
    """Main CLI entrypoint for the module runner."""
    if argv is None:
        argv = sys.argv[1:]

    parser, agent_parser = _build_parser()

    if not argv:
        parser.print_help(sys.stdout)
        return 0

    args = parser.parse_args(argv)

    if args.command == "guide":
        return _run_guide(args)
    if args.command == "agent":
        if getattr(args, "agent_command", None) == "init":
            return _run_agent_init(args)
        agent_parser.print_help(sys.stdout)
        return 0

    parser.print_help(sys.stdout)
    return 0
