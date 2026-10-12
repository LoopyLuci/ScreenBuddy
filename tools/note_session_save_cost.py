"""Measure how long SessionStore::save() takes at the worst case the caps allow.

The audit called the per-push save "synchronous file I/O on the render loop".
Whether that is a real problem depends on the cost, so measure it rather than
assume: 50 sessions x 200 messages is the cap the store enforces.

Run: cargo run --release --example bench_session_save   (or via the test below)
"""

# Kept as a note; the measurement itself lives in the Rust test
# `session::tests::save_cost_is_measured` so it uses the real code path.
print(__doc__)
