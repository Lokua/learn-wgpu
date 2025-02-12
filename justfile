start *ARGS:
  RUST_LOG=learn_wgpu=info cargo run --release {{ARGS}}

debug *ARGS:
  RUST_LOG=learn_wgpu=debug cargo run --release {{ARGS}}

trace *ARGS:
  RUST_LOG=learn_wgpu=trace cargo run --release {{ARGS}}

# Usage: just trace-module framework::frame_controller <sketch>
trace-module MODULE *ARGS:
  RUST_LOG=learn_wgpu=info,lattice::{{MODULE}}=trace cargo run --release {{ARGS}}

# To test just a single test, past the test name e.g. just test my_test
# To test a single module, pass the module name e.g. just test my::module
test *ARGS:
  RUST_LOG=learn_wgpu=trace cargo test -- {{ARGS}}

test-trace *ARGS:
  just test --nocapture {{ARGS}}

test-trace-solo *ARGS:
  RUST_LOG=learn_wgpu=trace cargo test {{ARGS}} -- --nocapture
