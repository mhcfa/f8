use std::convert::Infallible;
use std::sync::Arc;

use indexmap::IndexMap;

use crate::expr::ast::{BinaryOp, CallArg, CriteriaComparator, Expr, Literal, TableRef, UnaryOp};
use crate::functions::common::{BranchTypeUnifier, value_type_satisfies};
use crate::functions::error::FunctionError;
use crate::functions::lookup::search::{
    MatchMode, SearchMode, validate_homogeneous_slice, validate_lookup_array,
};
use crate::functions::traits::{
    BoundCriterion, BranchPolicy, ComparisonSide, EmptyPolicy, EvalContext, ExcelFunction,
    ExpectationOrigin, FunctionArg, LookupPolicy, ParamKind, ParamType, ParameterDef,
    ReferenceKind, RolePolicy, ShapePolicy, TypeExpectation, TypedRangeGrid, ValidationPolicy,
    VariadicRule,
};
use crate::model::table::AxisRole;
use crate::value::{
    CellValue, DateSerial, ExpectedType, MAX_SAFE_INTEGER, StrictIntError, TypeError, ValueType,
    is_valid_input_date_serial, normalize_zero, validate_text_criterion,
};

/// Error produced during Stage A or Stage B AST expression and slice-shape validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageAError<E> {
    /// Static or dormant function signature, value-type, or slice-shape error.
    Function(FunctionError),
    /// Coordinate resolution error while inspecting a `TableRef`.
    Resolve(E),
}

impl<E> From<FunctionError> for StageAError<E> {
    fn from(err: FunctionError) -> Self {
        Self::Function(err)
    }
}

impl<E> From<TypeError> for StageAError<E> {
    fn from(err: TypeError) -> Self {
        Self::Function(FunctionError::Type(err))
    }
}

/// Resolved shape, column role, and optional memoized runtime state for a [`TableRef`] during AST validation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResolvedRef {
    /// Number of rows in the resolved reference.
    pub(crate) rows: usize,
    /// Number of columns in the resolved reference.
    pub(crate) cols: usize,
    /// Uniform column axis role (`Measure` or `Descriptor`) of the resolved reference.
    pub(crate) axis_role: AxisRole,
    /// Memoized cell values in row-major order (`Some` in Stage B dormant validation; `None` in Stage A).
    pub(crate) values: Option<Vec<CellValue>>,
    /// Per-cell concrete non-`Empty` inferred [`ValueType`]s for `CellValue::Error(_)` cells (`Some` in Stage B; `None` in Stage A).
    pub(crate) error_types: Option<Vec<Option<ValueType>>>,
}

/// Strategy resolving [`TableRef`] coordinates during unified Stage A / Stage B AST validation.
pub(crate) trait ReferenceResolver {
    /// Coordinate resolution error type.
    type Error;

    /// Resolves `table_ref` to its shape, axis role, and optional memoized values, or `Ok(None)` when shape inspection is unavailable.
    fn resolve_ref(&mut self, table_ref: &TableRef) -> Result<Option<ResolvedRef>, Self::Error>;
}

struct ShapeOnlyResolver<F>(F);

impl<E, F> ReferenceResolver for ShapeOnlyResolver<F>
where
    F: FnMut(&TableRef) -> Result<Option<(usize, usize, AxisRole)>, E>,
{
    type Error = E;

    fn resolve_ref(&mut self, table_ref: &TableRef) -> Result<Option<ResolvedRef>, Self::Error> {
        (self.0)(table_ref).map(|opt| {
            opt.map(|(rows, cols, axis_role)| ResolvedRef {
                rows,
                cols,
                axis_role,
                values: None,
                error_types: None,
            })
        })
    }
}

/// Thread-safe registry and dispatcher of built-in and user-defined Excel functions.
#[derive(Clone, Default)]
pub struct FunctionRegistry {
    functions: IndexMap<String, Arc<dyn ExcelFunction>>,
}

impl FunctionRegistry {
    /// Constructs an empty function registry with no functions registered.
    #[must_use]
    pub fn new() -> Self {
        Self {
            functions: IndexMap::new(),
        }
    }

    /// Constructs a standard function registry populated with the 48 built-in Excel functions.
    #[must_use]
    pub fn standard() -> Self {
        let mut registry = Self::new();
        crate::functions::math::register(&mut registry);
        crate::functions::logical::register(&mut registry);
        crate::functions::financial::register(&mut registry);
        crate::functions::lookup::register(&mut registry);
        crate::functions::date::register(&mut registry);
        crate::functions::information::register(&mut registry);
        registry
    }

    /// Registers a function implementation, storing it by canonical uppercase name.
    pub fn register(&mut self, func: Arc<dyn ExcelFunction>) {
        self.functions
            .insert(func.name().to_ascii_uppercase(), func);
    }

    fn get_entry(&self, name: &str) -> Option<&Arc<dyn ExcelFunction>> {
        if let Some(func) = self.functions.get(name) {
            return Some(func);
        }
        if name.bytes().any(|b| b.is_ascii_lowercase()) {
            return self.functions.get(name.to_ascii_uppercase().as_str());
        }
        None
    }

    /// Looks up a function by name (case-insensitive).
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<&dyn ExcelFunction> {
        self.get_entry(name).map(std::convert::AsRef::as_ref)
    }

