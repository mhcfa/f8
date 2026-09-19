---
title: Modern Lookup & Reference Functions Suite (XLOOKUP, XMATCH, INDEX)
date: 2026-09-19
status: completed
target-living-spec: specs/living-specs/functions/registry.md
---

## Proposal

### What

Implement the modern Excel lookup and coordinate reference function suite:
- `XLOOKUP`: Modern directional vector lookup with exact, approximate, and wildcard matching, reverse search, binary search, and lazy fallback evaluation.
- `XMATCH`: Modern relative position lookup returning 1-based coordinate indices across 1D vectors with shared match and search modes.
- `INDEX`: Positional coordinate retrieval supporting 1D vectors and 2D matrix intersections.

In addition, implement a shared 1D lookup engine (`src/functions/lookup/search.rs`) consolidating match modes (`0`, `-1`, `1`, `2`), search modes (`1`, `-1`, `2`, `-2`), and unified wildcard pattern matching.

### Why

1. **Foundational modeling triad**: `XLOOKUP`, `XMATCH`, and `INDEX` represent the clean, non-volatile modern standard for tabular data retrieval, scenario dispatch, and 2D matrix intersections in financial modeling, superseding legacy `VLOOKUP`/`HLOOKUP`/`MATCH`.
2. **Deterministic and non-volatile**: None of these functions are volatile. They depend strictly on precedent cell coordinates and participate deterministically in topological dependency graphs.
3. **Shared search engine**: `XLOOKUP` and `XMATCH` share ~95% of their search and matching mechanics. Consolidating this logic into a single source of truth eliminates duplicate search algorithms and ensures identical edge-case handling.
4. **2D coordinate symmetry**: Combining `INDEX` with `XMATCH` provides intuitive, mathematically clean $(y, x)$ coordinate lookups (`INDEX(matrix, XMATCH(...), XMATCH(...))`) without awkward nested vector lookups.
5. **Short-circuiting fallback parity**: `XLOOKUP`'s `if_not_found` parameter must evaluate lazily to prevent unneeded sub-expression execution or error bubbling when a match is successfully found.

### Scope

- **In-scope**:
  - `src/functions/lookup/mod.rs`: Module exports and registry dispatcher for `FunctionCategory::Lookup`.
  - `src/functions/lookup/search.rs`: Shared 1D vector search and matching engine (`match_mode`, `search_mode`, binary search, approximate bounds, and wildcard matching).
  - `src/functions/lookup/xlookup.rs`: `XlookupFunction` implementation.
  - `src/functions/lookup/xmatch.rs`: `XmatchFunction` implementation.
  - `src/functions/lookup/index.rs`: `IndexFunction` implementation.
  - Expose wildcard parsing and matching from `src/functions/common.rs` for shared consumption between `*IFS` criteria and `XLOOKUP`/`XMATCH`.
  - Integration into `src/functions/mod.rs` and `FunctionRegistry::standard()`.
  - Comprehensive unit and integration test suite in `tests/function_lookup_test.rs`.
  - Update standard function count in `tests/function_registry_test.rs` and `tests/function_math_test.rs` (from 32 to 35 functions).
- **Out-of-scope**:
  - Legacy lookup functions (`VLOOKUP`, `HLOOKUP`, `MATCH`, `LOOKUP`).
  - Dynamic array spilling or whole-vector returns (e.g. `INDEX` with `row_num = 0` spilling entire columns).
  - Volatile reference functions (`OFFSET`, `INDIRECT`).
  - Modulo arithmetic (`MOD`), which belongs in `finx::functions::math`.

### What Changes

- `src/functions/common.rs`: Centralize or re-export wildcard parsing and matching tokens (`WildcardToken`, `matches_wildcard`).
- `src/functions/lookup/`: New module with `search.rs`, `xlookup.rs`, `xmatch.rs`, `index.rs`, and `mod.rs`.
- `src/functions/mod.rs`: Declare `pub mod lookup;`.
- `src/functions/registry.rs`: Register lookup functions in `FunctionRegistry::standard()`.
- `docs/functions/catalog.md`: Add `XMATCH` to Section 5.
- `tests/function_lookup_test.rs`: New integration tests covering 1D vectors, 2D matrices, exact match, approximate smaller/larger, reverse search, binary search, wildcards, and bounds checking.
- `tests/function_registry_test.rs`: Update registry count assertion to 35.

