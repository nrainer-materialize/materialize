# Copyright Materialize, Inc. and contributors. All rights reserved.
#
# Use of this software is governed by the Business Source License
# included in the LICENSE file at the root of this repository.
#
# As of the Change Date specified in that file, in accordance with
# the Business Source License, use of this software will be governed
# by the Apache License, Version 2.0.

import re


class ErrorMessageNormalizer:
    def normalize(self, error_message: str) -> str:
        # replace source prefix in column
        normalized_message = re.sub(
            'column "[^.]*\\.', 'column "<source>.', error_message
        )
        normalized_message = normalized_message.replace("Evaluation error: ", "")

        if normalized_message.startswith("mz_timestamp out of range ("):
            normalized_message = normalized_message[0 : normalized_message.index(" (")]

        return normalized_message