    /// Looks up a shared reference to a function by name (case-insensitive).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn ExcelFunction>> {
        self.get_entry(name).cloned()
    }

    /// Returns an iterator over all registered functions in deterministic insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Arc<dyn ExcelFunction>)> {
        self.functions.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Returns the number of registered functions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Returns `true` if no functions are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// Validates an invocation signature against registered arity bounds, variadic rules, and positional omission rules.
    pub fn validate_call(
        &self,
        name: &str,
        args: &[CallArg],
    ) -> Result<&dyn ExcelFunction, FunctionError> {
        let func = self
            .lookup(name)
            .ok_or_else(|| FunctionError::UnknownFunction(name.to_string()))?;

        validate_signature(func, args)?;

        Ok(func)
    }

    /// Statically validates function signatures, static types, syntactic scalar-slice exclusions,
    /// and constant index bounds across all active and dormant branches of `expr`.
    pub fn validate_expr(&self, expr: &Expr) -> Result<(), FunctionError> {
        match self.validate_expr_with_shapes(expr, |_| Ok::<_, Infallible>(None)) {
            Ok(()) => Ok(()),
            Err(StageAError::Function(err)) => Err(err),
            Err(StageAError::Resolve(never)) => match never {},
        }
    }

    /// Performs two-pass Stage A static validation over `expr` using `resolve_shape` to inspect `TableRef` dimensions and column axis role.
    ///
    /// - Pass 1 validates function existence, arity, variadic pair parity, and `_` omission across the full AST first.
    /// - Pass 2 validates static value types, discrete integer/domain literals, syntactic scalar-slice exclusion,
    ///   `ParamKind::Range` and `RolePolicy::MeasureOnly` reference constraints, slice dimension/length/rank rules,
    ///   2D `INDEX` arity, and constant `INDEX`/`CHOOSE` bounds.
    pub fn validate_expr_with_shapes<E, F>(
        &self,
        expr: &Expr,
        resolve_shape: F,
    ) -> Result<(), StageAError<E>>
    where
        F: FnMut(&TableRef) -> Result<Option<(usize, usize, AxisRole)>, E>,
    {
        self.validate_signatures_pass1(expr)?;
        let mut resolver = ShapeOnlyResolver(resolve_shape);
        self.validate_ast_with_resolver(expr, None, &mut resolver)?;
        Ok(())
    }

    fn validate_signatures_pass1(&self, expr: &Expr) -> Result<(), FunctionError> {
        match expr {
            Expr::Literal(_) | Expr::Reference(_) => Ok(()),
            Expr::Unary { expr, .. } => self.validate_signatures_pass1(expr),
            Expr::Binary { left, right, .. } => {
                self.validate_signatures_pass1(left)?;
                self.validate_signatures_pass1(right)
            }
            Expr::FunctionCall { name, args } => {
                self.validate_call(name, args)?;
                for arg in args {
                    match arg {
                        CallArg::Expr(arg_expr) | CallArg::Comparator { expr: arg_expr, .. } => {
                            self.validate_signatures_pass1(arg_expr)?;
                        }
                        CallArg::BlankCheck(_) | CallArg::Omitted => {}
                    }
                }
                Ok(())
            }
        }
    }

    /// Unified Stage A / Stage B AST validator parameterized over [`ReferenceResolver`].
    ///
    /// Validates literal, operator, branch, lookup, mode-literal, and slice-shape contracts,
    /// returning the inferred scalar [`ValueType`] of `expr` when statically or runtime-known.
    pub(crate) fn validate_ast_with_resolver<R: ReferenceResolver>(
        &self,
        expr: &Expr,
        expected: Option<TypeExpectation>,
        resolver: &mut R,
    ) -> Result<Option<ValueType>, StageAError<R::Error>> {
        match expr {
            Expr::Literal(lit) => {
                let vt = match lit {
                    Literal::Number(_) => ValueType::Number,
                    Literal::String(_) => ValueType::Text,
                    Literal::Boolean(_) => ValueType::Boolean,
                };
                check_type_expectation(expr, vt, expected)?;
                Ok(Some(vt))
            }

            Expr::Reference(table_ref) => {
                let resolved = resolver
                    .resolve_ref(table_ref)
                    .map_err(StageAError::Resolve)?;
                if table_ref.is_slice() {
                    let (rows, cols) = resolved.as_ref().map_or((0, 0), |r| (r.rows, r.cols));
                    return Err(StageAError::Function(FunctionError::SliceInScalarContext {
                        rows,
                        cols,
                    }));
                }
                let Some(res) = resolved else {
                    return Ok(None);
                };
                if res.rows * res.cols > 1 {
                    return Err(StageAError::Function(FunctionError::SliceInScalarContext {
                        rows: res.rows,
                        cols: res.cols,
                    }));
                }
                let Some(values) = res.values.as_deref() else {
                    return Ok(None);
                };
                match values {
                    [] | [CellValue::Empty] => Ok(None),
                    [CellValue::Error(_)] => {
                        if let Some(err_vt) = res
                            .error_types
                            .as_deref()
                            .and_then(|ets| ets.first().copied().flatten())
                            .filter(|vt| *vt != ValueType::Empty)
                        {
                            check_type_expectation(expr, err_vt, expected)?;
                            Ok(Some(err_vt))
                        } else {
                            Ok(None)
                        }
                    }
                    [val @ (CellValue::Number(_) | CellValue::Text(_) | CellValue::Boolean(_))] => {
                        let vt = match val {
                            CellValue::Number(_) => ValueType::Number,
                            CellValue::Text(_) => ValueType::Text,
                            CellValue::Boolean(_) => ValueType::Boolean,
                            CellValue::Empty | CellValue::Error(_) => return Ok(None),
                        };
                        if let Some(TypeExpectation {
                            expected: ExpectedType::Integer,
                            origin,
                        }) = expected
                        {
                            match val.strict_integer() {
                                Ok(_) => {}
                                Err(StrictIntError::WrongType(found)) => {
                                    return Err(StageAError::Function(FunctionError::Type(
                                        origin.into_invalid_type(ExpectedType::Integer, found),
                                    )));
                                }
                                Err(StrictIntError::NonInteger(value)) => {
                                    return Err(StageAError::Function(FunctionError::Type(
                                        origin.into_non_integer(value),
                                    )));
                                }
                            }
                        } else {
                            check_type_expectation(expr, vt, expected)?;
                        }
                        Ok(Some(vt))
                    }
                    [_, _, ..] => Err(StageAError::Function(FunctionError::SliceInScalarContext {
                        rows: res.rows,
                        cols: res.cols,
                    })),
                }
            }

            Expr::Unary { op, expr: inner } => {
                let child_exp = match op {
                    UnaryOp::Neg => TypeExpectation {
                        expected: ExpectedType::Number,
                        origin: ExpectationOrigin::UnaryOp(op.as_str()),
                    },
                };
                self.validate_ast_with_resolver(inner, Some(child_exp), resolver)?;
                check_type_expectation(expr, ValueType::Number, expected)?;
                Ok(Some(ValueType::Number))
            }

            Expr::Binary { op, left, right } => match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow => {
                    let child_exp = TypeExpectation {
                        expected: ExpectedType::Number,
                        origin: ExpectationOrigin::BinaryOp(op.as_str()),
                    };
                    self.validate_ast_with_resolver(left, Some(child_exp), resolver)?;
                    self.validate_ast_with_resolver(right, Some(child_exp), resolver)?;
                    check_type_expectation(expr, ValueType::Number, expected)?;
                    Ok(Some(ValueType::Number))
                }
                BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::Lte
                | BinaryOp::Gt
                | BinaryOp::Gte => {
                    let left_vt = self.validate_ast_with_resolver(left, None, resolver)?;
                    let right_exp = left_vt.and_then(|l_vt| {
                        ExpectedType::from_value_type(l_vt).map(|exp_ty| TypeExpectation {
                            expected: exp_ty,
                            origin: ExpectationOrigin::ComparisonOp {
                                operator: op.as_str(),
                                other_side: l_vt,
                                side: ComparisonSide::Right,
                            },
                        })
                    });
                    let right_vt = self.validate_ast_with_resolver(right, right_exp, resolver)?;
                    if left_vt.is_none()
                        && let Some(r_vt) = right_vt
                        && let Some(exp_ty) = ExpectedType::from_value_type(r_vt)
                    {
                        let left_exp = TypeExpectation {
                            expected: exp_ty,
                            origin: ExpectationOrigin::ComparisonOp {
                                operator: op.as_str(),
                                other_side: r_vt,
                                side: ComparisonSide::Left,
                            },
                        };
                        self.validate_ast_with_resolver(left, Some(left_exp), resolver)?;
                    }
                    if let (Some(l_vt), Some(r_vt)) = (left_vt, right_vt)
                        && l_vt != r_vt
                    {
                        return Err(StageAError::Function(FunctionError::Type(
                            TypeError::IncompatibleComparison {
                                operator: op.as_str(),
                                left: l_vt,
                                right: r_vt,
                            },
                        )));
                    }
                    check_type_expectation(expr, ValueType::Boolean, expected)?;
                    Ok(Some(ValueType::Boolean))
                }
            },

            Expr::FunctionCall { name, args } => {
                let func = self
                    .lookup(name)
                    .ok_or_else(|| FunctionError::UnknownFunction(name.clone()))?;
                validate_criteria_slots(func, args)?;

                match func.validation_policy() {
                    ValidationPolicy::Standard => self.validate_positional_call(
                        expr,
                        func,
                        args,
                        PositionalPolicy::Standard,
                        expected,
                        resolver,
                    ),
                    ValidationPolicy::ShapeCoupled(shape_policy) => self.validate_positional_call(
                        expr,
                        func,
                        args,
                        PositionalPolicy::ShapeCoupled(shape_policy),
                        expected,
                        resolver,
                    ),
                    ValidationPolicy::CalendarDate => self.validate_positional_call(
                        expr,
                        func,
                        args,
                        PositionalPolicy::CalendarDate,
                        expected,
                        resolver,
                    ),
                    ValidationPolicy::Branch(branch_policy) => self.validate_branch_call(
                        expr,
                        func,
                        args,
                        branch_policy,
                        expected,
                        resolver,
                    ),
                    ValidationPolicy::Lookup(lookup_policy) => self.validate_lookup_call(
                        expr,
                        func,
                        args,
                        lookup_policy,
                        expected,
                        resolver,
                    ),
                }
            }
        }
    }

    fn validate_positional_call<R: ReferenceResolver>(
        &self,
        call_expr: &Expr,
        func: &dyn ExcelFunction,
        args: &[CallArg],
        call_policy: PositionalPolicy,
        expected: Option<TypeExpectation>,
        resolver: &mut R,
    ) -> Result<Option<ValueType>, StageAError<R::Error>> {
        let mut shapes: Vec<Option<(usize, usize)>> = Vec::with_capacity(args.len());
        let mut resolved_refs: Vec<Option<ResolvedRef>> = Vec::with_capacity(args.len());

        // Pass A: Resolve reference shapes/roles and validate `ParamKind::Range` / scalar-slice exclusions.
        for (idx, call_arg) in args.iter().enumerate() {
            let param = resolve_parameter(func, idx)?;
            match call_arg {
                CallArg::Omitted => {
                    shapes.push(None);
                    resolved_refs.push(None);
                }
                CallArg::BlankCheck(_) => {
                    if param.kind != ParamKind::Criteria {
                        return Err(StageAError::Function(
                            FunctionError::UnexpectedCriteriaSyntax {
                                function: func.name(),
                                param: param.name,
                                position: idx + 1,
                            },
                        ));
                    }
                    shapes.push(Some((1, 1)));
                    resolved_refs.push(None);
                }
                CallArg::Comparator { expr: arg_expr, .. } => {
                    if param.kind != ParamKind::Criteria {
                        return Err(StageAError::Function(
                            FunctionError::UnexpectedCriteriaSyntax {
                                function: func.name(),
                                param: param.name,
                                position: idx + 1,
                            },
                        ));
                    }
                    match arg_expr {
                        Expr::Reference(table_ref) => {
                            let resolved = resolver
                                .resolve_ref(table_ref)
                                .map_err(StageAError::Resolve)?;
                            if table_ref.is_slice() {
                                let (rows, cols) =
                                    resolved.as_ref().map_or((0, 0), |r| (r.rows, r.cols));
                                return Err(StageAError::Function(
                                    FunctionError::SliceInScalarContext { rows, cols },
                                ));
                            }
                            if let Some(res) = &resolved
                                && res.rows * res.cols > 1
                            {
                                return Err(StageAError::Function(
                                    FunctionError::SliceInScalarContext {
                                        rows: res.rows,
                                        cols: res.cols,
                                    },
                                ));
                            }
                            shapes.push(Some((1, 1)));
                            resolved_refs.push(resolved);
                        }
                        Expr::Literal(_)
                        | Expr::Unary { .. }
                        | Expr::Binary { .. }
                        | Expr::FunctionCall { .. } => {
                            shapes.push(Some((1, 1)));
                            resolved_refs.push(None);
                        }
                    }
                }
                CallArg::Expr(arg_expr) => match param.kind {
                    ParamKind::Range => match arg_expr {
                        Expr::Reference(table_ref) => {
                            let resolved = resolver
                                .resolve_ref(table_ref)
                                .map_err(StageAError::Resolve)?;
                            shapes.push(resolved.as_ref().map(|r| (r.rows, r.cols)));
                            resolved_refs.push(resolved);
                        }
                        Expr::Literal(_)
                        | Expr::Unary { .. }
                        | Expr::Binary { .. }
                        | Expr::FunctionCall { .. } => {
                            return Err(StageAError::Function(
                                FunctionError::InvalidArgumentKind {
                                    function: func.name(),
                                    expected: "range",
                                    found: "scalar",
                                },
                            ));
                        }
                    },
                    ParamKind::RangeOrArray => match arg_expr {
                        Expr::Reference(table_ref) => {
                            let resolved = resolver
                                .resolve_ref(table_ref)
                                .map_err(StageAError::Resolve)?;
                            shapes.push(resolved.as_ref().map(|r| (r.rows, r.cols)));
                            resolved_refs.push(resolved);
                        }
                        Expr::Literal(_)
                        | Expr::Unary { .. }
                        | Expr::Binary { .. }
                        | Expr::FunctionCall { .. } => {
                            shapes.push(Some((1, 1)));
                            resolved_refs.push(None);
                        }
                    },
                    ParamKind::Scalar | ParamKind::Lazy | ParamKind::Criteria => match arg_expr {
                        Expr::Reference(table_ref) => {
                            let resolved = resolver
                                .resolve_ref(table_ref)
                                .map_err(StageAError::Resolve)?;
                            if table_ref.is_slice() {
                                let (rows, cols) =
                                    resolved.as_ref().map_or((0, 0), |r| (r.rows, r.cols));
                                return Err(StageAError::Function(
                                    FunctionError::SliceInScalarContext { rows, cols },
                                ));
                            }
                            if let Some(res) = &resolved
                                && res.rows * res.cols > 1
                            {
                                return Err(StageAError::Function(
                                    FunctionError::SliceInScalarContext {
                                        rows: res.rows,
                                        cols: res.cols,
                                    },
                                ));
                            }
                            shapes.push(Some((1, 1)));
                            resolved_refs.push(resolved);
                        }
                        Expr::Literal(_)
                        | Expr::Unary { .. }
                        | Expr::Binary { .. }
                        | Expr::FunctionCall { .. } => {
                            shapes.push(Some((1, 1)));
                            resolved_refs.push(None);
                        }
                    },
                },
            }
        }

        // Pass B: Coupled slice-shape invariants run before role and element type/domain checks.
        if let PositionalPolicy::ShapeCoupled(shape_policy) = call_policy {
            validate_shape_policy(func.name(), shape_policy, &shapes)?;
        }

        // Pass C: Declarative `RolePolicy::MeasureOnly` runs before element `ParamType` checks.
        for (idx, res_opt) in resolved_refs.iter().enumerate() {
            let param = resolve_parameter(func, idx)?;
            if param.role_policy == RolePolicy::MeasureOnly
                && let Some(res) = res_opt
                && res.axis_role == AxisRole::Descriptor
            {
                return Err(StageAError::Function(
                    FunctionError::DescriptorReferenceNotAllowed {
                        function: func.name(),
                        param: param.name,
                        position: idx + 1,
                    },
                ));
            }
        }

        // Pass D: Per-argument value-type, subtree, and parameter-boundary domain validation.
        for (idx, call_arg) in args.iter().enumerate() {
            let param = resolve_parameter(func, idx)?;
            let position = idx + 1;
            match param.kind {
                ParamKind::Criteria => {
                    self.validate_criteria_call_arg(
                        func,
                        idx,
                        param,
                        call_arg,
                        &resolved_refs,
                        resolver,
                    )?;
                }
                ParamKind::Range => {
                    if extract_non_criteria_expr(func, param, position, call_arg)?.is_some()
                        && let Some(res) = &resolved_refs[idx]
                    {
                        validate_resolved_range_values(
                            res,
                            func.name(),
                            param.name,
                            position,
                            param.param_type,
                        )?;
                    }
                }
                ParamKind::RangeOrArray => {
                    let Some(arg_expr) =
                        extract_non_criteria_expr(func, param, position, call_arg)?
                    else {
                        continue;
                    };
                    match arg_expr {
                        Expr::Reference(_) => {
                            if let Some(res) = &resolved_refs[idx] {
                                validate_resolved_range_values(
                                    res,
                                    func.name(),
                                    param.name,
                                    position,
                                    param.param_type,
                                )?;
                            }
                        }
                        Expr::Literal(_)
                        | Expr::Unary { .. }
                        | Expr::Binary { .. }
                        | Expr::FunctionCall { .. } => {
                            let arg_exp =
                                param
                                    .param_type
                                    .to_expected_type()
                                    .map(|exp_ty| TypeExpectation {
                                        expected: exp_ty,
                                        origin: ExpectationOrigin::FunctionParam {
                                            function: func.name(),
                                            param: param.name,
                                            position,
                                        },
                                    });
                            self.validate_ast_with_resolver(arg_expr, arg_exp, resolver)?;
                            validate_param_const_domain(
                                arg_expr,
                                func.name(),
                                param.name,
                                position,
                                param.param_type,
                            )?;
                        }
                    }
                }
                ParamKind::Scalar | ParamKind::Lazy => {
                    let Some(arg_expr) =
                        extract_non_criteria_expr(func, param, position, call_arg)?
                    else {
                        continue;
                    };
                    let arg_exp =
                        param
                            .param_type
                            .to_expected_type()
                            .map(|exp_ty| TypeExpectation {
                                expected: exp_ty,
                                origin: ExpectationOrigin::FunctionParam {
                                    function: func.name(),
                                    param: param.name,
                                    position,
                                },
                            });
                    match arg_expr {
                        Expr::Reference(_) => {
                            if let Some(res) = &resolved_refs[idx] {
                                validate_resolved_scalar_ref(
                                    arg_expr,
                                    res,
                                    func.name(),
                                    param.name,
                                    position,
                                    param.param_type,
                                    arg_exp,
                                )?;
                            }
                        }
                        Expr::Literal(_)
                        | Expr::Unary { .. }
                        | Expr::Binary { .. }
                        | Expr::FunctionCall { .. } => {
                            self.validate_ast_with_resolver(arg_expr, arg_exp, resolver)?;
                            validate_param_const_domain(
                                arg_expr,
                                func.name(),
                                param.name,
                                position,
                                param.param_type,
                            )?;
                        }
                    }
                }
            }
        }

        // Pass E: Calendar `(year, month, day)` validation after `ParamType::Integer` checks.
        if call_policy == PositionalPolicy::CalendarDate {
            let year = extract_resolved_calendar_int(
                args.first(),
                resolved_refs.first().and_then(Option::as_ref),
            );
            let month = extract_resolved_calendar_int(
                args.get(1),
                resolved_refs.get(1).and_then(Option::as_ref),
            );
            let day = extract_resolved_calendar_int(
                args.get(2),
                resolved_refs.get(2).and_then(Option::as_ref),
            );
            validate_calendar_date_components(year, month, day)?;
        }

        let ret_vt = func.return_type();
        if let Some(vt) = ret_vt {
            check_type_expectation(call_expr, vt, expected)?;
        }
        Ok(ret_vt)
    }

    fn validate_criteria_call_arg<R: ReferenceResolver>(
        &self,
        func: &dyn ExcelFunction,
        idx: usize,
        param: &ParameterDef,
        call_arg: &CallArg,
        resolved_refs: &[Option<ResolvedRef>],
        resolver: &mut R,
    ) -> Result<(), StageAError<R::Error>> {
        let position = idx + 1;
        let paired_range = if idx > 0 {
            let prev_param = resolve_parameter(func, idx - 1)?;
            if prev_param.kind == ParamKind::Range {
                Some((idx - 1, prev_param.name))
            } else {
                None
            }
        } else {
            None
        };

        let range_vt = if let Some((cr_idx, cr_param_name)) = paired_range
            && let Some(res_cr) = &resolved_refs[cr_idx]
            && let Some(cr_vals) = res_cr.values.as_deref()
        {
            let cr_err_types = res_cr.error_types.as_deref().unwrap_or(&[]);
            validate_homogeneous_slice(
                cr_vals,
                cr_err_types,
                func.name(),
                cr_param_name,
                cr_idx + 1,
            )?
        } else {
            None
        };

        match call_arg {
            CallArg::Omitted | CallArg::BlankCheck(_) => Ok(()),

            CallArg::Comparator {
                op:
                    CriteriaComparator::Lt
                    | CriteriaComparator::Lte
                    | CriteriaComparator::Gt
                    | CriteriaComparator::Gte,
                expr: arg_expr,
            } => {
                if let Some((cr_idx, cr_param_name)) = paired_range
                    && let Some(r_vt) = range_vt
                    && r_vt != ValueType::Number
                {
                    return Err(StageAError::Function(FunctionError::Type(
                        TypeError::InvalidArgumentType {
                            function: func.name(),
                            param: cr_param_name,
                            position: cr_idx + 1,
                            expected: ExpectedType::Number,
                            found: r_vt,
                        },
                    )));
                }
                let arg_exp = Some(TypeExpectation {
                    expected: ExpectedType::Number,
                    origin: ExpectationOrigin::FunctionParam {
                        function: func.name(),
                        param: param.name,
                        position,
                    },
                });
                match arg_expr {
                    Expr::Reference(_) => {
                        if let Some(res) = &resolved_refs[idx] {
                            validate_resolved_scalar_ref(
                                arg_expr,
                                res,
                                func.name(),
                                param.name,
                                position,
                                ParamType::Number,
                                arg_exp,
                            )?;
                        }
                    }
                    Expr::Literal(_)
                    | Expr::Unary { .. }
                    | Expr::Binary { .. }
                    | Expr::FunctionCall { .. } => {
                        self.validate_ast_with_resolver(arg_expr, arg_exp, resolver)?;
                    }
                }
                Ok(())
            }

            CallArg::Expr(arg_expr) => self.validate_equality_criteria_expr(
                (func, idx, param),
                range_vt,
                0,
                arg_expr,
                resolved_refs[idx].as_ref(),
                resolver,
            ),

            CallArg::Comparator {
                op: CriteriaComparator::NotEq,
                expr: arg_expr,
            } => self.validate_equality_criteria_expr(
                (func, idx, param),
                range_vt,
                2,
                arg_expr,
                resolved_refs[idx].as_ref(),
                resolver,
            ),
        }
    }

    fn validate_equality_criteria_expr<R: ReferenceResolver>(
        &self,
        (func, idx, param): (&dyn ExcelFunction, usize, &ParameterDef),
        range_vt: Option<ValueType>,
        prefix_utf16_len: usize,
        arg_expr: &Expr,
        resolved_ref: Option<&ResolvedRef>,
        resolver: &mut R,
    ) -> Result<(), StageAError<R::Error>> {
        let position = idx + 1;
        let expected_ty = range_vt
            .and_then(ExpectedType::from_value_type)
            .unwrap_or(ExpectedType::NonEmpty);
        let arg_exp = Some(TypeExpectation {
            expected: expected_ty,
            origin: ExpectationOrigin::FunctionParam {
                function: func.name(),
                param: param.name,
                position,
            },
        });

        match arg_expr {
            Expr::Reference(_) => {
                if let Some(res) = resolved_ref {
                    validate_resolved_criteria_ref(
                        arg_expr,
                        res,
                        func.name(),
                        param.name,
                        position,
                        prefix_utf16_len,
                        arg_exp,
                    )?;
                }
            }
            Expr::Literal(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::FunctionCall { .. } => {
                self.validate_ast_with_resolver(arg_expr, arg_exp, resolver)?;
                if let Expr::Literal(Literal::String(s)) = arg_expr {
                    validate_text_criterion(s, prefix_utf16_len).map_err(|reason| {
                        StageAError::Function(FunctionError::InvalidTextCriterion {
                            function: func.name(),
                            param: param.name,
                            position,
                            reason,
                        })
                    })?;
                }
            }
        }
        Ok(())
    }

    fn validate_branch_call<R: ReferenceResolver>(
        &self,
        call_expr: &Expr,
        func: &dyn ExcelFunction,
        args: &[CallArg],
        branch_policy: BranchPolicy,
        expected: Option<TypeExpectation>,
        resolver: &mut R,
    ) -> Result<Option<ValueType>, StageAError<R::Error>> {
        let result_indices: Vec<usize> = match branch_policy {
            BranchPolicy::If => {
                if let Some(CallArg::Expr(cond_expr)) = args.first() {
                    let cond_exp = TypeExpectation {
                        expected: ExpectedType::Boolean,
                        origin: ExpectationOrigin::FunctionParam {
                            function: func.name(),
                            param: "logical_test",
                            position: 1,
                        },
                    };
                    self.validate_ast_with_resolver(cond_expr, Some(cond_exp), resolver)?;
                }
                vec![1, 2]
            }
            BranchPolicy::Ifs => {
                for idx in (0..args.len()).step_by(2) {
                    if let Some(CallArg::Expr(cond_expr)) = args.get(idx) {
                        let cond_exp = TypeExpectation {
                            expected: ExpectedType::Boolean,
                            origin: ExpectationOrigin::FunctionParam {
                                function: func.name(),
                                param: "logical_test1",
                                position: idx + 1,
                            },
                        };
                        self.validate_ast_with_resolver(cond_expr, Some(cond_exp), resolver)?;
                    }
                }
                (1..args.len()).step_by(2).collect()
            }
            BranchPolicy::Choose => {
                if let Some(CallArg::Expr(idx_expr)) = args.first() {
                    let idx_exp = TypeExpectation {
                        expected: ExpectedType::Integer,
                        origin: ExpectationOrigin::FunctionParam {
                            function: func.name(),
                            param: "index_num",
                            position: 1,
                        },
                    };
                    self.validate_ast_with_resolver(idx_expr, Some(idx_exp), resolver)?;
                    let choices_len = args.len().saturating_sub(1);
                    if choices_len > 0
                        && let Some(idx) = extract_const_integer_literal(idx_expr)
                        && !index_in_bounds(idx, choices_len)
                    {
                        return Err(StageAError::Function(FunctionError::IndexOutOfBounds {
                            function: func.name(),
                            param: "index_num",
                            index: idx,
                            min: 1,
                            max: choices_len,
                        }));
                    }
                }
                (1..args.len()).collect()
            }
            BranchPolicy::ErrorTrap => vec![0, 1],
        };

        let mut unifier = BranchTypeUnifier::new(func.name());
        for idx in result_indices {
            if let Some(CallArg::Expr(arm_expr)) = args.get(idx) {
                let arm_exp = match branch_policy {
                    BranchPolicy::ErrorTrap => {
                        let param = resolve_parameter(func, idx)?;
                        Some(TypeExpectation {
                            expected: ExpectedType::NonEmpty,
                            origin: ExpectationOrigin::FunctionParam {
                                function: func.name(),
                                param: param.name,
                                position: idx + 1,
                            },
                        })
                    }
                    BranchPolicy::If | BranchPolicy::Ifs | BranchPolicy::Choose => {
                        unifier.unified().and_then(|u_vt| {
                            ExpectedType::from_value_type(u_vt).map(|exp_ty| TypeExpectation {
                                expected: exp_ty,
                                origin: ExpectationOrigin::BranchResult {
                                    function: func.name(),
                                    position: idx + 1,
                                },
                            })
                        })
                    }
                };
                let arm_vt = self.validate_ast_with_resolver(arm_expr, arm_exp, resolver)?;
                unifier.record(idx + 1, arm_vt)?;
            }
        }

        let unified_vt = unifier.unified();
        if let Some(vt) = unified_vt {
            check_type_expectation(call_expr, vt, expected)?;
        }
        Ok(unified_vt)
    }

    fn validate_lookup_call<R: ReferenceResolver>(
        &self,
        call_expr: &Expr,
        func: &dyn ExcelFunction,
        args: &[CallArg],
        lookup_policy: LookupPolicy,
        expected: Option<TypeExpectation>,
        resolver: &mut R,
    ) -> Result<Option<ValueType>, StageAError<R::Error>> {
        match lookup_policy {
            LookupPolicy::Index => {
                let Some(CallArg::Expr(Expr::Reference(arr_ref))) = args.first() else {
                    return Err(StageAError::Function(FunctionError::InvalidArgumentKind {
                        function: func.name(),
                        expected: "range",
                        found: "scalar",
                    }));
                };
                let resolved_arr = resolver
                    .resolve_ref(arr_ref)
                    .map_err(StageAError::Resolve)?;

                if let Some(CallArg::Expr(row_expr)) = args.get(1) {
                    let row_exp = TypeExpectation {
                        expected: ExpectedType::Integer,
                        origin: ExpectationOrigin::FunctionParam {
                            function: func.name(),
                            param: "row_num",
                            position: 2,
                        },
                    };
                    self.validate_ast_with_resolver(row_expr, Some(row_exp), resolver)?;
                }

                if let Some(CallArg::Expr(col_expr)) = args.get(2) {
                    let col_exp = TypeExpectation {
                        expected: ExpectedType::Integer,
                        origin: ExpectationOrigin::FunctionParam {
                            function: func.name(),
                            param: "col_num",
                            position: 3,
                        },
                    };
                    self.validate_ast_with_resolver(col_expr, Some(col_exp), resolver)?;
                }

                if let Some(res) = &resolved_arr
                    && res.rows > 0
                    && res.cols > 0
                {
                    validate_index_bounds(func.name(), args, res.rows, res.cols)?;
                }

                if let Some(res) = &resolved_arr
                    && let Some(values) = res.values.as_deref()
                {
                    let error_types = res.error_types.as_deref().unwrap_or(&[]);
                    let arr_vt =
                        validate_homogeneous_slice(values, error_types, func.name(), "array", 1)?;
                    if let Some(vt) = arr_vt {
                        check_type_expectation(call_expr, vt, expected)?;
                    }
                    Ok(arr_vt)
                } else {
                    Ok(None)
                }
            }

            LookupPolicy::Xlookup => {
                let lookup_vt = if let Some(CallArg::Expr(lv_expr)) = args.first() {
                    let lv_exp = TypeExpectation {
                        expected: ExpectedType::NonEmpty,
                        origin: ExpectationOrigin::FunctionParam {
                            function: func.name(),
                            param: "lookup_value",
                            position: 1,
                        },
                    };
                    self.validate_ast_with_resolver(lv_expr, Some(lv_exp), resolver)?
                } else {
                    None
                };

                let Some(CallArg::Expr(Expr::Reference(l_ref))) = args.get(1) else {
                    return Err(StageAError::Function(FunctionError::InvalidArgumentKind {
                        function: func.name(),
                        expected: "range",
                        found: "scalar",
                    }));
                };
                let Some(CallArg::Expr(Expr::Reference(r_ref))) = args.get(2) else {
                    return Err(StageAError::Function(FunctionError::InvalidArgumentKind {
                        function: func.name(),
                        expected: "range",
                        found: "scalar",
                    }));
                };

                let resolved_l = resolver.resolve_ref(l_ref).map_err(StageAError::Resolve)?;
                let resolved_r = resolver.resolve_ref(r_ref).map_err(StageAError::Resolve)?;

                if let Some(CallArg::Expr(mm_expr)) = args.get(4) {
                    let mm_exp = TypeExpectation {
                        expected: ExpectedType::Integer,
                        origin: ExpectationOrigin::FunctionParam {
                            function: func.name(),
                            param: "match_mode",
                            position: 5,
                        },
                    };
                    self.validate_ast_with_resolver(mm_expr, Some(mm_exp), resolver)?;
                    validate_match_mode_literal(func.name(), "match_mode", 5, mm_expr)?;
                }

                let search_mode = if let Some(CallArg::Expr(sm_expr)) = args.get(5) {
                    let sm_exp = TypeExpectation {
                        expected: ExpectedType::Integer,
                        origin: ExpectationOrigin::FunctionParam {
                            function: func.name(),
                            param: "search_mode",
                            position: 6,
                        },
                    };
                    self.validate_ast_with_resolver(sm_expr, Some(sm_exp), resolver)?;
                    parse_search_mode_literal(func.name(), "search_mode", 6, sm_expr)?
                } else {
                    SearchMode::FirstToLast
                };

                let l_shape = resolved_l.as_ref().map(|r| (r.rows, r.cols));
                let r_shape = resolved_r.as_ref().map(|r| (r.rows, r.cols));
                require_1d_slice(func.name(), "lookup_array", l_shape)?;
                require_1d_slice(func.name(), "return_array", r_shape)?;
                if let (Some((lr, lc)), Some((rr, rc))) = (l_shape, r_shape)
                    && (lr, lc) != (rr, rc)
                {
                    return Err(StageAError::Function(FunctionError::DimensionMismatch {
                        function: func.name(),
                        expected_rows: lr,
                        expected_cols: lc,
                        found_rows: rr,
                        found_cols: rc,
                    }));
                }

                if let Some(res_l) = &resolved_l
                    && let Some(l_vals) = res_l.values.as_deref()
                {
                    let l_err_types = res_l.error_types.as_deref().unwrap_or(&[]);
                    validate_lookup_array(
                        l_vals,
                        l_err_types,
                        lookup_vt,
                        search_mode,
                        func.name(),
                    )?;
                }

                let ret_vt = match &resolved_r {
                    Some(res_r) => match res_r.values.as_deref() {
                        Some(r_vals) => {
                            let r_err_types = res_r.error_types.as_deref().unwrap_or(&[]);
                            validate_homogeneous_slice(
                                r_vals,
                                r_err_types,
                                func.name(),
                                "return_array",
                                3,
                            )?
                        }
                        None => None,
                    },
                    None => None,
                };
                let mut unifier = BranchTypeUnifier::with_initial(func.name(), ret_vt);

                if let Some(CallArg::Expr(nf_expr)) = args.get(3) {
                    let nf_exp = unifier.unified().and_then(|u_vt| {
                        ExpectedType::from_value_type(u_vt).map(|exp_ty| TypeExpectation {
                            expected: exp_ty,
                            origin: ExpectationOrigin::BranchResult {
                                function: func.name(),
                                position: 4,
                            },
                        })
                    });
                    let nf_vt = self.validate_ast_with_resolver(nf_expr, nf_exp, resolver)?;
                    unifier.record(4, nf_vt)?;
                }

                let unified_vt = unifier.unified();
                if let Some(vt) = unified_vt {
                    check_type_expectation(call_expr, vt, expected)?;
                }
                Ok(unified_vt)
            }

            LookupPolicy::Xmatch => {
                let lookup_vt = if let Some(CallArg::Expr(lv_expr)) = args.first() {
                    let lv_exp = TypeExpectation {
                        expected: ExpectedType::NonEmpty,
                        origin: ExpectationOrigin::FunctionParam {
                            function: func.name(),
                            param: "lookup_value",
                            position: 1,
                        },
                    };
                    self.validate_ast_with_resolver(lv_expr, Some(lv_exp), resolver)?
                } else {
                    None
                };

                let Some(CallArg::Expr(Expr::Reference(l_ref))) = args.get(1) else {
                    return Err(StageAError::Function(FunctionError::InvalidArgumentKind {
                        function: func.name(),
                        expected: "range",
                        found: "scalar",
                    }));
                };
                let resolved_l = resolver.resolve_ref(l_ref).map_err(StageAError::Resolve)?;

                if let Some(CallArg::Expr(mm_expr)) = args.get(2) {
                    let mm_exp = TypeExpectation {
                        expected: ExpectedType::Integer,
                        origin: ExpectationOrigin::FunctionParam {
                            function: func.name(),
                            param: "match_mode",
                            position: 3,
                        },
                    };
                    self.validate_ast_with_resolver(mm_expr, Some(mm_exp), resolver)?;
                    validate_match_mode_literal(func.name(), "match_mode", 3, mm_expr)?;
                }

                let search_mode = if let Some(CallArg::Expr(sm_expr)) = args.get(3) {
                    let sm_exp = TypeExpectation {
                        expected: ExpectedType::Integer,
                        origin: ExpectationOrigin::FunctionParam {
                            function: func.name(),
                            param: "search_mode",
                            position: 4,
                        },
                    };
                    self.validate_ast_with_resolver(sm_expr, Some(sm_exp), resolver)?;
                    parse_search_mode_literal(func.name(), "search_mode", 4, sm_expr)?
                } else {
                    SearchMode::FirstToLast
                };

                let l_shape = resolved_l.as_ref().map(|r| (r.rows, r.cols));
                require_1d_slice(func.name(), "lookup_array", l_shape)?;

                if let Some(res_l) = &resolved_l
                    && let Some(l_vals) = res_l.values.as_deref()
                {
                    let l_err_types = res_l.error_types.as_deref().unwrap_or(&[]);
                    validate_lookup_array(
                        l_vals,
                        l_err_types,
                        lookup_vt,
                        search_mode,
                        func.name(),
                    )?;
                }

                check_type_expectation(call_expr, ValueType::Number, expected)?;
                Ok(Some(ValueType::Number))
            }
        }
    }

    /// Binds parsed formula arguments 1-to-1 into runtime `FunctionArg` values, substituting defaults for intermediate omitted optional parameters.
    pub fn bind_arguments<'a>(
        &self,
        func: &dyn ExcelFunction,
        expr_args: &'a [CallArg],
        ctx: &dyn EvalContext,
    ) -> Result<Vec<FunctionArg<'a>>, FunctionError> {
        validate_signature(func, expr_args)?;

        let mut bound = Vec::with_capacity(expr_args.len());

        for (i, call_arg) in expr_args.iter().enumerate() {
            let param = resolve_parameter(func, i)?;
            let position = i + 1;

            let param_exp = param
                .param_type
                .to_expected_type()
                .map(|exp_ty| TypeExpectation {
                    expected: exp_ty,
                    origin: ExpectationOrigin::FunctionParam {
                        function: func.name(),
                        param: param.name,
                        position,
                    },
                });

            match param.kind {
                ParamKind::Criteria => {
                    bound.push(bind_criteria_arg(param, call_arg, ctx)?);
                }
                ParamKind::Lazy => {
                    let Some(arg_expr) =
                        extract_non_criteria_expr(func, param, position, call_arg)?
                    else {
                        bound.push(bind_omitted_non_criteria(param));
                        continue;
                    };
                    bound.push(FunctionArg::Lazy(arg_expr));
                }
                ParamKind::Scalar => {
                    let Some(arg_expr) =
                        extract_non_criteria_expr(func, param, position, call_arg)?
                    else {
                        bound.push(bind_omitted_non_criteria(param));
                        continue;
                    };
                    let val = ctx.eval_expr(arg_expr)?;
                    let mut inferred_err_vt: Option<ValueType> = None;
                    if let Some(mode_slot) = lookup_mode_slot(func.validation_policy(), i) {
                        validate_value_against_param_type(
                            &val,
                            func.name(),
                            param.name,
                            position,
                            ParamType::Integer,
                            EmptyValidation::RejectEmpty,
                        )?;
                        if val.is_error() {
                            inferred_err_vt = ctx.validate_dormant_expr(arg_expr, param_exp)?;
                        }
                        match mode_slot {
                            LookupModeSlot::MatchMode { param, position } => {
                                validate_match_mode_literal(
                                    func.name(),
                                    param,
                                    position,
                                    arg_expr,
                                )?;
                            }
                            LookupModeSlot::SearchMode { param, position } => {
                                parse_search_mode_literal(func.name(), param, position, arg_expr)?;
                            }
                        }
                    } else if val.is_error() {
                        inferred_err_vt = ctx.validate_dormant_expr(arg_expr, param_exp)?;
                    }
                    bound.push(bind_scalar_value(val, inferred_err_vt));
                }
                ParamKind::Range => {
                    let Some(arg_expr) =
                        extract_non_criteria_expr(func, param, position, call_arg)?
                    else {
                        bound.push(bind_omitted_non_criteria(param));
                        continue;
                    };
                    match arg_expr {
                        Expr::Reference(table_ref) => {
                            let TypedRangeGrid {
                                rows,
                                cols,
                                values,
                                error_types,
                                axis_role,
                            } = ctx.eval_typed_range(arg_expr)?;
                            let ref_kind = if table_ref.is_slice() {
                                ReferenceKind::Slice
                            } else {
                                ReferenceKind::SingleCell
                            };
                            bound.push(FunctionArg::Array {
                                rows,
                                cols,
                                values,
                                error_types,
                                ref_kind,
                                axis_role,
                            });
                        }
                        Expr::Literal(_)
                        | Expr::Unary { .. }
                        | Expr::Binary { .. }
                        | Expr::FunctionCall { .. } => {
                            return Err(FunctionError::InvalidArgumentKind {
                                function: func.name(),
                                expected: "range",
                                found: "scalar",
                            });
                        }
                    }
                }
                ParamKind::RangeOrArray => {
                    let Some(arg_expr) =
                        extract_non_criteria_expr(func, param, position, call_arg)?
                    else {
                        bound.push(bind_omitted_non_criteria(param));
                        continue;
                    };
                    match arg_expr {
                        Expr::Reference(table_ref) => {
                            let TypedRangeGrid {
                                rows,
                                cols,
                                values,
                                error_types,
                                axis_role,
                            } = ctx.eval_typed_range(arg_expr)?;
                            let ref_kind = if table_ref.is_slice() {
                                ReferenceKind::Slice
                            } else {
                                ReferenceKind::SingleCell
                            };
                            bound.push(FunctionArg::Array {
                                rows,
                                cols,
                                values,
                                error_types,
                                ref_kind,
                                axis_role,
                            });
                        }
                        Expr::Literal(_)
                        | Expr::Unary { .. }
                        | Expr::Binary { .. }
                        | Expr::FunctionCall { .. } => {
                            let val = ctx.eval_expr(arg_expr)?;
                            let inferred_err_vt = if val.is_error() {
                                ctx.validate_dormant_expr(arg_expr, param_exp)?
                            } else {
                                None
                            };
                            bound.push(bind_scalar_value(val, inferred_err_vt));
                        }
                    }
                }
            }
        }

        Ok(bound)
    }
}

