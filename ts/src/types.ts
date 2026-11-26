import type { Cell, Model, Table } from "./builder.js"

export type CellScalarValue = number | string | boolean | null

export type CellValueDict =
  | { readonly Number: number }
  | { readonly Text: string }
  | { readonly Boolean: boolean }
  | "Empty"
  | { readonly Error: string }

export type ExcelErrorCode =
  | "#DIV/0!"
  | "#VALUE!"
  | "#REF!"
  | "#NUM!"
  | "#N/A"
  | "#NAME?"

export type ModelMarkdownDetail = "values" | "index" | "layout"

export type FormatPreset =
  | "currency"
  | "number"
  | "percentage"
  | "multiple"
  | "basis_points"
  | "date"
  | "text"
  | "integer"

export type CurrencyCode =
  | "USD"
  | "EUR"
  | "GBP"
  | "JPY"
  | "CHF"
  | "CAD"
  | "AUD"
  | "CNY"
  | "HKD"
  | "SGD"
  | "SEK"
  | "NOK"
  | "NZD"
  | "INR"
  | "BRL"
  | "usd"
  | "eur"
  | "gbp"
  | "jpy"
  | "chf"
  | "cad"
  | "aud"
  | "cny"
  | "hkd"
  | "sgd"
  | "sek"
  | "nok"
  | "nzd"
  | "inr"
  | "brl"

export type SemanticStyle = "data" | "bold" | "subtotal" | "total"

export type Shade = "none" | "muted" | "alt"

export type AxisRole = "measure" | "descriptor"

export type SheetCellKind =
  | "empty"
  | "table_title"
  | "table_subtitle"
  | "column_header"
  | "section_header"
  | "gutter"
  | "data"
  | "overflow"

export type SheetLayoutMode = "auto" | "grid"

export type PaperSize = "a4" | "letter"

export type PageOrientation = "landscape" | "portrait"

export type PrintFitMode = "fit_width" | "fit_page"

export interface HeaderFooterInput {
  readonly left?: string
  readonly center?: string
  readonly right?: string
}

export interface SheetPrintSpecInput {
  readonly paperSize: PaperSize
  readonly orientation?: PageOrientation
  readonly fit?: PrintFitMode
  readonly centerHorizontally?: boolean
  readonly header?: HeaderFooterInput
  readonly footer?: HeaderFooterInput
}

export interface SheetPrintOverrideInput {
  readonly paperSize?: PaperSize
  readonly orientation?: PageOrientation
  readonly fit?: PrintFitMode
  readonly centerHorizontally?: boolean
  readonly header?: HeaderFooterInput | "none"
  readonly footer?: HeaderFooterInput | "none"
}

export interface AutoSheetLayoutSpecInput {
  readonly kind: "auto"
}

export interface GridSheetLayoutSpecInput {
  readonly kind: "grid"
  readonly colWidth?: number
}

export type SheetLayoutSpecInput =
  | AutoSheetLayoutSpecInput
  | GridSheetLayoutSpecInput

export interface CellCoordinate {
  readonly table: string
  readonly row: string
  readonly col: string
}

export interface CellError {
  readonly coordinate: CellCoordinate
  readonly sheet: string
  readonly physicalAddress: string
  readonly error: ExcelErrorCode
  readonly formulaExpr: string | null
  readonly excelFormula: string | null
}

export interface ModelMarkdownOptions {
  readonly detail?: ModelMarkdownDetail
  readonly sheets?: readonly string[]
  readonly tables?: readonly string[]
  readonly showIds?: boolean
}

export interface TableSummary {
  readonly id: string
  readonly title: string
  readonly rowCount: number
  readonly colCount: number
  readonly cellCount: number
  readonly driverCount: number
  readonly formulaCount: number
}

export interface SheetSummary {
  readonly name: string
  readonly tables: readonly TableSummary[]
  readonly totalCells: number
}

export interface ModelSummary {
  readonly modelName: string
  readonly themeName: string
  readonly sheets: readonly SheetSummary[]
  readonly totalSheets: number
  readonly totalTables: number
  readonly totalCells: number
  readonly totalDrivers: number
  readonly totalFormulas: number
}

export interface CellNeighborInspection {
  readonly cellId: CellCoordinate
  readonly sheetAddress: string
  readonly value: CellValueDict | null
  readonly formattedValue: string | null
}

