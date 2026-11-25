"""Pi 4 energy-placement package ABI v1. Model ticks are floor(CNTFRQ/10)."""
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path

U64 = (1 << 64) - 1
MAX_BYTES = 65536
REQUEST = ("work", "minimum_capacity", "allowed_cpus", "deadline_ticks", "deadline_reserve")
LIMIT = ("available", "thermal_capacity", "shared_load", "reserve_capacity")
POINT = ("cpu", "opp", "capacity", "energy_per_work", "wake_energy", "wake_ticks")


def _unique(pairs):
    obj = {}
    for key, value in pairs:
        if key in obj:
            raise ValueError(f"duplicate profile key {key!r}")
        obj[key] = value
    return obj


def _fields(obj, required, where, optional=()):
    if type(obj) is not dict:
        raise ValueError(f"{where} must be an object")
    missing = set(required) - obj.keys()
    extra = obj.keys() - set(required) - set(optional)
    if missing or extra:
        raise ValueError(f"{where}: missing {sorted(missing)}, unknown {sorted(extra)}")


def _uint(value, where, maximum=U64):
    if type(value) is not int or not 0 <= value <= maximum:
        raise ValueError(f"{where} must be an integer in 0..{maximum}")
    return value


def _point(obj, where):
    _fields(obj, POINT, where)
    return {k: _uint(obj[k], f"{where}.{k}", {"cpu": 1, "opp": 65535}.get(k, U64))
            for k in POINT}


def _limit(obj, where):
    _fields(obj, LIMIT, where)
    if type(obj["available"]) is not bool:
        raise ValueError(f"{where}.available must be boolean")
    return {"available": obj["available"],
            **{k: _uint(obj[k], f"{where}.{k}") for k in LIMIT[1:]}}


@dataclass(frozen=True)
class PlacementProfile:
    wire: bytes
    source_sha256: str

    @property
    def normalized(self):
        return json.loads(self.wire)

    @property
    def wire_sha256(self):
        return hashlib.sha256(self.wire).hexdigest()

    def rust_constants(self):
        """Emit typed arrays of actual length; no sentinel candidates."""
        obj = self.normalized

        def row(keys, item):
            return "(" + ", ".join(str(item[k]).lower() for k in keys) + ")"

        point_type = "(u8,u16,u64,u64,u64,u64)"
        limits = ", ".join(row(LIMIT, x) for x in obj["limits"])
        points = ", ".join(row(POINT, x) for x in obj["points"])
        fallback = obj["fallback"]
        optional = "None" if fallback is None else f"Some({row(POINT, fallback)})"
        return (
            f"const ENERGY_REQUEST: (u64,u64,u64,u64,u64) = {row(REQUEST, obj)};\n"
            f"const ENERGY_LIMITS: [(bool,u64,u64,u64);2] = [{limits}];\n"
            f"const ENERGY_POINTS: [{point_type};{len(obj['points'])}] = [{points}];\n"
            f"const ENERGY_FALLBACK: Option<{point_type}> = {optional};\n"
        )


def parse_profile(data):
    """Validate original UTF-8 bytes and preserve both provenance identities."""
    if type(data) is not bytes or len(data) > MAX_BYTES:
        raise ValueError(f"profile must be at most {MAX_BYTES} UTF-8 bytes")
    try:
        obj = json.loads(data.decode("utf-8"), object_pairs_hook=_unique)
    except RecursionError as exc:
        raise ValueError("profile nesting exceeds parser limit") from exc
    _fields(obj, ("version", *REQUEST, "limits", "points"), "profile", ("fallback",))
    if _uint(obj["version"], "version") != 1:
        raise ValueError("unsupported profile version")
    request = {k: _uint(obj[k], k, 3 if k == "allowed_cpus" else U64) for k in REQUEST}
    if type(obj["limits"]) is not list or len(obj["limits"]) != 2:
        raise ValueError("limits requires exactly CPU0 and CPU1 in order")
    if type(obj["points"]) is not list or len(obj["points"]) > 8:
        raise ValueError("points requires zero to eight entries")
    limits = [_limit(item, f"limits[{i}]") for i, item in enumerate(obj["limits"])]
    points = [_point(item, f"points[{i}]") for i, item in enumerate(obj["points"])]
    if len({(p["cpu"], p["opp"]) for p in points}) != len(points):
        raise ValueError("duplicate catalogue CPU/OPP")
    fallback = obj.get("fallback")
    normalized = {"version": 1, **request, "limits": limits, "points": points,
                  "fallback": None if fallback is None else _point(fallback, "fallback")}
    wire = json.dumps(normalized, sort_keys=True, separators=(",", ":")).encode("ascii")
    return PlacementProfile(wire, hashlib.sha256(data).hexdigest())


def load_profile(path):
    with Path(path).open("rb") as stream:
        return parse_profile(stream.read(MAX_BYTES + 1))
