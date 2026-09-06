@echo off
rem Windows entry point for the ordinary-SQLite reader used by the encrypted
rem database tests. Point FOCUSBRIDGE_SQLITE3_TEST_BIN at this file when the
rem machine has no real sqlite3.exe. See ordinary-sqlite3.py for why.
python "%~dp0ordinary-sqlite3.py" %*
