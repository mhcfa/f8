use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

use super::error::ModelError;
use super::id::TableId;
use super::print::{
    HeaderFooterSection, MAX_EXCEL_PAGE_BREAKS, PrintFitMode, SheetPrintSpec,
    compile_header_footer, validate_header_footer_spec,
};
use super::theme::{Theme, ThemeSpec, is_valid_hex_color};

/// Maximum column count supported by OpenXML / Excel (columns 0..=16,383, i.e. A..XFD).
pub const MAX_EXCEL_COLUMNS: u32 = 16_384;

/// Maximum row count supported by OpenXML / Excel (rows 0..=1,048,575, i.e. 1..1,048,576).
pub const MAX_EXCEL_ROWS: u32 = 1_048_576;

/// Minimum allowed uniform column width on a `grid` worksheet (Excel character units).
pub const MIN_GRID_COL_WIDTH: f64 = 1.0;

/// Maximum allowed uniform column width on a `grid` worksheet (Excel 255-char limit).
pub const MAX_GRID_COL_WIDTH: f64 = 255.0;

/// Default vertical blank row count between stacked bands and stacked tables within a lane.
pub const DEFAULT_VERTICAL_TABLE_GAP: u32 = 2;

/// Default horizontal blank column count between adjacent lanes within a band.
pub const DEFAULT_HORIZONTAL_TABLE_GAP: u32 = 1;

fn validate_sheet_name(name: &str) -> Result<(), ModelError> {
    if name.is_empty() {
        return Err(ModelError::InvalidSheetName {
            sheet: name.to_string(),
            reason: "sheet name must not be empty",
        });
    }
    if name.encode_utf16().count() > 31 {
        return Err(ModelError::InvalidSheetName {
            sheet: name.to_string(),
            reason: "sheet name must not exceed 31 UTF-16 code units",
        });
    }
    if name.starts_with('\'') || name.ends_with('\'') {
        return Err(ModelError::InvalidSheetName {
            sheet: name.to_string(),
            reason: "sheet name must not start or end with a single quote",
        });
    }
    if name
        .chars()
        .any(|c| matches!(c, ':' | '\\' | '/' | '?' | '*' | '[' | ']'))
    {
        return Err(ModelError::InvalidSheetName {
            sheet: name.to_string(),
            reason: "sheet name must not contain ':', '\\', '/', '?', '*', '[', or ']'",
        });
    }
    if name.eq_ignore_ascii_case("history") {
        return Err(ModelError::InvalidSheetName {
            sheet: name.to_string(),
            reason: "sheet name 'History' is reserved by Excel",
        });
    }
    Ok(())
}

/// Root workbook manifest defining model metadata, theme, and sheet ordering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub model_name: String,
    #[serde(default)]
    pub theme: ThemeSpec,
    pub sheets: Vec<SheetSpec>,
}