fn extract_non_criteria_expr<'a>(
    func: &dyn ExcelFunction,
    param: &ParameterDef,
    position: usize,
    call_arg: &'a CallArg,
) -> Result<Option<&'a Expr>, FunctionError> {
    match call_arg {
        CallArg::Expr(expr) => Ok(Some(expr)),
        CallArg::Omitted => Ok(None),
        CallArg::Comparator { .. } | CallArg::BlankCheck(_) => {
            Err(FunctionError::UnexpectedCriteriaSyntax {
                function: func.name(),
                param: param.name,
                position,
            })
        }
    }
}

fn bind_omitted_non_criteria<'a>(param: &ParameterDef) -> FunctionArg<'a> {
    if let Some(default_val) = param.requirement.default_value() {
        FunctionArg::Scalar(default_val.clone())
    } else {
        FunctionArg::Omitted
    }
}

fn bind_criteria_arg<'a>(
    param: &ParameterDef,
    call_arg: &CallArg,
    ctx: &dyn EvalContext,
) -> Result<FunctionArg<'a>, FunctionError> {
    match call_arg {
        CallArg::Omitted => {
            if let Some(default_val) = param.requirement.default_value() {
                Ok(FunctionArg::Criterion(BoundCriterion::Exact(
                    default_val.clone(),
                )))
            } else {
                Ok(FunctionArg::Omitted)
            }
        }
        CallArg::BlankCheck(check) => {
            Ok(FunctionArg::Criterion(BoundCriterion::BlankCheck(*check)))
        }
        CallArg::Expr(arg_expr) => {
            let val = ctx.eval_expr(arg_expr)?;
            if let CellValue::Error(error) = val {
                let inferred_type = ctx
                    .validate_dormant_expr(arg_expr, None)?
                    .filter(|vt| *vt != ValueType::Empty);
                Ok(FunctionArg::Criterion(BoundCriterion::Errored {
                    op: None,
                    error,
                    inferred_type,
                }))
            } else {
                Ok(FunctionArg::Criterion(BoundCriterion::Exact(val)))
            }
        }
        CallArg::Comparator { op, expr: arg_expr } => {
            let val = ctx.eval_expr(arg_expr)?;
            if let CellValue::Error(error) = val {
                let inferred_type = ctx
                    .validate_dormant_expr(arg_expr, None)?
                    .filter(|vt| *vt != ValueType::Empty);
                Ok(FunctionArg::Criterion(BoundCriterion::Errored {
                    op: Some(*op),
                    error,
                    inferred_type,
                }))
            } else {
                Ok(FunctionArg::Criterion(BoundCriterion::Comparator {
                    op: *op,
                    value: val,
                }))
            }
        }
    }
}

