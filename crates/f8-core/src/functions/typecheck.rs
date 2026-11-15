//! Pass 4 single-pass bottom-up static AST type checker (`validate_and_synth_expr`).

use std::convert::Infallible;

use crate::expr::ast::{BinaryOp, CallArg, Expr, Literal, TableRef, UnaryOp};
use crate::functions::binder::{resolve_parameter, validate_calendar_date_components};
use crate::functions::common::{BranchTypeUnifier, value_type_satisfies};
use crate::functions::error::FunctionError;
use crate::functions::lookup::search::{MatchMode, SearchMode};
use crate::functions::registry::FunctionRegistry;
use crate::functions::traits::{
    BranchPolicy, ExcelFunction, LookupPolicy, ParamKind, ParamType, RolePolicy, ShapePolicy,
    ValidationPolicy, VariadicRule,
};
use crate::model::table::AxisRole;
use crate::value::{
    ExpectedType, MAX_SAFE_INTEGER, TypeError, ValueType, is_valid_input_date_serial,
    normalize_zero,
};

/// Static metadata for a resolved [`TableRef`] during Pass 4a type checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticRef {
    /// Number of rows in the resolved reference.
    pub rows: usize,
    /// Number of columns in the resolved reference.
    pub cols: usize,
    /// Uniform column axis role (`Measure` or `Descriptor`) of the resolved reference.
    pub axis_role: AxisRole,
    /// Per-cell static value types in row-major order (empty when element types are unavailable).
    pub elem_types: Vec<Option<ValueType>>,
}

/// Error produced during static AST expression and slice-shape validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageAError<E> {
    /// Static function signature, value-type, or slice-shape error.
    Function(FunctionError),
    /// Coordinate resolution error while inspecting a [`TableRef`].
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

impl FunctionRegistry {
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

    /// Statically validates `expr` using `resolve_ref` to inspect [`TableRef`] dimensions, column axis role, and optional element types.
    pub fn validate_expr_with_shapes<E, F>(
        &self,
        expr: &Expr,
        resolve_ref: F,
    ) -> Result<(), StageAError<E>>
    where
        F: FnMut(&TableRef) -> Result<Option<StaticRef>, E>,
    {
        self.validate_and_synth_expr(expr, resolve_ref)?;
        Ok(())
    }

    /// Single-pass bottom-up static AST validator and return-type synthesizer.
    pub fn validate_and_synth_expr<E, F>(
        &self,
        expr: &Expr,
        mut resolve_ref: F,
    ) -> Result<Option<ValueType>, StageAError<E>>
    where
        F: FnMut(&TableRef) -> Result<Option<StaticRef>, E>,
    {
        self.synth_expr(expr, &mut resolve_ref)
    }

