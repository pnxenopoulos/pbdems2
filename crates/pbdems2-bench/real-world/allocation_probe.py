"""ctypes bridge for optional, isolated Rust allocation-probe builds."""
import ctypes

STAGES = ("other", "rows", "columns", "frame")


class Totals(ctypes.Structure):
    _fields_ = [(name, ctypes.c_uint64) for name in
                ("allocations", "reallocations", "deallocations", "requested_bytes")]


class Snapshot(ctypes.Structure):
    _fields_ = [(name, ctypes.c_uint64) for name in
                ("live_start", "live_end", "peak_live", "underflows")]
    _fields_.append(("stages", Totals * 4))


class AllocationProbe:
    def __init__(self, library):
        self.library = ctypes.CDLL(str(library))
        try:
            self._begin = self.library.pbdems2_profile_alloc_begin
            self._end = self.library.pbdems2_profile_alloc_end
        except AttributeError as error:
            raise RuntimeError("--allocations requires an isolated allocation-probe build") from error
        self._begin.argtypes = []
        self._begin.restype = None
        self._end.argtypes = []
        self._end.restype = Snapshot

    def begin(self):
        self._begin()

    def end(self):
        snapshot = self._end()
        if snapshot.underflows:
            raise RuntimeError("allocator accounting underflow: live-byte results are invalid")
        result = {name: getattr(snapshot, name) for name, _ in Snapshot._fields_[:-1]}
        result["peak_growth"] = max(0, snapshot.peak_live - snapshot.live_start)
        result["net_live"] = snapshot.live_end - snapshot.live_start
        result["stages"] = {
            name: {field: getattr(totals, field) for field, _ in Totals._fields_}
            for name, totals in zip(STAGES, snapshot.stages)
        }
        return result