fn bind_scalar_value<'a>(val: CellValue, inferred_vt: Option<ValueType>) -> FunctionArg<'a> {
    if let CellValue::Error(error) = val
        && let Some(vt @ (ValueType::Number | ValueType::Text | ValueType::Boolean)) = inferred_vt
    {
        FunctionArg::TypedErrorScalar {
            error,
            inferred_type: vt,
        }
    } else {
        FunctionArg::Scalar(val)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionalPolicy {
    Standard,
    ShapeCoupled(ShapePolicy),
    CalendarDate,
}

fn validate_resolved_scalar_ref(
    arg_expr: &Expr,
    res: &ResolvedRef,
    function: &'static str,
    param: &'static str,
    position: usize,
    param_type: ParamType,
    expected: Option<TypeExpectation>,
) -> Result<(), FunctionError> {
    let Some(values) = res.values.as_deref() else {
        return Ok(());
    };
    match values {
        [] | [CellValue::Empty] => Ok(()),
        [CellValue::Error(_)] => {
            if let Some(err_vt) = res
                .error_types
                .as_deref()
                .and_then(|ets| ets.first().copied().flatten())
                .filter(|vt| *vt != ValueType::Empty)
            {
                check_type_expectation(arg_expr, err_vt, expected)?;
            }
            Ok(())
        }
        [val @ (CellValue::Number(_) | CellValue::Text(_) | CellValue::Boolean(_))] => {
            validate_value_against_param_type(
                val,
                function,
                param,
                position,
                param_type,
                EmptyValidation::SkipEmpty,
            )
        }
        [_, _, ..] => Err(FunctionError::SliceInScalarContext {
            rows: res.rows,
            cols: res.cols,
        }),
    }
}

fn validate_resolved_criteria_ref(
    arg_expr: &Expr,
    res: &ResolvedRef,
    function: &'static str,
    param: &'static str,
    position: usize,
    prefix_utf16_len: usize,
    expected: Option<TypeExpectation>,
) -> Result<(), FunctionError> {
    let Some(values) = res.values.as_deref() else {
        return Ok(());
    };
    match values {
        [] | [CellValue::Empty] => Ok(()),
        [CellValue::Error(_)] => {
            if let Some(err_vt) = res
                .error_types
                .as_deref()
                .and_then(|ets| ets.first().copied().flatten())
                .filter(|vt| *vt != ValueType::Empty)
            {
                check_type_expectation(arg_expr, err_vt, expected)?;
            }
            Ok(())
        }
        [val @ (CellValue::Number(_) | CellValue::Text(_) | CellValue::Boolean(_))] => {
            let vt = match val {
                CellValue::Number(_) => ValueType::Number,
                CellValue::Text(_) => ValueType::Text,
                CellValue::Boolean(_) => ValueType::Boolean,
                CellValue::Empty | CellValue::Error(_) => return Ok(()),
            };
            check_type_expectation(arg_expr, vt, expected)?;
            if let CellValue::Text(s) = val {
                validate_text_criterion(s, prefix_utf16_len).map_err(|reason| {
                    FunctionError::InvalidTextCriterion {
                        function,
                        param,
                        position,
                        reason,
                    }
                })?;
            }
            Ok(())
        }
        [_, _, ..] => Err(FunctionError::SliceInScalarContext {
            rows: res.rows,
            cols: res.cols,
        }),
    }
}

fn validate_param_const_domain(
    arg_expr: &Expr,
    function: &'static str,
    param: &'static str,
    position: usize,
    param_type: ParamType,
) -> Result<(), FunctionError> {
    match param_type {
        ParamType::Any
        | ParamType::NonEmpty
        | ParamType::Number
        | ParamType::Integer
        | ParamType::Boolean => Ok(()),
        ParamType::DateSerial => {
            if let Some(raw) = extract_signed_const_number(arg_expr)
                && raw.is_finite()
            {
                let serial = normalize_zero(raw);
                if !is_valid_input_date_serial(serial) {
                    return Err(FunctionError::InvalidDateSerial {
                        function,
                        param,
                        position,
                        serial: serial.to_string(),
                    });
                }
            }
            Ok(())
        }
        ParamType::AnnuityType => {
            if let Some(iv) = extract_signed_const_integer(arg_expr)
                && !(0..=1).contains(&iv)
            {
                return Err(FunctionError::InvalidAnnuityType {
                    function,
                    found: iv,
                });
            }
            Ok(())
        }
        ParamType::DayCountBasis => {
            if let Some(iv) = extract_signed_const_integer(arg_expr)
                && !(0..=4).contains(&iv)
            {
                return Err(FunctionError::InvalidDayCountBasis { found: iv });
            }
            Ok(())
        }
    }
}

fn extract_resolved_calendar_int(
    call_arg: Option<&CallArg>,
    resolved_ref: Option<&ResolvedRef>,
) -> Option<i64> {
    match call_arg {
        Some(CallArg::Expr(arg_expr)) => {
            if let Some(iv) = extract_signed_const_integer(arg_expr) {
                return Some(iv);
            }
            if let Some(res) = resolved_ref
                && let Some([val]) = res.values.as_deref()
                && let Ok(Ok(iv)) = val.strict_integer()
            {
                return Some(iv);
            }
            None
        }
        Some(CallArg::Comparator { .. } | CallArg::BlankCheck(_) | CallArg::Omitted) | None => None,
    }
}

fn extract_bound_calendar_int(arg: Option<&FunctionArg<'_>>) -> Option<i64> {
    match arg {
        Some(FunctionArg::Scalar(val)) => match val.strict_integer() {
            Ok(Ok(iv)) => Some(iv),
            Ok(Err(_)) | Err(_) => None,
        },
        Some(
            FunctionArg::TypedErrorScalar { .. }
            | FunctionArg::Array { .. }
            | FunctionArg::Lazy(_)
            | FunctionArg::Criterion(_)
            | FunctionArg::Omitted,
        )
        | None => None,
    }
}

fn validate_calendar_date_components(
    year: Option<i64>,
    month: Option<i64>,
    day: Option<i64>,
) -> Result<(), FunctionError> {
    if let Some(y) = year
        && !(1900..=9999).contains(&y)
    {
        return Err(FunctionError::InvalidDateYear { year: y });
    }
    if let Some(m) = month
        && !(1..=12).contains(&m)
    {
        return Err(FunctionError::InvalidDateMonth { month: m });
    }
    if let Some(d) = day {
        let max_day = match (year, month) {
            (Some(y), Some(m)) => DateSerial::days_in_month(y as i32, m as u32),
            (None, Some(m)) => DateSerial::days_in_month(2000, m as u32),
            (_, None) => 31,
        };
        if d < 1 || d > i64::from(max_day) {
            return Err(FunctionError::InvalidDateDay { day: d, max_day });
        }
    }
    Ok(())
}

fn validate_resolved_range_values(
    res: &ResolvedRef,
    function: &'static str,
    param: &'static str,
    position: usize,
    param_type: ParamType,
) -> Result<(), FunctionError> {
    let Some(values) = res.values.as_deref() else {
        return Ok(());
    };
    let error_types = res.error_types.as_deref().unwrap_or(&[]);
    for (idx, val) in values.iter().enumerate() {
        if matches!(val, CellValue::Error(_)) {
            if let Some(err_vt) = error_types.get(idx).copied().flatten()
                && err_vt != ValueType::Empty
                && let Some(expected) = param_type.to_expected_type()
                && !value_type_satisfies(err_vt, expected)
            {
                return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function,
                    param,
                    position,
                    expected,
                    found: err_vt,
                }));
            }
            continue;
        }
        validate_value_against_param_type(
            val,
            function,
            param,
            position,
            param_type,
            EmptyValidation::SkipEmpty,
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LookupModeSlot {
    MatchMode {
        param: &'static str,
        position: usize,
    },
    SearchMode {
        param: &'static str,
        position: usize,
    },
}

const fn lookup_mode_slot(policy: ValidationPolicy, arg_idx: usize) -> Option<LookupModeSlot> {
    match policy {
        ValidationPolicy::Lookup(LookupPolicy::Xlookup) => {
            if arg_idx == 4 {
                Some(LookupModeSlot::MatchMode {
                    param: "match_mode",
                    position: 5,
                })
            } else if arg_idx == 5 {
                Some(LookupModeSlot::SearchMode {
                    param: "search_mode",
                    position: 6,
                })
            } else {
                None
            }
        }
        ValidationPolicy::Lookup(LookupPolicy::Xmatch) => {
            if arg_idx == 2 {
                Some(LookupModeSlot::MatchMode {
                    param: "match_mode",
                    position: 3,
                })
            } else if arg_idx == 3 {
                Some(LookupModeSlot::SearchMode {
                    param: "search_mode",
                    position: 4,
                })
            } else {
                None
            }
        }
        ValidationPolicy::Lookup(LookupPolicy::Index)
        | ValidationPolicy::Standard
        | ValidationPolicy::ShapeCoupled(_)
        | ValidationPolicy::Branch(_)
        | ValidationPolicy::CalendarDate => None,
    }
}

fn validate_match_mode_literal(
    function: &'static str,
    param: &'static str,
    position: usize,
    arg_expr: &Expr,
) -> Result<MatchMode, FunctionError> {
    let mode =
        extract_const_integer_literal(arg_expr).ok_or(FunctionError::NonLiteralLookupMode {
            function,
            param,
            position,
        })?;
    MatchMode::from_mode_int(mode, function, param, position)
}

fn parse_search_mode_literal(
    function: &'static str,
    param: &'static str,
    position: usize,
    arg_expr: &Expr,
) -> Result<SearchMode, FunctionError> {
    let mode =
        extract_const_integer_literal(arg_expr).ok_or(FunctionError::NonLiteralLookupMode {
            function,
            param,
            position,
        })?;
    SearchMode::from_mode_int(mode, function, param, position)
}

fn extract_const_number_literal(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Literal(Literal::Number(n)) => Some(*n),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr: inner,
        } => match inner.as_ref() {
            Expr::Literal(Literal::Number(n)) => Some(-*n),
            Expr::Literal(Literal::String(_) | Literal::Boolean(_))
            | Expr::Reference(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::FunctionCall { .. } => None,
        },
        Expr::Literal(Literal::String(_) | Literal::Boolean(_))
        | Expr::Reference(_)
        | Expr::Binary { .. }
        | Expr::FunctionCall { .. } => None,
    }
}

fn extract_signed_const_number(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Literal(Literal::Number(n)) => Some(*n),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr: inner,
        } => extract_signed_const_number(inner).map(|n| -n),
        Expr::Literal(Literal::String(_) | Literal::Boolean(_))
        | Expr::Reference(_)
        | Expr::Binary { .. }
        | Expr::FunctionCall { .. } => None,
    }
}

