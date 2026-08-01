# Heh Standard Library (v1.0 — frozen)

This is the complete surface. Every entry below is exercised by the
conformance corpus; nothing here changes meaning after v1.0 (SPEC §1.2).
Builtin methods are always called with parentheses, `len` included.

## Built-in Methods

### str
- `str.len() -> int`
- `str.upper() -> str`
- `str.lower() -> str`
- `str.trim() -> str`
- `str.split(sep: str) -> list[str]`
- `str.replace(old: str, new: str) -> str`
- `str.contains(sub: str) -> bool`
- `str.starts_with(prefix: str) -> bool`
- `str.chars() -> list[str]`

### list
- `list.len() -> int`
- `list.push(val: Any)`
- `list.pop() -> Any or error` — `err` when the list is empty
- `list.get(idx: int) -> Any?` — `none` when out of range; the non-faulting
  index (SPEC §7.3). Plain `l[i]` faults instead.
- `list.sort()` — sorts in place
- `list.map(f: Fn) -> list[Any]`
- `list.filter(f: Fn) -> list[Any]`
- `list.join(sep: str) -> str`

### map
Maps preserve insertion order (SPEC §5.4), so `keys()`, `values()`, iteration,
printing, and `json.write` are all deterministic.

- `map.len() -> int`
- `map.get(key: Any) -> Any?` — `none` when the key is absent
- `map.set(key: Any, val: Any)`
- `map.remove(key: Any)`
- `map.keys() -> list[Any]`
- `map.values() -> list[Any]`

## Conversions and constructors

Free functions, always in scope (SPEC §5.2, §5.5):

- `int(x: int|float) -> int` — truncates towards zero; faults on `nan`/`inf`
- `float(x: int|float) -> float`
- `str(x: Any) -> str`
- `int_of(s: str) -> int or error` — parse a string
- `list(x: range|str|map|list) -> list` — materializes a **bounded** range,
  a string's chars, or a map's keys

## Modules

All eight modules are pure (no I/O); bring one into scope with `use std/<name>`.

### std/math
- `math.sin(x: float) -> float`
- `math.cos(x: float) -> float`
- `math.sqrt(x: float) -> float`
- `math.abs(x: float) -> float`
- `math.pow(base: float, exp: float) -> float`
- `math.log(x: float) -> float` (natural log)
- `math.floor(x: float) -> float`
- `math.ceil(x: float) -> float`
- `math.pi() -> float`
- `math.e() -> float`

### std/fmt
Heh has native string interpolation (`"{expr}"`), so `std/fmt` covers what
interpolation cannot express:
- `fmt.pad_left(s: str, width: int, fill: str) -> str`
- `fmt.pad_right(s: str, width: int, fill: str) -> str`
- `fmt.repeat(s: str, n: int) -> str`
- `fmt.hex(n: int) -> str`
- `fmt.fixed(x: float, places: int) -> str`

### std/json
- `json.parse(s: str) -> Any or error`
- `json.write(v: Any) -> str` — object keys keep their insertion order, so
  `json.write(json.parse(s))` round-trips key order

### std/csv
- `csv.parse(s: str) -> list[list[str]]`
- `csv.write(rows: list[list[str]]) -> str`

### std/hash
- `hash.sha256(data: str) -> str`
- `hash.crc32(data: str) -> str`

### std/regex
- `regex.is_match(pattern: str, text: str) -> bool`
- `regex.find(pattern: str, text: str) -> str or error`

### std/time
Pure UTC calendar arithmetic over unix milliseconds — the same integer
`sys.clock.now()` returns. Nothing here reads the clock; the instant is always
an argument, so time-dependent code stays testable and capability-free.
Proleptic Gregorian, no timezones.

- `time.format(ms: int) -> str` — ISO-8601 UTC, `"YYYY-MM-DDTHH:MM:SSZ"`
- `time.parts(ms: int) -> map[str, int]` — keys, in this order: `year`,
  `month` (1–12), `day`, `hour`, `minute`, `second`, `milli`,
  `weekday` (0 = Monday … 6 = Sunday), `yearday` (1-based)
- `time.from_parts(year, month, day, hour, minute, second) -> int or error` —
  unix millis; `err` on an out-of-range field (e.g. 30 February)
- `time.is_leap(year: int) -> bool`
- `time.days_in_month(year: int, month: int) -> int or error`

### std/debug
- `debug.fault(msg: str)`
- `debug.assert(cond: bool, msg: str)`

## Capabilities (`sys`)

Only `fn main(sys: Sys)` receives the `Sys` object; pure code has no way to
reach I/O. Each capability can be denied at the CLI with `--deny-<cap>`; a
denied op returns `err("capability denied: <cap>")` and never touches the
resource (fail closed). Relative paths containing `..` are rejected —
traversal outside the working directory requires an absolute path.

### sys
- `sys.print(...args)` — print space-joined values + newline
- `sys.input() -> str or error` — read one line from stdin
- `sys.args -> list[str]` — program arguments (after run flags)

### sys.fs — `--deny-fs`
- `sys.fs.read(path: str) -> str or error`
- `sys.fs.read_bytes(path: str) -> list[int] or error`
- `sys.fs.write(path: str, data: str) -> none or error`
- `sys.fs.append(path: str, data: str) -> none or error`
- `sys.fs.exists(path: str) -> bool`
- `sys.fs.list_dir(path: str) -> list[str] or error`
- `sys.fs.remove(path: str) -> none or error`

### sys.env — `--deny-env`
- `sys.env.get(key: str) -> str?`
- `sys.env.set(key: str, val: str)`

### sys.clock — `--deny-clock`
- `sys.clock.now() -> int` — milliseconds since the Unix epoch
- `sys.clock.sleep(ms: int)` — pause the program; a negative value does not sleep

### sys.rand — `--deny-rand`
- `sys.rand.bytes(n: int) -> list[int] or error` (OS entropy, /dev/urandom)
- `sys.rand.int(min: int, max: int) -> int or error` (half-open `[min, max)`)
- `sys.rand.float() -> float or error` — uniform in `[0.0, 1.0)`, 53 random bits

### sys.net — `--deny-net`
- `sys.net.get(url: str) -> str or error` — HTTP/1.1 GET. `http://` uses a
  std `TcpStream` directly; `https://` shells out to `curl` (the std library
  has no TLS), returning a clean error if `curl` is not installed. Returns the
  response body on a 2xx status, otherwise an error.

v1.0 has no raw-socket capability; see the note under SPEC §10.
