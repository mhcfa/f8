"""Direct on-disk project loading and compilation."""

from __future__ import annotations

import json
from pathlib import Path
from typing import cast

from f8._core import CompiledModel
from f8._core import compile as core_compile
from f8.types import ProjectManifestDict, TableSpecDict, ThemeInput


def compile_project(
    dir_path: str | Path,
    theme: ThemeInput | Path | None = None,
) -> CompiledModel:
    """Load an existing multi-file JSON project from disk and compile it.

    Locates `<dir_path>/f8.json` and contained `<dir_path>/tables/*.json` specifications,
    assembles them in deterministic order, and compiles directly into a `CompiledModel`.

    Args:
        dir_path: Path to the project root directory containing `f8.json`, or path to `f8.json`.
        theme: Optional theme name, theme file path, or custom theme definition overriding
            manifest theme.

    Returns:
        CompiledModel: Fully validated and evaluated compiled model.

    Raises:
        FileNotFoundError: If the directory or `f8.json` manifest is missing.
        ValueError: If duplicate table IDs or malformed table files are discovered
            across JSON files.
    """
    path = Path(dir_path)

    if path.is_file():
        if path.name == "f8.json":
            manifest_path = path
            root_dir = path.parent
        else:
            raise FileNotFoundError(f"Expected project directory or 'f8.json', got file '{path}'")
    elif path.is_dir():
        manifest_path = path / "f8.json"
        root_dir = path
    else:
        raise FileNotFoundError(f"Project path '{dir_path}' does not exist")

    if not manifest_path.is_file():
        raise FileNotFoundError(f"Project manifest 'f8.json' not found in '{root_dir}'")

    manifest_raw = manifest_path.read_text(encoding="utf-8")
    manifest: ProjectManifestDict = json.loads(manifest_raw)

    discovered_tables: dict[str, tuple[Path, TableSpecDict]] = {}
    tables_dir = root_dir / "tables"

    if tables_dir.is_dir():
        file_paths = sorted(
            [p for p in tables_dir.iterdir() if p.is_file() and p.suffix == ".json"],
            key=lambda p: p.name,
        )
        for p in file_paths:
            content = p.read_text(encoding="utf-8")
            loaded: object = json.loads(content)
            if not isinstance(loaded, dict):
                raise ValueError(
                    f"Table file '{p}' must contain a JSON object, got {type(loaded).__name__}"
                )
            table_dict = cast(TableSpecDict, loaded)
            raw_id = table_dict.get("id")
            if not isinstance(raw_id, str) or not raw_id.strip():
                raise ValueError(f"Table file '{p}' must specify a non-empty string 'id'")
            table_id = raw_id.strip()
            if table_id in discovered_tables:
                existing_path = discovered_tables[table_id][0]
                raise ValueError(
                    f"Duplicate table id '{table_id}' found in '{p}' and '{existing_path}'"
                )
            discovered_tables[table_id] = (p, table_dict)

    tables: list[TableSpecDict] = []
    seen: set[str] = set()

    for sheet in manifest.get("sheets", []):
        for table_id in sheet.get("tables", []):
            if table_id in discovered_tables and table_id not in seen:
                tables.append(discovered_tables[table_id][1])
                seen.add(table_id)

    for table_id, (_, table_spec) in discovered_tables.items():
        if table_id not in seen:
            tables.append(table_spec)
            seen.add(table_id)

    theme_to_pass: ThemeInput | None = None
    if theme is None:
        raw_theme = manifest.get("theme")
        if raw_theme and raw_theme.strip():
            candidate = root_dir / raw_theme.strip()
            if candidate.is_file():
                custom_theme_content: ThemeInput = json.loads(candidate.read_text(encoding="utf-8"))
                theme_to_pass = custom_theme_content
    elif isinstance(theme, (str, Path)):
        theme_path = Path(theme)
        candidate = root_dir / theme_path
        if not candidate.is_file() and theme_path.is_file():
            candidate = theme_path
        if candidate.is_file():
            custom_theme_content_override: ThemeInput = json.loads(
                candidate.read_text(encoding="utf-8")
            )
            theme_to_pass = custom_theme_content_override
        else:
            theme_to_pass = str(theme)
    else:
        theme_to_pass = theme

    return core_compile(manifest, tables, theme=theme_to_pass)


__all__ = ["compile_project"]