impl ProjectManifest {
    /// Validates sheet naming uniqueness and structural invariants.
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.sheets.is_empty() {
            return Err(ModelError::EmptySheets);
        }
        let mut seen_sheets = IndexSet::new();
        let mut seen_tables: IndexMap<&TableId, &str> = IndexMap::new();
        for sheet in &self.sheets {
            validate_sheet_name(&sheet.name)?;
            if !seen_sheets.insert(sheet.name.to_lowercase()) {
                return Err(ModelError::DuplicateSheetName {
                    sheet: sheet.name.clone(),
                });
            }
            if let Some(color) = &sheet.tab_color
                && !is_valid_hex_color(color)
            {
                return Err(ModelError::InvalidSheetTabColor {
                    sheet: sheet.name.clone(),
                    color: color.clone(),
                });
            }
            if let Some(freeze) = sheet.freeze_panes
                && (freeze.row >= MAX_EXCEL_ROWS || freeze.col >= MAX_EXCEL_COLUMNS)
            {
                return Err(ModelError::InvalidFreezePanes {
                    sheet: sheet.name.clone(),
                    row: freeze.row,
                    col: freeze.col,
                    max_rows: MAX_EXCEL_ROWS,
                    max_cols: MAX_EXCEL_COLUMNS,
                });
            }
            match sheet.layout {
                SheetLayoutSpec::Auto | SheetLayoutSpec::Grid { col_width: None } => {}
                SheetLayoutSpec::Grid { col_width: Some(w) } => {
                    if !w.is_finite() || !(MIN_GRID_COL_WIDTH..=MAX_GRID_COL_WIDTH).contains(&w) {
                        return Err(ModelError::InvalidSheetColWidth {
                            sheet: sheet.name.clone(),
                            col_width: w,
                        });
                    }
                }
            }

            if let Some(print) = &sheet.print {
                if let Some(header) = &print.header {
                    validate_header_footer_spec(
                        &sheet.name,
                        HeaderFooterSection::Header,
                        header,
                        &self.model_name,
                    )?;
                }
                if let Some(footer) = &print.footer {
                    validate_header_footer_spec(
                        &sheet.name,
                        HeaderFooterSection::Footer,
                        footer,
                        &self.model_name,
                    )?;
                }
            }

            let mut page_break_count: usize = 0;

            for (band_index, band) in sheet.bands.iter().enumerate() {
                if band.lanes.is_empty() {
                    return Err(ModelError::EmptyBand {
                        sheet: sheet.name.clone(),
                        band_index,
                    });
                }
                if band_index == 0
                    && let Some(gap) = band.gap
                {
                    return Err(ModelError::InvalidFirstBandGap {
                        sheet: sheet.name.clone(),
                        gap,
                    });
                }
                if band.lanes.len() == 1
                    && let Some(lane_gap) = band.lane_gap
                {
                    return Err(ModelError::InvalidSingleLaneGap {
                        sheet: sheet.name.clone(),
                        band_index,
                        lane_gap,
                    });
                }
                if band.page_break_before {
                    if band_index == 0 {
                        return Err(ModelError::InvalidFirstBandPageBreak {
                            sheet: sheet.name.clone(),
                        });
                    }
                    match &sheet.print {
                        None => {
                            return Err(ModelError::PageBreakWithoutPrint {
                                sheet: sheet.name.clone(),
                                band_index,
                            });
                        }
                        Some(print) => match print.fit {
                            PrintFitMode::FitPage => {
                                return Err(ModelError::PageBreakWithFitPage {
                                    sheet: sheet.name.clone(),
                                    band_index,
                                });
                            }
                            PrintFitMode::FitWidth => {
                                page_break_count += 1;
                            }
                        },
                    }
                }

                for (lane_index, lane) in band.lanes.iter().enumerate() {
                    if lane.tables.is_empty() {
                        return Err(ModelError::EmptyLane {
                            sheet: sheet.name.clone(),
                            band_index,
                            lane_index,
                        });
                    }
                    if lane.tables.len() == 1
                        && let Some(table_gap) = lane.table_gap
                    {
                        return Err(ModelError::InvalidSingleTableGap {
                            sheet: sheet.name.clone(),
                            band_index,
                            lane_index,
                            table_gap,
                        });
                    }

                    for table_id in &lane.tables {
                        if let Some(&first_sheet) = seen_tables.get(table_id) {
                            return Err(ModelError::DuplicateSheetTable {
                                table: table_id.clone(),
                                first_sheet: first_sheet.to_string(),
                                second_sheet: sheet.name.clone(),
                            });
                        }
                        seen_tables.insert(table_id, sheet.name.as_str());
                    }
                }
            }

            if page_break_count > MAX_EXCEL_PAGE_BREAKS {
                return Err(ModelError::TooManyPageBreaks {
                    sheet: sheet.name.clone(),
                    count: page_break_count,
                    max: MAX_EXCEL_PAGE_BREAKS,
                });
            }
        }
        Ok(())
    }

    /// Validates worksheet print headers and footers against resolved theme typography and palette.
    ///
    /// # Errors
    /// Returns [`ModelError`] if any header/footer references an invalid theme font or color,
    /// fails zone expansion, or exceeds Excel's 255 UTF-16 code unit limit.
    pub fn validate_print_with_theme(&self, theme: &Theme) -> Result<(), ModelError> {
        for sheet in &self.sheets {
            let Some(print) = &sheet.print else {
                continue;
            };
            if let Some(header) = &print.header {
                compile_header_footer(
                    &sheet.name,
                    HeaderFooterSection::Header,
                    header,
                    &self.model_name,
                    theme,
                )?;
            }
            if let Some(footer) = &print.footer {
                compile_header_footer(
                    &sheet.name,
                    HeaderFooterSection::Footer,
                    footer,
                    &self.model_name,
                    theme,
                )?;
            }
        }
        Ok(())
    }

    /// Resolves and validates the workbook theme from a built-in name or inline custom theme specification.
    ///
    /// # Errors
    /// Returns [`ModelError::ThemeNotFound`] if a named theme is not in the built-in catalog,
    /// or theme validation errors if a custom theme violates palette, metric, or format invariants.
    pub fn resolve_theme(&self) -> Result<Theme, ModelError> {
        match &self.theme {
            ThemeSpec::Name(name) => {
                let trimmed = name.trim();
                let theme_name = if trimmed.is_empty() {
                    Theme::DEFAULT_NAME
                } else {
                    trimmed
                };
                Theme::resolve_builtin(theme_name)
            }
            ThemeSpec::Custom(theme) => {
                theme.validate()?;
                Ok((**theme).clone())
            }
        }
    }
}