### Capabilities

- **New capabilities**:
  - `XLOOKUP`: Finds values in 1D vectors (horizontal or vertical), returning corresponding elements from a return vector. Supports default exact match (`match_mode: 0`), next smaller (`-1`), next larger (`1`), wildcards (`2`), linear scan (`1`), reverse scan (`-1`), and binary search (`2`, `-2`). Lazily evaluates `if_not_found`.
  - `XMATCH`: Searches 1D vectors and returns the 1-based relative position integer. Supports identical match and search modes as `XLOOKUP`. Returns `#N/A` on failure.
  - `INDEX`: Retrieves cell values at specified 1-based row and column offsets in 1D ranges or 2D matrix grids. Returns `#REF!` on out-of-bounds coordinates.
- **Modified capabilities**:
  - Standard function registry expands from 32 to 35 functions.

---

## Design

### Goals

1. **Excel parity**: Full compliance with Microsoft Excel 365 / 2021 calculation semantics for `XLOOKUP`, `XMATCH`, and `INDEX`.
2. **Single source of truth**: Consolidate linear scan, reverse scan, binary search, and approximate matching into `src/functions/lookup/search.rs`.
3. **Lazy fallback evaluation**: Ensure `if_not_found` in `XLOOKUP` is never evaluated when a match is found.
4. **Strict dimension checking**: Enforce 1D vectors for lookup arrays; enforce dimension matching between `lookup_array` and `return_array`.
5. **Zero production panics**: Use `thiserror`, bubble calculation errors via `?`, and avoid unwraps.

### Invariants

- **1D vector constraint for lookups**: `lookup_array` in `XLOOKUP` and `XMATCH` must be a 1D vector (either $1 \times N$ or $N \times 1$). If `rows > 1 && cols > 1`, return `ExcelError::Value`.
- **Dimension parity between lookup and return**: In `XLOOKUP`, `return_array` must have the same number of elements as `lookup_array` ($N$). If lengths differ, return `ExcelError::Value`.
- **Error propagation in lookup value**: If `lookup_value` is an error (e.g. `#REF!`, `#DIV/0!`), it bubbles up immediately without searching.
- **Search and match mode validity**:
  - `match_mode` must be an integer in `{-1, 0, 1, 2}`. Other values return `ExcelError::Value`.
  - `search_mode` must be an integer in `{-2, -1, 1, 2}`. Other values return `ExcelError::Value`.
  - Combining `match_mode = 2` (wildcards) with binary search (`search_mode = 2` or `-2`) returns `ExcelError::Value`.
- **INDEX coordinate bounds**:
  - 1-based coordinates: row and column numbers are truncated to integers.
  - Negative or zero indices, or indices exceeding array dimensions, return `ExcelError::Ref`.
  - On a 1D row vector ($1 \times C$), a single coordinate argument is interpreted as the column offset.
  - On a 2D matrix ($R \times C$ where $R > 1, C > 1$), omitting `col_num` returns `ExcelError::Value`.

### Decisions

- **Decision 1: Lazy binding for `if_not_found`**:
  - **Choice**: Define `if_not_found` parameter in `XLOOKUP` with `ParamKind::Lazy`.
  - **Rationale**: Prevents evaluating expensive expressions or premature error bubbling when a match is found (e.g. `XLOOKUP("key", keys, vals, 1/0)`).
  - **Alternatives considered**: `ParamKind::Scalar`. Rejected because eager evaluation breaks Excel short-circuiting parity.
- **Decision 2: Dedicated shared search module**:
  - **Choice**: Implement `find_match_index` in `src/functions/lookup/search.rs` returning `Result<Option<usize>, ExcelError>`.
  - **Rationale**: `XMATCH` and `XLOOKUP` share exact matching, approximate matching, binary search, and wildcard logic. Implementing it once avoids divergent behavior.
  - **Alternatives considered**: Implementing search directly inside `xlookup.rs` and delegating from `xmatch.rs`. Rejected to preserve separation of concerns and avoid circular dependencies.
