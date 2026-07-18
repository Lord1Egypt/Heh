# Heh Standard Library (v1.0)

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
- `list.pop() -> Any or error`
- `list.get(idx: int) -> Any or error`
- `list.sort()`
- `list.map(f: Fn) -> list[Any]`
- `list.filter(f: Fn) -> list[Any]`
- `list.join(sep: str) -> str`

### map
- `map.len() -> int`
- `map.get(key: Any) -> Any or error`
- `map.set(key: Any, val: Any)`
- `map.remove(key: Any)`
- `map.keys() -> list[Any]`
- `map.values() -> list[Any]`

## Modules

### std/math
- `math.sin(x: float) -> float`
- `math.cos(x: float) -> float`
- `math.sqrt(x: float) -> float`
- `math.abs(x: float) -> float`
- `math.pow(base: float, exp: float) -> float`
- `math.log(x: float) -> float`

### std/fmt
- `fmt.format(template: str, args: list[Any]) -> str`

### std/json
- `json.parse(s: str) -> Any or error`
- `json.write(v: Any) -> str`

### std/time
- `time.now_utc() -> int` (Note: pure time functions only, effectful clock is in sys.clock)

### std/csv
- `csv.parse(s: str) -> list[list[str]]`
- `csv.write(rows: list[list[str]]) -> str`

### std/hash
- `hash.sha256(data: str) -> str`
- `hash.crc32(data: str) -> str`

### std/regex
- `regex.is_match(pattern: str, text: str) -> bool`
- `regex.find(pattern: str, text: str) -> str or error`

### std/debug
- `debug.fault(msg: str)`
- `debug.assert(cond: bool, msg: str)`