/// Freeze panes coordinate configuration for a worksheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FreezePanes {
    pub row: u32,
    pub col: u32,
}

/// Worksheet column layout strategy.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SheetLayoutSpec {
    #[default]
    Auto,
    Grid {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        col_width: Option<f64>,
    },
}

impl<'de> Deserialize<'de> for SheetLayoutSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Serde ignores unknown fields on internally-tagged unit variants even with
        // `deny_unknown_fields`; deserialize via an empty struct variant `Auto {}`
        // so `{"kind": "auto", "col_width": ...}` and other unknown keys fail strictly.
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum SheetLayoutSpecDe {
            Auto {},
            Grid {
                #[serde(default)]
                col_width: Option<f64>,
            },
        }

        match SheetLayoutSpecDe::deserialize(deserializer)? {
            SheetLayoutSpecDe::Auto {} => Ok(Self::Auto),
            SheetLayoutSpecDe::Grid { col_width } => Ok(Self::Grid { col_width }),
        }
    }
}

impl SheetLayoutSpec {
    /// Creates an automatic per-column arbitration sheet layout.
    #[must_use]
    pub const fn auto() -> Self {
        Self::Auto
    }

    /// Creates a uniform grid sheet layout using the theme's `min_col_width` as base column width.
    #[must_use]
    pub const fn grid() -> Self {
        Self::Grid { col_width: None }
    }

    /// Creates a uniform grid sheet layout with an explicit base column width.
    #[must_use]
    pub const fn grid_with_col_width(col_width: f64) -> Self {
        Self::Grid {
            col_width: Some(col_width),
        }
    }

    /// Returns true if this layout is the default `Auto` mode.
    #[must_use]
    pub const fn is_default(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

/// Vertical stack of tables sharing one column lane inside a worksheet band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaneSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table_gap: Option<u32>,
    pub tables: Vec<TableId>,
}

impl LaneSpec {
    /// Creates a lane specification with default vertical table spacing.
    pub fn new(tables: impl IntoIterator<Item = impl Into<TableId>>) -> Self {
        Self {
            table_gap: None,
            tables: tables.into_iter().map(Into::into).collect(),
        }
    }

    /// Builder method setting an explicit vertical row gap between tables in this lane.
    #[must_use]
    pub const fn with_table_gap(mut self, table_gap: u32) -> Self {
        self.table_gap = Some(table_gap);
        self
    }
}

/// Horizontal band of side-by-side column lanes on a worksheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BandSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_gap: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub page_break_before: bool,
    pub lanes: Vec<LaneSpec>,
}

impl BandSpec {
    /// Creates a single-lane, single-table band with default spacing.
    pub fn single(table: impl Into<TableId>) -> Self {
        Self {
            gap: None,
            lane_gap: None,
            page_break_before: false,
            lanes: vec![LaneSpec::new([table])],
        }
    }

    /// Creates a band from an ordered sequence of column lanes.
    pub fn new(lanes: impl IntoIterator<Item = LaneSpec>) -> Self {
        Self {
            gap: None,
            lane_gap: None,
            page_break_before: false,
            lanes: lanes.into_iter().collect(),
        }
    }

    /// Builder method setting an explicit vertical row gap above this band.
    #[must_use]
    pub const fn with_gap(mut self, gap: u32) -> Self {
        self.gap = Some(gap);
        self
    }

    /// Builder method setting an explicit horizontal column gap between lanes in this band.
    #[must_use]
    pub const fn with_lane_gap(mut self, lane_gap: u32) -> Self {
        self.lane_gap = Some(lane_gap);
        self
    }

    /// Builder method enabling or disabling a manual horizontal page break immediately before this band.
    #[must_use]
    pub const fn with_page_break_before(mut self, page_break_before: bool) -> Self {
        self.page_break_before = page_break_before;
        self
    }
}

/// Worksheet container metadata and hierarchical band/lane table layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetSpec {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freeze_panes: Option<FreezePanes>,
    #[serde(default)]
    pub show_gridlines: bool,
    #[serde(default, skip_serializing_if = "SheetLayoutSpec::is_default")]
    pub layout: SheetLayoutSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print: Option<SheetPrintSpec>,
    pub bands: Vec<BandSpec>,
}

impl SheetSpec {
    /// Iterates over all table identifiers on this worksheet in canonical band -> lane -> table order.
    pub fn table_ids(&self) -> impl Iterator<Item = &TableId> {
        self.bands
            .iter()
            .flat_map(|band| band.lanes.iter())
            .flat_map(|lane| lane.tables.iter())
    }

    /// Wraps a sequence of table identifiers into default stacked 1-table bands.
    pub fn stacked_bands(tables: impl IntoIterator<Item = impl Into<TableId>>) -> Vec<BandSpec> {
        tables.into_iter().map(BandSpec::single).collect()
    }
}
