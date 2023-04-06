#!/bin/sh
# Just check that our current version has no diff with the example
set -e
cargo r -r -p proto-gen -- validate -d examples/example-integration/proto -d examples/example-integration/include -f examples/example-integration/proto/toplevel.proto -f examples/example-integration/proto/sublevel-at-toplevel.proto -f examples/example-integration/proto/sublevel/sublevel.proto -o examples/example-integration/src/proto_types
cd examples/example-integration
cargo test