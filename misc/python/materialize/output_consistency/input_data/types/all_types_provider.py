# Copyright Materialize, Inc. and contributors. All rights reserved.
#
# Use of this software is governed by the Business Source License
# included in the LICENSE file at the root of this repository.
#
# As of the Change Date specified in that file, in accordance with
# the Business Source License, use of this software will be governed
# by the Apache License, Version 2.0.
import itertools

from materialize.output_consistency.data_type.data_type import DataType
from materialize.output_consistency.input_data.types.array_type_provider import (
    ARRAY_DATA_TYPES,
)
from materialize.output_consistency.input_data.types.boolean_type_provider import (
    BOOLEAN_DATA_TYPE,
)
from materialize.output_consistency.input_data.types.bytea_type_provider import (
    BYTEA_DATA_TYPE,
)
from materialize.output_consistency.input_data.types.date_time_types_provider import (
    DATE_TIME_DATA_TYPES,
)
from materialize.output_consistency.input_data.types.jsonb_type_provider import (
    JSONB_DATA_TYPE,
)
from materialize.output_consistency.input_data.types.list_type_provider import (
    LIST_DATA_TYPES,
)
from materialize.output_consistency.input_data.types.map_type_provider import (
    MAP_DATA_TYPES,
)
from materialize.output_consistency.input_data.types.number_types_provider import (
    NUMERIC_DATA_TYPES,
)
from materialize.output_consistency.input_data.types.text_type_provider import (
    TEXT_DATA_TYPE,
)
from materialize.output_consistency.input_data.types.uuid_type_provider import (
    UUID_DATA_TYPE,
)

DATA_TYPES: list[DataType] = list(
    itertools.chain(
        NUMERIC_DATA_TYPES,
        [BOOLEAN_DATA_TYPE],
        DATE_TIME_DATA_TYPES,
        [TEXT_DATA_TYPE],
        [BYTEA_DATA_TYPE],
        [JSONB_DATA_TYPE],
        MAP_DATA_TYPES,
        LIST_DATA_TYPES,
        ARRAY_DATA_TYPES,
        [UUID_DATA_TYPE],
    )
)
