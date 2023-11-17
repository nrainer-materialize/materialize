// Copyright Materialize, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Fuses a sequence of `TopK` operators in to one `TopK` operator

use mz_expr::visit::Visit;
use mz_expr::MirRelationExpr;

use crate::TransformCtx;

/// Fuses a sequence of `TopK` operators in to one `TopK` operator if
/// they happen to share the same grouping and ordering key.
#[derive(Debug)]
pub struct TopK;

impl crate::Transform for TopK {
    #[tracing::instrument(
        target = "optimizer",
        level = "debug",
        skip_all,
        fields(path.segment = "topk_fusion")
    )]
    fn transform(
        &self,
        relation: &mut MirRelationExpr,
        _: &mut TransformCtx,
    ) -> Result<(), crate::TransformError> {
        relation.visit_mut_pre(&mut Self::action)?;
        mz_repr::explain::trace_plan(&*relation);
        Ok(())
    }
}

impl TopK {
    /// Fuses a sequence of `TopK` operators in to one `TopK` operator.
    pub fn action(relation: &mut MirRelationExpr) {
        if let MirRelationExpr::TopK {
            input,
            group_key,
            order_key,
            limit,
            offset,
            monotonic,
            expected_group_size,
        } = relation
        {
            while let MirRelationExpr::TopK {
                input: inner_input,
                group_key: inner_group_key,
                order_key: inner_order_key,
                limit: inner_limit,
                offset: inner_offset,
                monotonic: inner_monotonic,
                expected_group_size: inner_expected_group_size,
            } = &mut **input
            {
                // We can fuse two chained TopK operators as long as they share the
                // same grouping and ordering key.
                if *group_key == *inner_group_key && *order_key == *inner_order_key {
                    // Given the following limit/offset pairs:
                    //
                    // inner_offset          inner_limit
                    // |------------|xxxxxxxxxxxxxxxxxx|
                    //              |------------|xxxxxxxxxxxx|
                    //              outer_offset    outer_limit
                    //
                    // the limit/offset pair of the fused TopK operator is computed
                    // as:
                    //
                    // offset = inner_offset + outer_offset
                    // limit = min(max(inner_limit - outer_offset, 0), outer_limit)
                    let inner_limit_usize = inner_limit.as_ref().map(|l| l.as_literal_usize());
                    let outer_limit_usize = limit.as_ref().map(|l| l.as_literal_usize());
                    // If either limit is an expression rather than a literal, bail out.
                    if inner_limit_usize == Some(None) || outer_limit_usize == Some(None) {
                        break;
                    }
                    let inner_limit_usize = inner_limit_usize.and_then(|l| l);
                    let outer_limit_usize = outer_limit_usize.and_then(|l| l);

                    if let Some(inner_limit) = inner_limit_usize {
                        let inner_limit_minus_outer_offset = inner_limit.saturating_sub(*offset);
                        let new_limit = if let Some(outer_limit) = outer_limit_usize {
                            std::cmp::min(outer_limit, inner_limit_minus_outer_offset)
                        } else {
                            inner_limit_minus_outer_offset
                        };
                        *limit = Some(mz_expr::MirScalarExpr::literal_ok(
                            mz_repr::Datum::UInt64(new_limit.try_into().unwrap()),
                            mz_repr::ScalarType::UInt64,
                        ));
                    }

                    if let Some(0) = limit.as_ref().and_then(|l| l.as_literal_usize()) {
                        relation.take_safely();
                        break;
                    }

                    *offset += *inner_offset;
                    *monotonic = *inner_monotonic;

                    // Expected group size is only a hint, and setting it small when the group size
                    // might actually be large would be bad.
                    //
                    // rust-lang/rust#70086 would allow a.zip_with(b, max) here.
                    *inner_expected_group_size =
                        match (&expected_group_size, &inner_expected_group_size) {
                            (Some(a), Some(b)) => Some(std::cmp::max(*a, *b)),
                            _ => None,
                        };

                    **input = inner_input.take_dangerous();
                } else {
                    break;
                }
            }
        }
    }
}
