"""A minimal stand-in for the sqlite3 shell, backed by Python's stdlib SQLite.

`tests/encrypted_database.rs` proves that an implementation with no knowledge of
our key cannot read the encrypted database. That proof is only worth anything if
the reader is genuinely independent of our build, so the test shells out to an
ordinary SQLite rather than reusing the SQLCipher-linked `rusqlite` in-process.

Prefer a real `sqlite3` binary when the machine has one: point
`FOCUSBRIDGE_SQLITE3_TEST_BIN` at it. This helper exists for machines that do
not, and is independent in the way that matters — CPython links its own upstream
SQLite, built separately from this project and with no cipher support at all
(`PRAGMA cipher_version` returns no rows, which the test asserts).

Usage, matching the subset of the shell the test invokes:

    ordinary-sqlite3.py <database> "<sql>;<sql>;..."

Rows print as `|`-joined columns, one per line. Any SQLite error goes to stderr
and exits non-zero, exactly as the shell does.
"""

import sqlite3
import sys


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(f"usage: {argv[0]} <database> <sql>", file=sys.stderr)
        return 2
    database, script = argv[1], argv[2]
    try:
        # Never create a missing file: the test distinguishes "unreadable" from
        # "silently replaced with a new empty database".
        uri = database == ":memory:"
        connection = sqlite3.connect(
            database if uri else f"file:{database}?mode=rw",
            uri=not uri,
            isolation_level=None,
        )
    except sqlite3.Error as error:
        print(f"Error: unable to open database file: {error}", file=sys.stderr)
        return 1

    try:
        with connection:
            for statement in (s.strip() for s in script.split(";")):
                if not statement:
                    continue
                for row in connection.execute(statement):
                    print("|".join("" if value is None else str(value) for value in row))
    except sqlite3.Error as error:
        # Python reports an encrypted or corrupt header as "file is not a
        # database", the same wording the shell uses.
        print(f"Error: {error}", file=sys.stderr)
        return 1
    finally:
        connection.close()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