fn extract_signed_const_integer(expr: &Expr) -> Option<i64> {
    let raw = extract_signed_const_number(expr)?;
    if !raw.is_finite() {
        return None;
    }
    let norm = normalize_zero(raw);
    if norm.fract() != 0.0 || !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&norm) {
        return None;
    }
    Some(norm as i64)
}

fn extract_const_integer_literal(expr: &Expr) -> Option<i64> {
    let raw = extract_const_number_literal(expr)?;
    if !raw.is_finite() {
        return None;
    }
    let norm = normalize_zero(raw);
    if norm.fract() != 0.0 || !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&norm) {
        return None;
    }
    Some(norm as i64)
}

fn check_type_expectation(
    expr: &Expr,
    found: ValueType,
    expected: Option<TypeExpectation>,
) -> Result<(), FunctionError> {
    let Some(TypeExpectation { expected, origin }) = expected else {
        return Ok(());
    };

    match expected {
        ExpectedType::NonEmpty => match found {
            ValueType::Number | ValueType::Text | ValueType::Boolean => Ok(()),
            ValueType::Empty => Err(FunctionError::Type(
                origin.into_invalid_type(expected, found),
            )),
        },
        ExpectedType::Number => match found {
            ValueType::Number => Ok(()),
            ValueType::Text | ValueType::Boolean | ValueType::Empty => Err(FunctionError::Type(
                origin.into_invalid_type(expected, found),
            )),
        },
        ExpectedType::Text => match found {
            ValueType::Text => Ok(()),
            ValueType::Number | ValueType::Boolean | ValueType::Empty => Err(FunctionError::Type(
                origin.into_invalid_type(expected, found),
            )),
        },
        ExpectedType::Boolean => match found {
            ValueType::Boolean => Ok(()),
            ValueType::Number | ValueType::Text | ValueType::Empty => Err(FunctionError::Type(
                origin.into_invalid_type(expected, found),
            )),
        },
        ExpectedType::Integer => match found {
            ValueType::Number => {
                if let Some(raw) = extract_signed_const_number(expr)
                    && raw.is_finite()
                {
                    let norm = normalize_zero(raw);
                    if norm.fract() != 0.0
                        || !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&norm)
                    {
                        return Err(FunctionError::Type(
                            origin.into_non_integer(norm.to_string()),
                        ));
                    }
                }
                Ok(())
            }
            ValueType::Text | ValueType::Boolean | ValueType::Empty => Err(FunctionError::Type(
                origin.into_invalid_type(expected, found),
            )),
        },
    }
}

