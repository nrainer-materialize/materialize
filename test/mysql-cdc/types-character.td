# Copyright Materialize, Inc. and contributors. All rights reserved.
#
# Use of this software is governed by the Business Source License
# included in the LICENSE file at the root of this repository.
#
# As of the Change Date specified in that file, in accordance with
# the Business Source License, use of this software will be governed
# by the Apache License, Version 2.0.

$ set-sql-timeout duration=1s
$ set-max-tries max-tries=20

$ postgres-execute connection=postgres://mz_system:materialize@${testdrive.materialize-internal-sql-addr}
ALTER SYSTEM SET enable_mysql_source = true

#
# Test the various character data types
#

> CREATE SECRET mysqlpass AS '${arg.mysql-root-password}'
> CREATE CONNECTION mysql_conn TO MYSQL (
    HOST mysql,
    USER root,
    PASSWORD SECRET mysqlpass
  )

$ mysql-connect name=mysql url=mysql://root@mysql password=${arg.mysql-root-password}

$ mysql-execute name=mysql
DROP DATABASE IF EXISTS public;
CREATE DATABASE public;
USE public;

# Insert data pre-snapshot
CREATE TABLE t1 (f1 VARCHAR(10), f2 CHAR(10), f3 TEXT);

INSERT INTO t1 VALUES ('abc', 'abc', 'abc');
INSERT INTO t1 VALUES ('ABC', 'ABC', 'ABC');
INSERT INTO t1 VALUES ('abc ', 'abc ', 'abc ');

> CREATE SOURCE mz_source
  FROM MYSQL CONNECTION mysql_conn
  FOR ALL TABLES;

> SELECT COUNT(*) > 0 FROM t1;
true

# Insert the same data post-snapshot
$ mysql-execute name=mysql
INSERT INTO t1 SELECT * FROM t1;

> SELECT pg_typeof(f1), pg_typeof(f2), pg_typeof(f3) FROM t1 LIMIT 1;
"character varying" "character" "text"

> SELECT * FROM t1;
"abc" "abc       " "abc"
"abc" "abc       " "abc"
"ABC" "ABC       " "ABC"
"ABC" "ABC       " "ABC"
"abc " "abc       " "abc "
"abc " "abc       " "abc "

$ mysql-execute name=mysql
UPDATE t1 SET f1 = 'klm', f2 = 'klm', f3 = 'klm' WHERE f1 = 'abc';
UPDATE t1 SET f1 = 'xyz ', f2 = 'xyz ', f3 = 'xyz ' WHERE f1 = 'abc ';

> SELECT * FROM t1;
"klm" "klm       " "klm"
"klm" "klm       " "klm"
"klm" "klm       " "klm"
"klm" "klm       " "klm"
"xyz " "xyz       " "xyz "
"xyz " "xyz       " "xyz "