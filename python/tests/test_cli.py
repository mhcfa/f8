"""Unit tests for the f8 CLI and module runner entrypoints."""

from __future__ import annotations

import subprocess
import sys
from importlib.resources import files
from pathlib import Path

import f8
import pytest
from f8.cli import GUIDE_TOPICS, main


def test_empty_invocation_prints_usage(capsys: pytest.CaptureFixture[str]) -> None:
    """Verify that calling f8 with no arguments writes usage to stdout and exits with 0."""
    exit_code = main([])
    assert exit_code == 0
    captured = capsys.readouterr()
    assert "usage: f8" in captured.out
    assert "guide" in captured.out
    assert "agent" in captured.out
    assert captured.err == ""


def test_agent_subcommand_without_args_prints_help(capsys: pytest.CaptureFixture[str]) -> None:
    """Verify calling f8 agent without sub-subcommand writes help to stdout and exits with 0."""
    exit_code = main(["agent"])
    assert exit_code == 0
    captured = capsys.readouterr()
    assert "usage: f8 agent" in captured.out
    assert "init" in captured.out


def test_guide_default_prints_skill_overview(capsys: pytest.CaptureFixture[str]) -> None:
    """Scenario 1: Output modeling guide overview (SKILL.md) to stdout."""
    exit_code = main(["guide"])
    assert exit_code == 0
    captured = capsys.readouterr()
    assert "name: f8-modeling" in captured.out
    assert "# f8 Financial Modeling Guide" in captured.out
    assert "Mental Model" in captured.out
    assert captured.err == ""


@pytest.mark.parametrize("topic", ["syntax", "functions", "audit"])
def test_guide_specific_topic(topic: str, capsys: pytest.CaptureFixture[str]) -> None:
    """Scenario 2: Output specific topic guide to stdout."""
    exit_code = main(["guide", topic])
    assert exit_code == 0
    captured = capsys.readouterr()
    rel_path, _ = GUIDE_TOPICS[topic]
    target_res = files("f8").joinpath("skills", "f8-modeling", *rel_path.split("/"))
    expected_content = target_res.read_text(encoding="utf-8")
    assert captured.out == expected_content
    assert captured.err == ""


def test_guide_list_topics(capsys: pytest.CaptureFixture[str]) -> None:
    """Scenario 3: List available guide topics with aligned summaries."""
    exit_code = main(["guide", "--list"])
    assert exit_code == 0
    captured = capsys.readouterr()
    expected_lines = [
        "Available topics:",
        "  syntax     2D coordinates, vector expressions, column roles, gutters, indentation",
        "  functions  44 built-in Excel functions, signatures, omission syntax, errors",
        "  audit      TableAuditor, inspection, unboxing, structured exception recovery",
        "  all        Complete concatenated guide",
    ]
    assert captured.out.strip() == "\n".join(expected_lines)
    assert captured.err == ""


@pytest.mark.parametrize("args", [["guide", "--all"], ["guide", "all"]])
def test_guide_all_concatenated(args: list[str], capsys: pytest.CaptureFixture[str]) -> None:
    """Scenario 4: Output complete concatenated guide separated by delimiters."""
    exit_code = main(args)
    assert exit_code == 0
    captured = capsys.readouterr()

    root = files("f8").joinpath("skills", "f8-modeling")
    sections = [
        root.joinpath("SKILL.md").read_text(encoding="utf-8").strip(),
        root.joinpath("references", "syntax-and-grid.md").read_text(encoding="utf-8").strip(),
        root.joinpath("references", "functions.md").read_text(encoding="utf-8").strip(),
        root.joinpath("references", "auditing-and-repair.md").read_text(encoding="utf-8").strip(),
    ]
    expected = "\n\n---\n\n".join(sections) + "\n"
    assert captured.out == expected
    assert captured.err == ""


def test_guide_invalid_topic_exits_with_code_2(capsys: pytest.CaptureFixture[str]) -> None:
    """Scenario 5: Reject invalid guide topic and exit with code 2."""
    with pytest.raises(SystemExit) as exc_info:
        main(["guide", "unknown_topic"])
    assert exc_info.value.code == 2
    captured = capsys.readouterr()
    assert "invalid choice: 'unknown_topic'" in captured.err