export interface CellInspection {
  readonly coordinate: CellCoordinate
  readonly sheet: string
  readonly physicalAddress: string
  readonly evaluatedValue: CellValueDict
  readonly formattedValue: string | null
  readonly format: FormatPreset | null
  readonly decimals: number | null
  readonly currency: CurrencyCode | null
  readonly isHighlight: boolean
  readonly shade: Shade | null
  readonly indent: number
  readonly isFormula: boolean
  readonly formulaExpr: string | null
  readonly excelFormula: string | null
  readonly note: string | null
  readonly precedents: readonly CellNeighborInspection[]
  readonly dependents: readonly CellNeighborInspection[]
}

export interface ColumnInspection {
  readonly id: string | null
  readonly group: string | null
  readonly headers: readonly string[]
  readonly isGutter: boolean
  readonly isDescriptor: boolean
  readonly style: SemanticStyle | null
  readonly shade: Shade | null
}

export interface RowInspection {
  readonly id: string | null
  readonly sectionTitle: string | null
  readonly isSectionHeader: boolean
  readonly isGutter: boolean
  readonly indent: number
  readonly style: SemanticStyle | null
  readonly shade: Shade | null
}

export interface TableCellInfo {
  readonly row: string
  readonly col: string
  readonly value: CellValueDict
  readonly formattedValue: string | null
  readonly isFormula: boolean
  readonly formulaExpr: string | null
  readonly excelFormula: string | null
  readonly physicalA1: string
  readonly format: FormatPreset | null
  readonly decimals: number | null
  readonly currency: CurrencyCode | null
}

export interface TableInspection {
  readonly tableId: string
  readonly title: string
  readonly subtitles: readonly string[]
  readonly sheet: string
  readonly columns: readonly ColumnInspection[]
  readonly rows: readonly RowInspection[]
  readonly cells: Readonly<Record<string, TableCellInfo>>
}

export interface SheetCellInspection {
  readonly physicalCol: number
  readonly physicalA1: string
  readonly tableId: string | null
  readonly rowId: string | null
  readonly colId: string | null
  readonly kind: SheetCellKind
  readonly isDescriptor: boolean
  readonly value: CellValueDict
  readonly formattedValue: string | null
  readonly formulaExpr: string | null
  readonly excelFormula: string | null
}

export interface SheetRowInspection {
  readonly physicalRow: number
  readonly excelRow: number
  readonly cells: readonly SheetCellInspection[]
}

export interface SheetInspection {
  readonly sheetId: string
  readonly title: string
  readonly tables: readonly string[]
  readonly minCol: number
  readonly maxCol: number
  readonly minRow: number
  readonly maxRow: number
  readonly colWidths: Readonly<Record<number, number>>
  readonly rows: readonly SheetRowInspection[]
}

export interface DependencyNode {
  readonly cellId: CellCoordinate
  readonly sheetAddress: string
  readonly value: CellValueDict | null
  readonly formattedValue: string | null
  readonly excelFormula: string | null
  readonly isPruned: boolean
  readonly children: readonly DependencyNode[]
}

export interface DependencyTree {
  readonly root: CellCoordinate
  readonly directCount: number
  readonly transitiveCount: number
  readonly nodes: readonly DependencyNode[]
}

export interface EmissionOptions {
  readonly valuesOnly?: boolean
  readonly omitFormulaCache?: boolean
  readonly title?: string
  readonly author?: string
}

export interface FormattedOptions {
  readonly unit?: string
}

export interface DependencyTraceOptions {
  readonly recursive?: boolean
}

export type TableOrient = "records" | "dict" | "index" | "split"

export type TableViewMode = "values" | "formulas" | "expr" | "addresses"

export interface SplitDict {
  readonly columns: string[]
  readonly index: string[]
  readonly data: CellScalarValue[][]
}

export interface TableMarkdownOptions {
  readonly view?: TableViewMode
  readonly rows?: readonly string[]
  readonly cols?: readonly string[]
  readonly includeSectionHeaders?: boolean
  readonly showIds?: boolean
  readonly compact?: boolean
}

export type CellValueLiteral = number | string | boolean | null

export interface DetailedCellInput {
  readonly value?: CellValueLiteral
  readonly expr?: string
  readonly style?: SemanticStyle
  readonly highlight?: boolean
  readonly shade?: Shade
  readonly format?: FormatPreset
  readonly decimals?: number
  readonly currency?: CurrencyCode
  readonly indent?: number
  readonly note?: string
}

export type CellInput = CellValueLiteral | DetailedCellInput

export interface FreezePanesInput {
  readonly row: number
  readonly col: number
}

export type TableRef = string | Table

export interface LaneInput {
  readonly tables: readonly TableRef[]
  readonly tableGap?: number
}