    fn synth_expr<E, F>(
        &self,
        expr: &Expr,
        resolve_ref: &mut F,
    ) -> Result<Option<ValueType>, StageAError<E>>
    where
        F: FnMut(&TableRef) -> Result<Option<StaticRef>, E>,
    {
        match expr {
            Expr::Literal(lit) => {
                let vt = match lit {
                    Literal::Number(_) => ValueType::Number,
                    Literal::String(_) => ValueType::Text,
                    Literal::Boolean(_) => ValueType::Boolean,
                };
                Ok(Some(vt))
            }

            Expr::Reference(table_ref) => {
                let resolved = resolve_ref(table_ref).map_err(StageAError::Resolve)?;
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
                Ok(res
                    .elem_types
                    .first()
                    .copied()
                    .flatten()
                    .filter(|vt| *vt != ValueType::Empty))
            }

            Expr::Unary { op, expr: inner } => match op {
                UnaryOp::Neg => {
                    if let Some(found) = self.synth_expr(inner, resolve_ref)?
                        && found != ValueType::Number
                    {
                        return Err(StageAError::Function(FunctionError::Type(
                            TypeError::InvalidOperandType {
                                operator: op.as_str(),
                                expected: ExpectedType::Number,
                                found,
                            },
                        )));
                    }
                    Ok(Some(ValueType::Number))
                }
            },

            Expr::Binary { op, left, right } => match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow => {
                    if let Some(found) = self.synth_expr(left, resolve_ref)?
                        && found != ValueType::Number
                    {
                        return Err(StageAError::Function(FunctionError::Type(
                            TypeError::InvalidOperandType {
                                operator: op.as_str(),
                                expected: ExpectedType::Number,
                                found,
                            },
                        )));
                    }
                    if let Some(found) = self.synth_expr(right, resolve_ref)?
                        && found != ValueType::Number
                    {
                        return Err(StageAError::Function(FunctionError::Type(
                            TypeError::InvalidOperandType {
                                operator: op.as_str(),
                                expected: ExpectedType::Number,
                                found,
                            },
                        )));
                    }
                    Ok(Some(ValueType::Number))
                }
                BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::Lte
                | BinaryOp::Gt
                | BinaryOp::Gte => {
                    let left_vt = self.synth_expr(left, resolve_ref)?;
                    let right_vt = self.synth_expr(right, resolve_ref)?;
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
                    Ok(Some(ValueType::Boolean))
                }
            },

            Expr::FunctionCall { name, args } => {
                let func = self.validate_call(name, args)?;
                match func.validation_policy() {
                    ValidationPolicy::Standard => self.synth_positional_call(
                        func,
                        args,
                        PositionalPolicy::Standard,
                        resolve_ref,
                    ),
                    ValidationPolicy::ShapeCoupled(shape_policy) => self.synth_positional_call(
                        func,
                        args,
                        PositionalPolicy::ShapeCoupled(shape_policy),
                        resolve_ref,
                    ),
                    ValidationPolicy::CalendarDate => self.synth_positional_call(
                        func,
                        args,
                        PositionalPolicy::CalendarDate,
                        resolve_ref,
                    ),
                    ValidationPolicy::Branch(branch_policy) => {
                        self.synth_branch_call(func, args, branch_policy, resolve_ref)
                    }
                    ValidationPolicy::Lookup(lookup_policy) => {
                        self.synth_lookup_call(func, args, lookup_policy, resolve_ref)
                    }
                }
            }
        }
    }

    fn synth_and_check_param<E, F>(
        &self,
        arg_expr: &Expr,
        function: &'static str,
        param: &'static str,
        position: usize,
        param_type: ParamType,
        resolve_ref: &mut F,
    ) -> Result<Option<ValueType>, StageAError<E>>
    where
        F: FnMut(&TableRef) -> Result<Option<StaticRef>, E>,
    {
        let vt_opt = self.synth_expr(arg_expr, resolve_ref)?;
        if let Some(found) = vt_opt {
            check_param_value_type(arg_expr, found, function, param, position, param_type)?;
        }
        validate_param_const_domain(arg_expr, function, param, position, param_type)?;
        Ok(vt_opt)
    }

    fn synth_positional_call<E, F>(
        &self,
        func: &dyn ExcelFunction,
        args: &[CallArg],
        call_policy: PositionalPolicy,
        resolve_ref: &mut F,
    ) -> Result<Option<ValueType>, StageAError<E>>
    where
        F: FnMut(&TableRef) -> Result<Option<StaticRef>, E>,
    {
        let mut shapes: Vec<Option<(usize, usize)>> = Vec::with_capacity(args.len());
        let mut resolved_refs: Vec<Option<StaticRef>> = Vec::with_capacity(args.len());

        // Pass A: Resolve range reference shapes/roles and validate `ParamKind::Range` constraints.
        for (idx, call_arg) in args.iter().enumerate() {
            let param = resolve_parameter(func, idx)?;
            match (param.kind, call_arg) {
                (_, CallArg::Omitted) => {
                    shapes.push(None);
                    resolved_refs.push(None);
                }
                (
                    ParamKind::Range | ParamKind::RangeOrArray,
                    CallArg::Expr(Expr::Reference(table_ref)),
                ) => {
                    let resolved = resolve_ref(table_ref).map_err(StageAError::Resolve)?;
                    shapes.push(resolved.as_ref().map(|r| (r.rows, r.cols)));
                    resolved_refs.push(resolved);
                }
                (
                    ParamKind::Range,
                    CallArg::Expr(
                        Expr::Literal(_)
                        | Expr::Unary { .. }
                        | Expr::Binary { .. }
                        | Expr::FunctionCall { .. },
                    ),
                ) => {
                    return Err(StageAError::Function(FunctionError::InvalidArgumentKind {
                        function: func.name(),
                        expected: "range",
                        found: "scalar",
                    }));
                }
                (
                    ParamKind::RangeOrArray | ParamKind::Scalar | ParamKind::Lazy,
                    CallArg::Expr(_),
                ) => {
                    shapes.push(Some((1, 1)));
                    resolved_refs.push(None);
                }
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
            let CallArg::Expr(arg_expr) = call_arg else {
                continue;
            };
            match (param.kind, arg_expr) {
                (ParamKind::Range, _) | (ParamKind::RangeOrArray, Expr::Reference(_)) => {
                    if let Some(res) = &resolved_refs[idx] {
                        validate_static_range_elem_types(
                            res,
                            func.name(),
                            param.name,
                            position,
                            param.param_type,
                        )?;
                    }
                }
                (
                    ParamKind::RangeOrArray,
                    Expr::Literal(_)
                    | Expr::Unary { .. }
                    | Expr::Binary { .. }
                    | Expr::FunctionCall { .. },
                )
                | (ParamKind::Scalar | ParamKind::Lazy, _) => {
                    self.synth_and_check_param(
                        arg_expr,
                        func.name(),
                        param.name,
                        position,
                        param.param_type,
                        resolve_ref,
                    )?;
                }
            }
        }

        // Pass E: Calendar `(year, month, day)` constant literal validation after `ParamType::Integer` checks.
        if call_policy == PositionalPolicy::CalendarDate {
            let year = extract_calendar_const_int(args.first());
            let month = extract_calendar_const_int(args.get(1));
            let day = extract_calendar_const_int(args.get(2));
            validate_calendar_date_components(year, month, day)?;
        }

        Ok(func.return_type())
    }

    fn synth_branch_call<E, F>(
        &self,
        func: &dyn ExcelFunction,
        args: &[CallArg],
        branch_policy: BranchPolicy,
        resolve_ref: &mut F,
    ) -> Result<Option<ValueType>, StageAError<E>>
    where
        F: FnMut(&TableRef) -> Result<Option<StaticRef>, E>,
    {
        let result_indices: Vec<usize> = match branch_policy {
            BranchPolicy::If => {
                if let Some(CallArg::Expr(cond_expr)) = args.first() {
                    self.synth_and_check_param(
                        cond_expr,
                        func.name(),
                        "logical_test",
                        1,
                        ParamType::Boolean,
                        resolve_ref,
                    )?;
                }
                vec![1, 2]
            }
            BranchPolicy::Ifs => {
                for idx in (0..args.len()).step_by(2) {
                    if let Some(CallArg::Expr(cond_expr)) = args.get(idx) {
                        self.synth_and_check_param(
                            cond_expr,
                            func.name(),
                            "logical_test1",
                            idx + 1,
                            ParamType::Boolean,
                            resolve_ref,
                        )?;
                    }
                }
                (1..args.len()).step_by(2).collect()
            }
            BranchPolicy::Choose => {
                if let Some(CallArg::Expr(idx_expr)) = args.first() {
                    self.synth_and_check_param(
                        idx_expr,
                        func.name(),
                        "index_num",
                        1,
                        ParamType::Integer,
                        resolve_ref,
                    )?;
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
                let arm_vt = self.synth_expr(arm_expr, resolve_ref)?;
                unifier.record(idx + 1, arm_vt)?;
            }
        }

        Ok(unifier.unified())
    }

    fn synth_lookup_call<E, F>(
        &self,
        func: &dyn ExcelFunction,
        args: &[CallArg],
        lookup_policy: LookupPolicy,
        resolve_ref: &mut F,
    ) -> Result<Option<ValueType>, StageAError<E>>
    where
        F: FnMut(&TableRef) -> Result<Option<StaticRef>, E>,
    {
        match lookup_policy {
            LookupPolicy::Index => {
                let Some(CallArg::Expr(Expr::Reference(arr_ref))) = args.first() else {
                    return Err(StageAError::Function(FunctionError::InvalidArgumentKind {
                        function: func.name(),
                        expected: "range",
                        found: "scalar",
                    }));
                };
                let resolved_arr = resolve_ref(arr_ref).map_err(StageAError::Resolve)?;

                if let Some(CallArg::Expr(row_expr)) = args.get(1) {
                    self.synth_and_check_param(
                        row_expr,
                        func.name(),
                        "row_num",
                        2,
                        ParamType::Integer,
                        resolve_ref,
                    )?;
                }

                if let Some(CallArg::Expr(col_expr)) = args.get(2) {
                    self.synth_and_check_param(
                        col_expr,
                        func.name(),
                        "col_num",
                        3,
                        ParamType::Integer,
                        resolve_ref,
                    )?;
                }

                if let Some(res) = &resolved_arr
                    && res.rows > 0
                    && res.cols > 0
                {
                    validate_index_bounds(func.name(), args, res.rows, res.cols)?;
                }

                if let Some(res) = &resolved_arr {
                    let arr_vt = validate_homogeneous_elem_types(
                        &res.elem_types,
                        None,
                        func.name(),
                        "array",
                        1,
                    )?;
                    Ok(arr_vt)
                } else {
                    Ok(None)
                }
            }

            LookupPolicy::Xlookup => {
                let lookup_vt = if let Some(CallArg::Expr(lv_expr)) = args.first() {
                    self.synth_and_check_param(
                        lv_expr,
                        func.name(),
                        "lookup_value",
                        1,
                        ParamType::NonEmpty,
                        resolve_ref,
                    )?
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

                let resolved_l = resolve_ref(l_ref).map_err(StageAError::Resolve)?;
                let resolved_r = resolve_ref(r_ref).map_err(StageAError::Resolve)?;

                if let Some(CallArg::Expr(mm_expr)) = args.get(4) {
                    self.synth_and_check_param(
                        mm_expr,
                        func.name(),
                        "match_mode",
                        5,
                        ParamType::Integer,
                        resolve_ref,
                    )?;
                    validate_match_mode_literal(func.name(), "match_mode", 5, mm_expr)?;
                }

                if let Some(CallArg::Expr(sm_expr)) = args.get(5) {
                    self.synth_and_check_param(
                        sm_expr,
                        func.name(),
                        "search_mode",
                        6,
                        ParamType::Integer,
                        resolve_ref,
                    )?;
                    parse_search_mode_literal(func.name(), "search_mode", 6, sm_expr)?;
                }

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

                if let Some(res_l) = &resolved_l {
                    validate_homogeneous_elem_types(
                        &res_l.elem_types,
                        lookup_vt,
                        func.name(),
                        "lookup_array",
                        2,
                    )?;
                }

                let ret_vt = match &resolved_r {
                    Some(res_r) => validate_homogeneous_elem_types(
                        &res_r.elem_types,
                        None,
                        func.name(),
                        "return_array",
                        3,
                    )?,
                    None => None,
                };
                let mut unifier = BranchTypeUnifier::with_initial(func.name(), ret_vt);

                if let Some(CallArg::Expr(nf_expr)) = args.get(3) {
                    let nf_vt = self.synth_expr(nf_expr, resolve_ref)?;
                    unifier.record(4, nf_vt)?;
                }

                Ok(unifier.unified())
            }

            LookupPolicy::Xmatch => {
                let lookup_vt = if let Some(CallArg::Expr(lv_expr)) = args.first() {
                    self.synth_and_check_param(
                        lv_expr,
                        func.name(),
                        "lookup_value",
                        1,
                        ParamType::NonEmpty,
                        resolve_ref,
                    )?
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
                let resolved_l = resolve_ref(l_ref).map_err(StageAError::Resolve)?;

                if let Some(CallArg::Expr(mm_expr)) = args.get(2) {
                    self.synth_and_check_param(
                        mm_expr,
                        func.name(),
                        "match_mode",
                        3,
                        ParamType::Integer,
                        resolve_ref,
                    )?;
                    validate_match_mode_literal(func.name(), "match_mode", 3, mm_expr)?;
                }

                if let Some(CallArg::Expr(sm_expr)) = args.get(3) {
                    self.synth_and_check_param(
                        sm_expr,
                        func.name(),
                        "search_mode",
                        4,
                        ParamType::Integer,
                        resolve_ref,
                    )?;
                    parse_search_mode_literal(func.name(), "search_mode", 4, sm_expr)?;
                }

                let l_shape = resolved_l.as_ref().map(|r| (r.rows, r.cols));
                require_1d_slice(func.name(), "lookup_array", l_shape)?;

                if let Some(res_l) = &resolved_l {
                    validate_homogeneous_elem_types(
                        &res_l.elem_types,
                        lookup_vt,
                        func.name(),
                        "lookup_array",
                        2,
                    )?;
                }

                Ok(Some(ValueType::Number))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionalPolicy {
    Standard,
    ShapeCoupled(ShapePolicy),
    CalendarDate,
}

fn validate_homogeneous_elem_types(
    elem_types: &[Option<ValueType>],
    initial_vt: Option<ValueType>,
    function: &'static str,
    param: &'static str,
    position: usize,
) -> Result<Option<ValueType>, FunctionError> {
    let mut expected_vt = initial_vt.filter(|vt| *vt != ValueType::Empty);
    for &vt_opt in elem_types {
        let Some(vt) = vt_opt else {
            continue;
        };
        if vt == ValueType::Empty {
            continue;
        }
        match expected_vt {
            None => expected_vt = Some(vt),
            Some(exp_vt) => {
                if vt != exp_vt {
                    return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                        function,
                        param,
                        position,
                        expected: ExpectedType::from_value_type(exp_vt)
                            .unwrap_or(ExpectedType::NonEmpty),
                        found: vt,
                    }));
                }
            }
        }
    }
    Ok(expected_vt)
}

fn validate_static_range_elem_types(
    res: &StaticRef,
    function: &'static str,
    param: &'static str,
    position: usize,
    param_type: ParamType,
) -> Result<(), FunctionError> {
    let Some(expected) = param_type.to_expected_type() else {
        return Ok(());
    };
    for &vt_opt in &res.elem_types {
        let Some(found) = vt_opt else {
            continue;
        };
        if found == ValueType::Empty {
            continue;
        }
        if !value_type_satisfies(found, expected) {
            return Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function,
                param,
                position,
                expected,
                found,
            }));
        }
    }
    Ok(())
}

fn check_param_value_type(
    arg_expr: &Expr,
    found: ValueType,
    function: &'static str,
    param: &'static str,
    position: usize,
    param_type: ParamType,
) -> Result<(), FunctionError> {
    let Some(expected) = param_type.to_expected_type() else {
        return Ok(());
    };
    if !value_type_satisfies(found, expected) {
        return Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function,
            param,
            position,
            expected,
            found,
        }));
    }
    if expected == ExpectedType::Integer
        && let Some(raw) = extract_signed_const_number(arg_expr)
        && raw.is_finite()
    {
        let norm = normalize_zero(raw);
        if norm.fract() != 0.0 || !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&norm) {
            return Err(FunctionError::Type(TypeError::NonIntegerArgument {
                function,
                param,
                position,
                value: norm.to_string(),
            }));
        }
    }
    Ok(())
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

fn extract_calendar_const_int(call_arg: Option<&CallArg>) -> Option<i64> {
    match call_arg {
        Some(CallArg::Expr(arg_expr)) => extract_signed_const_integer(arg_expr),
        Some(CallArg::Omitted) | None => None,
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

fn f64_to_safe_int(raw: f64) -> Option<i64> {
    if !raw.is_finite() {
        return None;
    }
    let norm = normalize_zero(raw);
    if norm.fract() != 0.0 || !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&norm) {
        return None;
    }
    Some(norm as i64)
}

fn extract_signed_const_integer(expr: &Expr) -> Option<i64> {
    extract_signed_const_number(expr).and_then(f64_to_safe_int)
}

fn extract_const_integer_literal(expr: &Expr) -> Option<i64> {
    extract_const_number_literal(expr).and_then(f64_to_safe_int)
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

/// Validates argument count, variadic pair parity, and omitted parameter eligibility against a resolved function.
pub(crate) fn validate_signature(
    func: &dyn ExcelFunction,
    args: &[CallArg],
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
        .rposition(|arg| !matches!(arg, CallArg::Omitted));

    for (i, arg) in args.iter().enumerate() {
        match arg {
            CallArg::Expr(_) => {}
            CallArg::Omitted => {
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