fn require_1d_slice(
    function: &'static str,
    param: &'static str,
    shape: Option<(usize, usize)>,
) -> Result<(), FunctionError> {
    if let Some((rows, cols)) = shape
        && rows > 1
        && cols > 1
    {
        return Err(FunctionError::ExpectedOneDimensionalSlice {
            function,
            param,
            rows,
            cols,
        });
    }
    Ok(())
}

fn validate_shape_policy(
    function: &'static str,
    policy: ShapePolicy,
    shapes: &[Option<(usize, usize)>],
) -> Result<(), FunctionError> {
    match policy {
        ShapePolicy::EqualAllRanges => {
            if let Some((exp_r, exp_c)) = shapes.first().copied().flatten() {
                for shape in shapes.iter().skip(1) {
                    if let Some((r, c)) = *shape
                        && (r, c) != (exp_r, exp_c)
                    {
                        return Err(FunctionError::DimensionMismatch {
                            function,
                            expected_rows: exp_r,
                            expected_cols: exp_c,
                            found_rows: r,
                            found_cols: c,
                        });
                    }
                }
            }
        }
        ShapePolicy::EqualTargetAndOddPairRanges => {
            if let Some((exp_r, exp_c)) = shapes.first().copied().flatten() {
                for idx in (1..shapes.len()).step_by(2) {
                    if let Some((r, c)) = shapes[idx]
                        && (r, c) != (exp_r, exp_c)
                    {
                        return Err(FunctionError::DimensionMismatch {
                            function,
                            expected_rows: exp_r,
                            expected_cols: exp_c,
                            found_rows: r,
                            found_cols: c,
                        });
                    }
                }
            }
        }
        ShapePolicy::EqualEvenPairRanges => {
            if let Some((exp_r, exp_c)) = shapes.first().copied().flatten() {
                for idx in (2..shapes.len()).step_by(2) {
                    if let Some((r, c)) = shapes[idx]
                        && (r, c) != (exp_r, exp_c)
                    {
                        return Err(FunctionError::DimensionMismatch {
                            function,
                            expected_rows: exp_r,
                            expected_cols: exp_c,
                            found_rows: r,
                            found_cols: c,
                        });
                    }
                }
            }
        }
        ShapePolicy::OneDimEqualLength {
            first_idx,
            first_param,
            second_idx,
            second_param,
        } => {
            let first_shape = shapes.get(first_idx).copied().flatten();
            let second_shape = shapes.get(second_idx).copied().flatten();
            require_1d_slice(function, first_param, first_shape)?;
            require_1d_slice(function, second_param, second_shape)?;
            if let (Some((r1, c1)), Some((r2, c2))) = (first_shape, second_shape) {
                let len1 = r1 * c1;
                let len2 = r2 * c2;
                if len1 != len2 {
                    return Err(FunctionError::LengthMismatch {
                        function,
                        expected: len1,
                        found: len2,
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_index_bounds(
    function: &'static str,
    args: &[CallArg],
    rows: usize,
    cols: usize,
) -> Result<(), FunctionError> {
    if args.len() == 2 {
        if rows > 1 && cols > 1 {
            return Err(FunctionError::MissingColumnIndexFor2DSlice {
                function,
                rows,
                cols,
            });
        }
        let len = rows.max(cols);
        if let Some(CallArg::Expr(row_expr)) = args.get(1)
            && let Some(idx) = extract_const_integer_literal(row_expr)
            && !index_in_bounds(idx, len)
        {
            return Err(FunctionError::IndexOutOfBounds {
                function,
                param: "row_num",
                index: idx,
                min: 1,
                max: len,
            });
        }
    } else if args.len() == 3 {
        if let Some(CallArg::Expr(row_expr)) = args.get(1)
            && let Some(r_idx) = extract_const_integer_literal(row_expr)
            && !index_in_bounds(r_idx, rows)
        {
            return Err(FunctionError::IndexOutOfBounds {
                function,
                param: "row_num",
                index: r_idx,
                min: 1,
                max: rows,
            });
        }
        if let Some(CallArg::Expr(col_expr)) = args.get(2)
            && let Some(c_idx) = extract_const_integer_literal(col_expr)
            && !index_in_bounds(c_idx, cols)
        {
            return Err(FunctionError::IndexOutOfBounds {
                function,
                param: "col_num",
                index: c_idx,
                min: 1,
                max: cols,
            });
        }
    }
    Ok(())
}

/// Returns `true` when 1-based integer index `idx` lies within `1..=max`.
fn index_in_bounds(idx: i64, max: usize) -> bool {
    matches!(usize::try_from(idx), Ok(u) if u >= 1 && u <= max)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArgPresence {
    Present,
    Omitted,
}

trait ArgSlot {
    fn presence(&self) -> ArgPresence;
}

impl ArgSlot for CallArg {
    fn presence(&self) -> ArgPresence {
        match self {
            Self::Expr(_) | Self::Comparator { .. } | Self::BlankCheck(_) => ArgPresence::Present,
            Self::Omitted => ArgPresence::Omitted,
        }
    }
}

impl ArgSlot for FunctionArg<'_> {
    fn presence(&self) -> ArgPresence {
        match self {
            Self::Scalar(_)
            | Self::TypedErrorScalar { .. }
            | Self::Array { .. }
            | Self::Lazy(_)
            | Self::Criterion(_) => ArgPresence::Present,
            Self::Omitted => ArgPresence::Omitted,
        }
    }
}

/// Validates argument count, variadic pair parity, omitted parameter eligibility, and scoped criteria syntax against a resolved function.
fn validate_signature(func: &dyn ExcelFunction, args: &[CallArg]) -> Result<(), FunctionError> {
    validate_arg_slots(func, args)?;
    validate_criteria_slots(func, args)
}

/// Rejects `CallArg::Comparator` and `CallArg::BlankCheck` outside `ParamKind::Criteria` slots.
fn validate_criteria_slots(
    func: &dyn ExcelFunction,
    args: &[CallArg],
) -> Result<(), FunctionError> {
    for (i, arg) in args.iter().enumerate() {
        match arg {
            CallArg::Expr(_) | CallArg::Omitted => {}
            CallArg::Comparator { .. } | CallArg::BlankCheck(_) => {
                let param = resolve_parameter(func, i)?;
                if param.kind != ParamKind::Criteria {
                    return Err(FunctionError::UnexpectedCriteriaSyntax {
                        function: func.name(),
                        param: param.name,
                        position: i + 1,
                    });
                }
            }
        }
    }
    Ok(())
}

/// Validates runtime bound argument count, variadic pair parity, omitted parameter eligibility,
/// argument kinds (`Range` vs scalar/slice), coupled slice shapes, column `RolePolicy`, `EmptyPolicy`,
/// and declarative `ParamType` / `CalendarDate` constraints across all arguments.
pub(crate) fn validate_bound_args(
    func: &dyn ExcelFunction,
    args: &[FunctionArg<'_>],
) -> Result<(), FunctionError> {
    validate_arg_slots(func, args)?;

    let mut shapes: Vec<Option<(usize, usize)>> = Vec::with_capacity(args.len());

    // Pass 1: Validate `ParamKind` vs bound `FunctionArg` variant and collect array shapes.
    for (i, arg) in args.iter().enumerate() {
        let param = resolve_parameter(func, i)?;

        match (param.kind, arg) {
            (_, FunctionArg::Omitted) => shapes.push(None),
            (ParamKind::Range, FunctionArg::Array { rows, cols, .. }) => {
                shapes.push(Some((*rows, *cols)));
            }
            (ParamKind::Range, FunctionArg::Scalar(_) | FunctionArg::TypedErrorScalar { .. }) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "range",
                    found: "scalar",
                });
            }
            (ParamKind::Range, FunctionArg::Lazy(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "range",
                    found: "lazy",
                });
            }
            (ParamKind::Range, FunctionArg::Criterion(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "range",
                    found: "criterion",
                });
            }
            (
                ParamKind::Scalar | ParamKind::Lazy,
                FunctionArg::Array {
                    rows,
                    cols,
                    ref_kind: ReferenceKind::Slice,
                    ..
                },
            ) => {
                return Err(FunctionError::SliceInScalarContext {
                    rows: *rows,
                    cols: *cols,
                });
            }
            (
                ParamKind::Scalar | ParamKind::Lazy,
                FunctionArg::Array {
                    ref_kind: ReferenceKind::SingleCell,
                    ..
                },
            ) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "scalar",
                    found: "array",
                });
            }
            (ParamKind::Scalar | ParamKind::Lazy, FunctionArg::Criterion(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "scalar",
                    found: "criterion",
                });
            }
            (
                ParamKind::Scalar | ParamKind::Lazy,
                FunctionArg::Scalar(_)
                | FunctionArg::TypedErrorScalar { .. }
                | FunctionArg::Lazy(_),
            ) => {
                shapes.push(Some((1, 1)));
            }
            (ParamKind::RangeOrArray, FunctionArg::Array { rows, cols, .. }) => {
                shapes.push(Some((*rows, *cols)));
            }
            (
                ParamKind::RangeOrArray,
                FunctionArg::Scalar(_)
                | FunctionArg::TypedErrorScalar { .. }
                | FunctionArg::Lazy(_),
            ) => {
                shapes.push(Some((1, 1)));
            }
            (ParamKind::RangeOrArray, FunctionArg::Criterion(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "range or scalar",
                    found: "criterion",
                });
            }
            (ParamKind::Criteria, FunctionArg::Criterion(_)) => {
                shapes.push(Some((1, 1)));
            }
            (
                ParamKind::Criteria,
                FunctionArg::Array {
                    rows,
                    cols,
                    ref_kind: ReferenceKind::Slice,
                    ..
                },
            ) => {
                return Err(FunctionError::SliceInScalarContext {
                    rows: *rows,
                    cols: *cols,
                });
            }
            (
                ParamKind::Criteria,
                FunctionArg::Array {
                    ref_kind: ReferenceKind::SingleCell,
                    ..
                },
            ) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "criterion",
                    found: "array",
                });
            }
            (
                ParamKind::Criteria,
                FunctionArg::Scalar(_) | FunctionArg::TypedErrorScalar { .. },
            ) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "criterion",
                    found: "scalar",
                });
            }
            (ParamKind::Criteria, FunctionArg::Lazy(_)) => {
                return Err(FunctionError::InvalidArgumentKind {
                    function: func.name(),
                    expected: "criterion",
                    found: "lazy",
                });
            }
        }
    }

    // Pass 2: Coupled slice-shape invariants run before role and element `ParamType` checks.
    if let ValidationPolicy::ShapeCoupled(shape_policy) = func.validation_policy() {
        validate_shape_policy(func.name(), shape_policy, &shapes)?;
    }

    // Pass 3: Declarative `RolePolicy::MeasureOnly` runs before element `ParamType` checks.
    for (i, arg) in args.iter().enumerate() {
        let param = resolve_parameter(func, i)?;
        if param.role_policy == RolePolicy::MeasureOnly
            && let FunctionArg::Array {
                axis_role: AxisRole::Descriptor,
                ..
            } = arg
        {
            return Err(FunctionError::DescriptorReferenceNotAllowed {
                function: func.name(),
                param: param.name,
                position: i + 1,
            });
        }
    }

    // Pass 4: Per-argument `ParamType` and `EmptyPolicy` validation.
    for (i, arg) in args.iter().enumerate() {
        let param = resolve_parameter(func, i)?;
        let position = i + 1;

        match arg {
            FunctionArg::Scalar(val) => {
                validate_value_against_param_type(
                    val,
                    func.name(),
                    param.name,
                    position,
                    param.param_type,
                    EmptyValidation::RejectEmpty,
                )?;
            }
            FunctionArg::TypedErrorScalar { inferred_type, .. } => {
                if let Some(expected) = param.param_type.to_expected_type()
                    && !value_type_satisfies(*inferred_type, expected)
                {
                    return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                        function: func.name(),
                        param: param.name,
                        position,
                        expected,
                        found: *inferred_type,
                    }));
                }
            }
            FunctionArg::Array {
                values,
                error_types,
                ref_kind,
                ..
            } => {
                let empty_validation = match (param.empty_policy, ref_kind) {
                    (EmptyPolicy::AllowInSlice, ReferenceKind::Slice) => EmptyValidation::SkipEmpty,
                    (EmptyPolicy::AllowInSlice, ReferenceKind::SingleCell)
                    | (EmptyPolicy::ForbidAll, ReferenceKind::SingleCell | ReferenceKind::Slice) => {
                        EmptyValidation::RejectEmpty
                    }
                };
                for (elem_idx, val) in values.iter().enumerate() {
                    if matches!(val, CellValue::Error(_)) {
                        if let Some(err_vt) = error_types.get(elem_idx).copied().flatten()
                            && err_vt != ValueType::Empty
                            && let Some(expected) = param.param_type.to_expected_type()
                            && !value_type_satisfies(err_vt, expected)
                        {
                            return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                                function: func.name(),
                                param: param.name,
                                position,
                                expected,
                                found: err_vt,
                            }));
                        }
                        continue;
                    }
                    validate_value_against_param_type(
                        val,
                        func.name(),
                        param.name,
                        position,
                        param.param_type,
                        empty_validation,
                    )?;
                }
            }
            FunctionArg::Lazy(_) | FunctionArg::Criterion(_) | FunctionArg::Omitted => {}
        }
    }

    // Pass 5: Calendar `(year, month, day)` domain validation after `ParamType::Integer` checks.
    if func.validation_policy() == ValidationPolicy::CalendarDate {
        let year = extract_bound_calendar_int(args.first());
        let month = extract_bound_calendar_int(args.get(1));
        let day = extract_bound_calendar_int(args.get(2));
        validate_calendar_date_components(year, month, day)?;
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmptyValidation {
    RejectEmpty,
    SkipEmpty,
}

fn validate_value_against_param_type(
    val: &CellValue,
    function: &'static str,
    param: &'static str,
    position: usize,
    param_type: ParamType,
    empty_policy: EmptyValidation,
) -> Result<(), FunctionError> {
    if param_type == ParamType::Any {
        return Ok(());
    }

    match empty_policy {
        EmptyValidation::SkipEmpty => {
            if matches!(val, CellValue::Empty) {
                return Ok(());
            }
        }
        EmptyValidation::RejectEmpty => {}
    }

    match param_type {
        ParamType::Any => Ok(()),
        ParamType::NonEmpty => match val {
            CellValue::Number(_)
            | CellValue::Text(_)
            | CellValue::Boolean(_)
            | CellValue::Error(_) => Ok(()),
            CellValue::Empty => Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function,
                param,
                position,
                expected: ExpectedType::NonEmpty,
                found: ValueType::Empty,
            })),
        },
        ParamType::Number => match val.strict_number() {
            Ok(_) => Ok(()),
            Err(found) => Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function,
                param,
                position,
                expected: ExpectedType::Number,
                found,
            })),
        },
        ParamType::DateSerial => match val.strict_number() {
            Ok(Ok(serial)) => {
                if is_valid_input_date_serial(serial) {
                    Ok(())
                } else {
                    Err(FunctionError::InvalidDateSerial {
                        function,
                        param,
                        position,
                        serial: serial.to_string(),
                    })
                }
            }
            Ok(Err(_)) => Ok(()),
            Err(found) => Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function,
                param,
                position,
                expected: ExpectedType::Number,
                found,
            })),
        },
        ParamType::Integer => match val.strict_integer() {
            Ok(_) => Ok(()),
            Err(StrictIntError::WrongType(found)) => {
                Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function,
                    param,
                    position,
                    expected: ExpectedType::Integer,
                    found,
                }))
            }
            Err(StrictIntError::NonInteger(value)) => {
                Err(FunctionError::Type(TypeError::NonIntegerArgument {
                    function,
                    param,
                    position,
                    value,
                }))
            }
        },
        ParamType::AnnuityType => match val.strict_integer() {
            Ok(Ok(iv)) => {
                if (0..=1).contains(&iv) {
                    Ok(())
                } else {
                    Err(FunctionError::InvalidAnnuityType {
                        function,
                        found: iv,
                    })
                }
            }
            Ok(Err(_)) => Ok(()),
            Err(StrictIntError::WrongType(found)) => {
                Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function,
                    param,
                    position,
                    expected: ExpectedType::Integer,
                    found,
                }))
            }
            Err(StrictIntError::NonInteger(value)) => {
                Err(FunctionError::Type(TypeError::NonIntegerArgument {
                    function,
                    param,
                    position,
                    value,
                }))
            }
        },
        ParamType::DayCountBasis => match val.strict_integer() {
            Ok(Ok(iv)) => {
                if (0..=4).contains(&iv) {
                    Ok(())
                } else {
                    Err(FunctionError::InvalidDayCountBasis { found: iv })
                }
            }
            Ok(Err(_)) => Ok(()),
            Err(StrictIntError::WrongType(found)) => {
                Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function,
                    param,
                    position,
                    expected: ExpectedType::Integer,
                    found,
                }))
            }
            Err(StrictIntError::NonInteger(value)) => {
                Err(FunctionError::Type(TypeError::NonIntegerArgument {
                    function,
                    param,
                    position,
                    value,
                }))
            }
        },
        ParamType::Boolean => match val.strict_bool() {
            Ok(_) => Ok(()),
            Err(found) => Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function,
                param,
                position,
                expected: ExpectedType::Boolean,
                found,
            })),
        },
    }
}

