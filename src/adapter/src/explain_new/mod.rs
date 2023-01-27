// Copyright Materialize, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! `EXPLAIN` support for various intermediate representations.
//!
//! Ideally, the `EXPLAIN` support for each IR should be in the
//! crate where this IR is defined. However, due to the use of
//! some generic structs with fields specific to LIR in the current
//! explain paths, and the dependency chain between the crates where
//! the various IRs live, this is not possible. Consequencly, we
//! currently resort to using a wrapper type.

use std::collections::BTreeMap;
use std::fmt;

use mz_expr::{MapFilterProject, RowSetFinishing};
use mz_ore::str::{Indent, IndentLike};
use mz_repr::explain_new::{
    separated_text, AnnotatedPlan, Attributes, DisplayJson, DisplayText, ExplainConfig,
    ExprHumanizer, Indices, RenderingContext, UsedIndexes,
};

pub(crate) mod fast_path;
pub(crate) mod hir;
pub(crate) mod lir;
pub(crate) mod mir;
pub(crate) mod optimizer_trace;

/// Newtype struct for wrapping types that should
/// implement the [`mz_repr::explain_new::Explain`] trait.
pub(crate) struct Explainable<'a, T>(&'a mut T);

impl<'a, T> Explainable<'a, T> {
    pub(crate) fn new(t: &'a mut T) -> Explainable<'a, T> {
        Explainable(t)
    }
}

/// Newtype struct for wrapping types that should implement one
/// of the `Display$Format` traits.
///
/// While explainable wraps a mutable reference passed to the
/// `explain*` methods in [`mz_repr::explain_new::Explain`],
/// [`Displayable`] wraps a shared reference passed to the
/// `fmt_$format` methods in `Display$Format`.
pub(crate) struct Displayable<'a, T>(&'a T);

impl<'a, T> From<&'a T> for Displayable<'a, T> {
    fn from(t: &'a T) -> Self {
        Displayable(t)
    }
}

/// Explain context shared by all [`mz_repr::explain_new::Explain`]
/// implementations in this crate.
#[derive(Debug)]
pub(crate) struct ExplainContext<'a> {
    pub(crate) config: &'a ExplainConfig,
    pub(crate) humanizer: &'a dyn ExprHumanizer,
    pub(crate) used_indexes: UsedIndexes,
    pub(crate) finishing: Option<RowSetFinishing>,
}

/// A structure produced by the `explain_$format` methods in
/// [`mz_repr::explain_new::Explain`] implementations for points
/// in the optimization pipeline identified with a single plan of
/// type `T`.
pub(crate) struct ExplainSinglePlan<'a, T> {
    context: &'a ExplainContext<'a>,
    plan: AnnotatedPlan<'a, T>,
}

impl<'a, T: 'a> DisplayText<()> for ExplainSinglePlan<'a, T>
where
    Displayable<'a, T>: DisplayText<PlanRenderingContext<'a, T>>,
{
    fn fmt_text(&self, f: &mut fmt::Formatter<'_>, _ctx: &mut ()) -> fmt::Result {
        let mut ctx = PlanRenderingContext::new(
            Indent::default(),
            self.context.humanizer,
            self.plan.annotations.clone(),
            self.context.config,
        );

        if let Some(finishing) = &self.context.finishing {
            finishing.fmt_text(f, &mut ctx.indent)?;
            ctx.indented(|ctx| Displayable::from(self.plan.plan).fmt_text(f, ctx))?;
        } else {
            Displayable::from(self.plan.plan).fmt_text(f, &mut ctx)?;
        }

        if !self.context.used_indexes.is_empty() {
            writeln!(f, "")?;
            self.context.used_indexes.fmt_text(f, &mut ctx)?;
        }

        Ok(())
    }
}

impl<'a, T: 'a> DisplayJson for ExplainSinglePlan<'a, T>
where
    T: serde::Serialize,
{
    fn to_serde_value(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self.plan.plan)
    }
}

