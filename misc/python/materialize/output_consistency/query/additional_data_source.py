# Copyright Materialize, Inc. and contributors. All rights reserved.
#
# Use of this software is governed by the Business Source License
# included in the LICENSE file at the root of this repository.
#
# As of the Change Date specified in that file, in accordance with
# the Business Source License, use of this software will be governed
# by the Apache License, Version 2.0.
from dataclasses import dataclass

from materialize.output_consistency.expression.expression import Expression
from materialize.output_consistency.query.data_source import DataSource
from materialize.output_consistency.query.join import JoinOperator


@dataclass(kw_only=True, unsafe_hash=True)
class AdditionalDataSource:
    data_source: DataSource
    join_operator: JoinOperator
    join_constraint: Expression


def as_data_sources(
    additional_data_sources: list[AdditionalDataSource],
) -> list[DataSource]:
    return [
        additional_data_source.data_source
        for additional_data_source in additional_data_sources
    ]
