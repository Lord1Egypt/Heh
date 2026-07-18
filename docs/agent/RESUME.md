# Current State
Phase P6 (String Formatting & Standard Library) is COMPLETE and merged.
String interpolation, `str.len`, `int_of`, `str`, `ok`, and `err` built-ins are working.
`strings.heh` passes the test suite.

# Next Step
Start Phase P7 — I/O Capabilities (Security)
- Implement `sys.read_file(path, cap)` and `sys.write_file(path, data, cap)`.
- Implement capability tokens (read/write access) checking.
- Gate: `tests/corpus/programs/io.heh`.