fn validate_arg_slots<T: ArgSlot>(
    func: &dyn ExcelFunction,
    args: &[T],
) -> Result<(), FunctionError> {
    let min = func.min_args();
    if args.len() < min {
        return Err(FunctionError::TooFewArguments {
            function: func.name(),
            expected: min,
            found: args.len(),
        });
    }

    if let Some(max) = func.max_args()
        && args.len() > max
    {
        return Err(FunctionError::TooManyArguments {
            function: func.name(),
            expected: max,
            found: args.len(),
        });
    }

    if func.variadic_rule() == VariadicRule::Pairs {
        let fixed_count = func.parameters().len();
        if args.len() > fixed_count && !(args.len() - fixed_count).is_multiple_of(2) {
            return Err(FunctionError::InvalidVariadicPairs {
                function: func.name(),
                count: args.len(),
            });
        }
    }

    let last_present = args
        .iter()
        .rposition(|arg| arg.presence() == ArgPresence::Present);

    for (i, arg) in args.iter().enumerate() {
        match arg.presence() {
            ArgPresence::Present => {}
            ArgPresence::Omitted => {
                let param = resolve_parameter(func, i)?;
                if !param.requirement.is_optional() {
                    return Err(FunctionError::CannotOmitRequiredArgument {
                        function: func.name(),
                        param: param.name,
                        position: i + 1,
                    });
                }
                if last_present.is_none_or(|last| i >= last) {
                    return Err(FunctionError::CannotOmitTrailingArgument {
                        function: func.name(),
                        param: param.name,
                        position: i + 1,
                    });
                }
            }
        }
    }

    Ok(())
}

/// Resolves the expected parameter definition for a given 0-indexed argument position,
/// applying cyclical pairs alternation when `VariadicRule::Pairs` is enabled.
pub(crate) fn resolve_parameter(
    func: &dyn ExcelFunction,
    index: usize,
) -> Result<&ParameterDef, FunctionError> {
    if index < func.parameters().len() {
        Ok(&func.parameters()[index])
    } else if func.variadic_rule() == VariadicRule::Pairs {
        let fixed_count = func.parameters().len();
        if fixed_count < 2 {
            return Err(FunctionError::EvaluationFailure {
                function: func.name(),
                message: "variadic pairs rule requires at least two parameter definitions".into(),
            });
        }
        let offset = (index - fixed_count) % 2;
        Ok(&func.parameters()[fixed_count - 2 + offset])
    } else {
        func.parameters()
            .last()
            .ok_or_else(|| FunctionError::EvaluationFailure {
                function: func.name(),
                message: "variadic argument without parameter definition".into(),
            })
    }
}