- **Decision 3: Reusing wildcard engine**:
  - **Choice**: Move `WildcardToken` and `matches_wildcard` to `src/functions/common.rs` (or expose them).
  - **Rationale**: Criteria matching (`*IFS`) and lookup wildcard matching (`match_mode = 2`) follow identical Excel wildcard rules (`*`, `?`, `~`). Reusing this engine prevents logic duplication.

### Data Contracts

```rust
/// Match mode configuration for XLOOKUP and XMATCH.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    /// Exact match (default: 0).
    Exact,
    /// Exact match or next smaller item (-1).
    ExactOrNextSmaller,
    /// Exact match or next larger item (1).
    ExactOrNextLarger,
    /// Wildcard match with `*`, `?`, and `~` (2).
    Wildcard,
}

/// Search mode configuration for XLOOKUP and XMATCH.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Linear search first-to-last (default: 1).
    FirstToLast,
    /// Linear search last-to-first (-1).
    LastToFirst,
    /// Binary search ascending (2).
    BinaryAscending,
    /// Binary search descending (-2).
    BinaryDescending,
}

/// Performs 1D vector lookup returning the 0-based matching index.
pub fn find_match_index(
    lookup_val: &CellValue,
    lookup_slice: &[CellValue],
    match_mode: MatchMode,
    search_mode: SearchMode,
) -> Result<Option<usize>, ExcelError>;
```

### Risks & Trade-offs

- **Binary search on unsorted data**: In Excel, binary search on unsorted inputs exhibits undefined/implementation-dependent behavior. Mitigation: Standard binary partition on `CellValue::excel_cmp` reproduces Excel-compatible binary branching.
- **Approximate matching on unsorted data**: In linear search modes (`1` and `-1`), approximate matching (`-1` or `1`) searches all elements to find the nearest bound, breaking ties by first-encountered order. Mitigation: Exhaustive linear scan tracking the best candidate index ensures deterministic behavior without requiring presorted arrays.

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Exact match with XLOOKUP (vertical and horizontal)**
  - **Given** a 1D column range `keys` containing `["A", "B", "C"]` and `values` containing `[10, 20, 30]`
  - **When** evaluating `XLOOKUP("B", keys, values)`
  - **Then** evaluates to `CellValue::Number(20.0)`.
  - **When** evaluating across a 1D horizontal row range
  - **Then** produces identical matching result.

- [x] **Scenario 2: Fallback handling with XLOOKUP**
  - **Given** a 1D range without key `"D"`
  - **When** evaluating `XLOOKUP("D", keys, values, "Missing")`
  - **Then** evaluates to `CellValue::Text("Missing")`.
  - **When** evaluating `XLOOKUP("D", keys, values)` without `if_not_found`
  - **Then** evaluates to `ExcelError::Na`.

- [x] **Scenario 3: Short-circuiting lazy fallback in XLOOKUP**
  - **Given** key `"A"` exists in `keys`
  - **When** evaluating `XLOOKUP("A", keys, values, 1/0)`
  - **Then** evaluates to `10.0` without evaluating `1/0` or raising `#DIV/0!`.

- [x] **Scenario 4: Reverse search in XLOOKUP and XMATCH**
  - **Given** a range containing duplicate keys `["A", "B", "A"]` and values `[1, 2, 3]`
  - **When** evaluating `XLOOKUP("A", keys, values, _, 0, -1)`
  - **Then** evaluates to `CellValue::Number(3.0)`.
  - **When** evaluating `XMATCH("A", keys, 0, -1)`
  - **Then** evaluates to `CellValue::Number(3.0)`.

- [x] **Scenario 5: Approximate match (next smaller and next larger)**
  - **Given** a range containing `[10, 20, 30]`
  - **When** evaluating `XMATCH(25, range, -1)`
  - **Then** finds next smaller item `20` and returns index `2.0`.
  - **When** evaluating `XMATCH(25, range, 1)`
  - **Then** finds next larger item `30` and returns index `3.0`.
  - **When** evaluating `XMATCH(5, range, -1)`
  - **Then** returns `ExcelError::Na` as no smaller item exists.