export type LaneInputItem = LaneInput | TableRef

export interface BandInput {
  readonly lanes: readonly LaneInputItem[]
  readonly gap?: number
  readonly laneGap?: number
  readonly pageBreakBefore?: boolean
}

export type BandInputItem = BandInput | TableRef

export interface LaneSpecInput {
  readonly tables: readonly string[]
  readonly tableGap?: number
}

export interface BandSpecInput {
  readonly lanes: readonly LaneSpecInput[]
  readonly gap?: number
  readonly laneGap?: number
  readonly pageBreakBefore?: boolean
}

export interface SheetSpecInput {
  readonly name: string
  readonly bands: readonly BandSpecInput[]
  readonly tabColor?: string
  readonly freezePanes?: FreezePanesInput
  readonly showGridlines?: boolean
  readonly layout?: SheetLayoutSpecInput
  readonly print?: SheetPrintSpecInput
}

export interface ProjectManifestInput {
  readonly modelName: string
  readonly theme?: string | Readonly<Record<string, unknown>>
  readonly sheets: readonly SheetSpecInput[]
}

export interface DataColumnSpecInput {
  readonly kind: "data"
  readonly id: string
  readonly role?: AxisRole
  readonly group?: string
  readonly headers?: readonly string[]
  readonly style?: SemanticStyle
  readonly highlight?: boolean
  readonly shade?: Shade
  readonly format?: FormatPreset
  readonly decimals?: number
  readonly currency?: CurrencyCode
  readonly expr?: string
}

export interface GutterColumnSpecInput {
  readonly kind: "gutter"
}

export type ColumnSpecInput = DataColumnSpecInput | GutterColumnSpecInput

export interface DataRowSpecInput {
  readonly kind: "data"
  readonly id: string
  readonly style?: SemanticStyle
  readonly highlight?: boolean
  readonly shade?: Shade
  readonly format?: FormatPreset
  readonly decimals?: number
  readonly currency?: CurrencyCode
  readonly expr?: string
  readonly cells?: Readonly<Record<string, CellInput>>
}

export interface SectionHeaderRowSpecInput {
  readonly kind: "section_header" | "sectionHeader"
  readonly title: string
  readonly indent?: number
  readonly note?: string
}

export interface GutterRowSpecInput {
  readonly kind: "gutter"
}

export type RowSpecInput =
  | DataRowSpecInput
  | SectionHeaderRowSpecInput
  | GutterRowSpecInput

export interface TableSpecInput {
  readonly id: string
  readonly title: string
  readonly subtitles?: readonly string[]
  readonly currency?: CurrencyCode
  readonly collapsible?: boolean
  readonly columns: readonly ColumnSpecInput[]
  readonly rows: readonly RowSpecInput[]
}

export type ThemeInput = string | Readonly<Record<string, unknown>> | URL

export type CellInputValue = CellValueLiteral | Cell | DetailedCellInput

export interface AxisSlice {
  readonly start: string
  readonly stop: string
}

export type AxisSelector = string | Iterable<string> | AxisSlice

export interface ModelOptions {
  readonly name?: string
  readonly theme?: ThemeInput
  readonly print?: SheetPrintSpecInput
}

export interface ModelSpec {
  readonly manifest: ProjectManifestInput
  readonly tables: readonly TableSpecInput[]
}

export interface SheetOptions {
  readonly tabColor?: string
  readonly showGridlines?: boolean
  readonly freezePanes?: FreezePanesInput
  readonly layout?: SheetLayoutMode
  readonly colWidth?: number
  readonly print?: SheetPrintOverrideInput | "none"
  readonly model?: Model
}

export interface TableOptions {
  readonly subtitle?: string | readonly string[]
  readonly currency?: CurrencyCode
  readonly collapsible?: boolean
}

export interface ColumnOptions {
  readonly role?: AxisRole
  readonly header?: string
  readonly headers?: readonly string[]
  readonly group?: string
  readonly style?: SemanticStyle
  readonly highlight?: boolean
  readonly shade?: Shade
  readonly format?: FormatPreset
  readonly decimals?: number
  readonly currency?: CurrencyCode
  readonly expr?: string
}

export interface RowOptions {
  readonly cells?: Readonly<Record<string, CellInputValue>>
  readonly expr?: string
  readonly style?: SemanticStyle
  readonly highlight?: boolean
  readonly shade?: Shade
  readonly format?: FormatPreset
  readonly decimals?: number
  readonly currency?: CurrencyCode
}

export interface SectionHeaderOptions {
  readonly indent?: number
  readonly note?: string
}