/// A structure produced by the `explain_$format` methods in
/// [`mz_repr::explain_new::Explain`] implementations at points
/// in the optimization pipeline identified with a
/// `DataflowDescription` instance with plans of type `T`.
pub(crate) struct ExplainMultiPlan<'a, T> {
    pub(crate) context: &'a ExplainContext<'a>,
    // Maps the names of the sources to the linear operators that will be
    // on them.
    pub(crate) sources: Vec<(String, &'a MapFilterProject)>,
    // elements of the vector are in topological order
    pub(crate) plans: Vec<(String, AnnotatedPlan<'a, T>)>,
}

impl<'a, T: 'a> DisplayText<()> for ExplainMultiPlan<'a, T>
where
    Displayable<'a, T>: DisplayText<PlanRenderingContext<'a, T>>,
{
    fn fmt_text(&self, f: &mut fmt::Formatter<'_>, _ctx: &mut ()) -> fmt::Result {
        let mut ctx = RenderingContext::new(Indent::default(), self.context.humanizer);

        // render plans
        for (no, (id, plan)) in self.plans.iter().enumerate() {
            let mut ctx = PlanRenderingContext::new(
                ctx.indent.clone(),
                ctx.humanizer,
                plan.annotations.clone(),
                self.context.config,
            );

            writeln!(f, "{}{}:", ctx.indent, id)?;
            ctx.indented(|ctx| {
                match &self.context.finishing {
                    // if present, a RowSetFinishing always applies to the first rendered plan
                    Some(finishing) if no == 0 => {
                        finishing.fmt_text(f, &mut ctx.indent)?;
                        ctx.indented(|ctx| Displayable(plan.plan).fmt_text(f, ctx))?;
                    }
                    // all other plans are rendered without a RowSetFinishing
                    _ => {
                        Displayable(plan.plan).fmt_text(f, ctx)?;
                    }
                }
                Ok(())
            })?;
        }
        if self.sources.iter().any(|(_, op)| !op.is_identity()) {
            // render one blank line between the plans and sources
            writeln!(f, "")?;
            // render sources
            for (id, op) in self.sources.iter().filter(|(_, op)| !op.is_identity()) {
                writeln!(f, "{}Source {}", ctx.indent, id)?;
                ctx.indented(|ctx| Displayable(*op).fmt_text(f, ctx))?;
            }
        }

        if !self.context.used_indexes.is_empty() {
            writeln!(f, "")?;
            self.context.used_indexes.fmt_text(f, &mut ctx)?;
        }

        Ok(())
    }
}

impl<'a, T: 'a> DisplayJson for ExplainMultiPlan<'a, T>
where
    T: serde::Serialize,
{
    fn to_serde_value(&self) -> serde_json::Result<serde_json::Value> {
        let plans = self
            .plans
            .iter()
            .map(|(id, plan)| {
                // TODO: fix plans with Constants
                serde_json::json!({
                    "id": id,
                    "plan": &plan.plan
                })
            })
            .collect::<Vec<_>>();

        let sources = self
            .sources
            .iter()
            .map(|(id, op)| {
                serde_json::json!({
                    "id": id,
                    "op": op
                })
            })
            .collect::<Vec<_>>();

        let result = serde_json::json!({ "plans": plans, "sources": sources });

        Ok(result)
    }
}

impl<'a> DisplayText<RenderingContext<'a>> for Displayable<'a, MapFilterProject> {
    fn fmt_text(&self, f: &mut fmt::Formatter<'_>, ctx: &mut RenderingContext<'a>) -> fmt::Result {
        let (scalars, predicates, outputs, input_arity) = (
            &self.0.expressions,
            &self.0.predicates,
            &self.0.projection,
            &self.0.input_arity,
        );

        // render `project` field iff not the identity projection
        if &outputs.len() != input_arity || outputs.iter().enumerate().any(|(i, p)| i != *p) {
            let outputs = Indices(outputs);
            writeln!(f, "{}project=({})", ctx.indent, outputs)?;
        }
        // render `filter` field iff predicates are present
        if !predicates.is_empty() {
            let predicates = predicates.iter().map(|(_, p)| Displayable::from(p));
            let predicates = separated_text(" AND ", predicates);
            writeln!(f, "{}filter=({})", ctx.indent, predicates)?;
        }
        // render `map` field iff scalars are present
        if !scalars.is_empty() {
            let scalars = CompactScalarSeq(scalars);
            writeln!(f, "{}map=({})", ctx.indent, scalars)?;
        }

        Ok(())
    }
}

#[allow(missing_debug_implementations)]
pub(crate) struct PlanRenderingContext<'a, T> {
    pub(crate) indent: Indent, // TODO: can this be a ref?
    pub(crate) humanizer: &'a dyn ExprHumanizer,
    pub(crate) annotations: BTreeMap<&'a T, Attributes>, // TODO: can this be a ref?
    pub(crate) config: &'a ExplainConfig,
}

impl<'a, T> PlanRenderingContext<'a, T> {
    pub fn new(
        indent: Indent,
        humanizer: &'a dyn ExprHumanizer,
        annotations: BTreeMap<&'a T, Attributes>,
        config: &'a ExplainConfig,
    ) -> PlanRenderingContext<'a, T> {
        PlanRenderingContext {
            indent,
            humanizer,
            annotations,
            config,
        }
    }
}

impl<'a, T> AsMut<Indent> for PlanRenderingContext<'a, T> {
    fn as_mut(&mut self) -> &mut Indent {
        &mut self.indent
    }
}

impl<'a, T> AsRef<&'a dyn ExprHumanizer> for PlanRenderingContext<'a, T> {
    fn as_ref(&self) -> &&'a dyn ExprHumanizer {
        &self.humanizer
    }
}

/// Pretty-prints a list of scalar expressions that may have runs of column
/// indices as a comma-separated list interleaved with interval expressions.
///
/// Interval expressions are used only for runs of three or more elements.
#[derive(Debug)]
pub struct CompactScalarSeq<'a, T: ScalarOps>(pub &'a [T]);

impl<'a, T> std::fmt::Display for CompactScalarSeq<'a, T>
where
    T: ScalarOps,
    Displayable<'a, T>: DisplayText,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut is_first = true;
        let mut slice = self.0;
        while !slice.is_empty() {
            if !is_first {
                write!(f, ", ")?;
            }
            is_first = false;
            if let Some(lead) = slice[0].match_col_ref() {
                if slice.len() > 2 && slice[1].references(lead + 1) && slice[2].references(lead + 2)
                {
                    let mut last = 3;
                    while slice
                        .get(last)
                        .map(|expr| expr.references(lead + last))
                        .unwrap_or(false)
                    {
                        last += 1;
                    }
                    Displayable::from(&slice[0]).fmt_text(f, &mut ())?;
                    write!(f, "..=")?;
                    Displayable::from(&slice[last - 1]).fmt_text(f, &mut ())?;
                    slice = &slice[last..];
                } else {
                    Displayable::from(&slice[0]).fmt_text(f, &mut ())?;
                    slice = &slice[1..];
                }
            } else {
                Displayable::from(&slice[0]).fmt_text(f, &mut ())?;
                slice = &slice[1..];
            }
        }
        Ok(())
    }
}

pub trait ScalarOps {
    fn match_col_ref(&self) -> Option<usize>;

    fn references(&self, col_ref: usize) -> bool;
}