def test_agent_init_empty_workspace(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    """Scenario 6: Initialize agent skill directory tree in empty workspace."""
    workspace = tmp_path / "my_project"
    exit_code = main(["agent", "init", "--dest", str(workspace)])
    assert exit_code == 0
    captured = capsys.readouterr()
    assert "Initialized f8-modeling skill in" in captured.out

    skill_dir = workspace / ".agents" / "skills" / "f8-modeling"
    assert skill_dir.is_dir()
    assert (skill_dir / "SKILL.md").is_file()
    assert (skill_dir / "references" / "syntax-and-grid.md").is_file()
    assert (skill_dir / "references" / "functions.md").is_file()
    assert (skill_dir / "references" / "auditing-and-repair.md").is_file()

    # Invariant: zero .py files in skill scaffolding
    py_files = list(skill_dir.rglob("*.py"))
    assert len(py_files) == 0


def test_agent_init_collision_without_force(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Scenario 7: Reject overwrite on collision without force flag."""
    workspace = tmp_path / "colliding_project"
    exit_code = main(["agent", "init", "--dest", str(workspace)])
    assert exit_code == 0

    skill_file = workspace / ".agents" / "skills" / "f8-modeling" / "SKILL.md"
    original_mtime = skill_file.stat().st_mtime_ns

    # Re-run without force
    collision_code = main(["agent", "init", "--dest", str(workspace)])
    assert collision_code == 1
    captured = capsys.readouterr()
    assert "already exists and contains files" in captured.err
    assert "Use --force to overwrite" in captured.err

    # Verify file was not modified
    assert skill_file.stat().st_mtime_ns == original_mtime


def test_agent_init_overwrite_with_force(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Scenario 8: Overwrite existing skill directory with force flag."""
    workspace = tmp_path / "force_project"
    exit_code = main(["agent", "init", "--dest", str(workspace)])
    assert exit_code == 0

    skill_file = workspace / ".agents" / "skills" / "f8-modeling" / "SKILL.md"
    skill_file.write_text("Modified content", encoding="utf-8")
    assert skill_file.read_text(encoding="utf-8") == "Modified content"

    # Re-run with force
    force_code = main(["agent", "init", "--dest", str(workspace), "--force"])
    assert force_code == 0
    captured = capsys.readouterr()
    assert "Initialized f8-modeling skill in" in captured.out

    # Content restored to packaged asset
    expected_skill = (
        files("f8").joinpath("skills", "f8-modeling", "SKILL.md").read_text(encoding="utf-8")
    )
    assert skill_file.read_text(encoding="utf-8") == expected_skill


def test_agent_init_prevents_self_copy(capsys: pytest.CaptureFixture[str]) -> None:
    """Scenario 9: Prevent self-copy into source asset tree."""
    pkg_dir = Path(str(files("f8"))).resolve()
    exit_code = main(["agent", "init", "--dest", str(pkg_dir)])
    assert exit_code == 1
    captured = capsys.readouterr()
    assert "Destination path resolves to package resource directory" in captured.err

    # Test pointing directly to skill source directory
    skill_src_dir = pkg_dir / "skills" / "f8-modeling"
    exit_code_src = main(["agent", "init", "--dest", str(skill_src_dir)])
    assert exit_code_src == 1
    captured_src = capsys.readouterr()
    assert "Destination path resolves to package resource directory" in captured_src.err


def test_module_runner_subprocess_execution() -> None:
    """Verify python -m f8 invocation via subprocess."""
    result = subprocess.run(
        [sys.executable, "-m", "f8", "guide", "--list"],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0
    assert "Available topics:" in result.stdout
    assert "syntax" in result.stdout
    assert "functions" in result.stdout
    assert "audit" in result.stdout
    assert "all" in result.stdout


@pytest.mark.parametrize("flag", ["--version", "-V"])
def test_version_flag_outputs_package_version(
    flag: str, capsys: pytest.CaptureFixture[str]
) -> None:
    """Verify --version and -V print the Cargo workspace version and exit with 0."""
    with pytest.raises(SystemExit) as exc_info:
        main([flag])
    assert exc_info.value.code == 0
    captured = capsys.readouterr()
    assert captured.out.strip() == f"f8 {f8.__version__}"
    assert len(f8.__version__.split(".")) >= 3