- [x] **Scenario 6: Wildcard matching in XLOOKUP and XMATCH**
  - **Given** a text range containing `["Apple", "Banana", "Apricot"]`
  - **When** evaluating `XMATCH("Ap*", range, 2)`
  - **Then** returns `1.0` (matching `"Apple"`).
  - **When** evaluating with reverse search `XMATCH("Ap*", range, 2, -1)`
  - **Then** returns `3.0` (matching `"Apricot"`).

- [x] **Scenario 7: Binary search ascending and descending**
  - **Given** sorted ascending numbers `[10, 20, 30, 40, 50]`
  - **When** evaluating `XMATCH(30, range, 0, 2)`
  - **Then** returns `3.0`.
  - **Given** sorted descending numbers `[50, 40, 30, 20, 10]`
  - **When** evaluating `XMATCH(40, range, 0, -2)`
  - **Then** returns `2.0`.

- [x] **Scenario 8: Dimension mismatch and 2D rejection in XLOOKUP and XMATCH**
  - **Given** a 2D lookup array ($2 \times 3$)
  - **When** evaluating `XLOOKUP("A", matrix, values)` or `XMATCH("A", matrix)`
  - **Then** returns `ExcelError::Value`.
  - **Given** a 1D lookup array of 3 elements and return array of 4 elements
  - **When** evaluating `XLOOKUP("A", lookup, return)`
  - **Then** returns `ExcelError::Value`.

- [x] **Scenario 9: INDEX on 1D vector and 2D matrix**
  - **Given** a 1D row vector `[10, 20, 30]`
  - **When** evaluating `INDEX(row, 2)`
  - **Then** returns `20.0`.
  - **Given** a 2D matrix of $3 \times 3$
  - **When** evaluating `INDEX(matrix, 2, 3)`
  - **Then** returns the cell value at row 2, column 3.
  - **When** evaluating `INDEX(matrix, 4, 1)` (out-of-bounds)
  - **Then** returns `ExcelError::Ref`.

- [x] **Scenario 10: 2D matrix intersection with INDEX + XMATCH**
  - **Given** a 2D matrix with row headers `["North", "South"]` and column headers `["Q1", "Q2"]`
  - **When** evaluating `INDEX(matrix, XMATCH("South", row_headers), XMATCH("Q2", col_headers))`
  - **Then** returns the exact cell coordinate intersection.

---

## Tasks

- [x] **Task 1: Shared wildcard parsing consolidation**
  - [x] Move or re-export `WildcardToken` and `matches_wildcard` to make them accessible to lookup functions without code duplication.
- [x] **Task 2: Implement shared 1D search engine (`src/functions/lookup/search.rs`)**
  - [x] Implement `MatchMode` and `SearchMode` parsers and validation.
  - [x] Implement linear forward and reverse scans for exact, approximate smaller/larger, and wildcard matches.
  - [x] Implement binary search ascending and descending for exact and approximate matches.
  - [x] Enforce rejection of wildcard match mode with binary search.
- [x] **Task 3: Implement `XMATCH` (`src/functions/lookup/xmatch.rs`)**
  - [x] Define parameter contracts and arity rules.
  - [x] Validate 1D vector shape and invoke `find_match_index`.
  - [x] Return 1-based index or `#N/A`.
- [x] **Task 4: Implement `XLOOKUP` (`src/functions/lookup/xlookup.rs`)**
  - [x] Define parameter contracts with lazy `if_not_found`.
  - [x] Enforce 1D shapes and equal lengths between lookup and return arrays.
  - [x] Evaluate matching return cell or lazy fallback.
- [x] **Task 5: Implement `INDEX` (`src/functions/lookup/index.rs`)**
  - [x] Define parameter contracts.
  - [x] Handle 1D row, 1D column, and 2D matrix coordinate resolution.
  - [x] Perform 1-based bounds checking returning `#REF!`.
- [x] **Task 6: Registry integration and test verification**
  - [x] Wire `lookup` module into `src/functions/mod.rs` and `FunctionRegistry::standard()`.
  - [x] Write integration test suite `tests/function_lookup_test.rs`.
  - [x] Update function count assertions across test suite.
  - [x] Verify using standard Cargo suite (`check`, `test`, `clippy`, `fmt`).
- [x] **Task 7: Living spec update and archive**
  - [x] Update `specs/living-specs/functions/registry.md` and `docs/functions/catalog.md`.
  - [x] Move draft spec to `specs/history/`.
