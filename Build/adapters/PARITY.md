# Parity Measurement Table

## Legend

| Value   | Meaning                                         |
|---------|-------------------------------------------------|
| yes     | implemented and documented in adapter API       |
| partial | available indirectly or with reduced ergonomics |
| no      | currently missing                               |

## Capability Parity

| Capability                  | Rust    | TypeScript | Python | C#      | C       | C++     |
|-----------------------------|---------|------------|--------|---------|---------|---------|
| call                        | yes     | yes        | yes    | yes     | yes     | yes     |
| cast                        | yes     | yes        | yes    | yes     | yes     | yes     |
| batch                       | yes     | yes        | yes    | yes     | yes     | yes     |
| stream                      | yes     | yes        | yes    | yes     | yes     | yes     |
| channel                     | yes     | yes        | yes    | yes     | yes     | yes     |
| resource invocation helpers | yes     | yes        | yes    | yes     | yes     | yes     |
| log forwarding helper       | yes     | yes        | yes    | yes     | yes     | yes     |
| provider registration       | yes     | yes        | yes    | yes     | yes     | yes     |

## Tooling Parity

| Capability                  | Rust    | TypeScript | Python | C#      | C       | C++     |
|-----------------------------|---------|------------|--------|---------|---------|---------|
| schema extractor CLI        | yes     | yes        | yes    | yes     | yes     | yes     |
| typed codegen output        | yes     | yes        | yes    | yes     | yes     | yes     |

## Test Coverage

| Test                        | Rust    | TypeScript | Python  | C#      | C       | C++     |
|-----------------------------|---------|------------|---------|---------|---------|---------|
| call                        | yes     | yes        | yes     | yes     | yes     | yes     |
| cast                        | yes     | yes        | yes     | yes     | yes     | yes     |
| batch                       | yes     | yes        | yes     | yes     | yes     | yes     |
| stream                      | yes     | yes        | yes     | yes     | yes     | yes     |
| channel                     | yes     | yes        | yes     | yes     | yes     | yes     |
| resource invocation helpers | yes     | yes        | yes     | yes     | yes     | yes     |
| log forwarding helper       | yes     | yes        | yes     | yes     | yes     | yes     |
| provider registration       | yes     | yes        | yes     | yes     | yes     | yes     |
| schema extractor CLI tests  | yes     | yes        | yes     | yes     | yes     | yes     |
| typed codegen output        | yes     | yes        | yes     | yes     | yes     | yes     |
| envelope roundtrip          | yes     | yes        | yes     | yes     | yes     | yes     |
| transport behavior          | yes     | yes        | yes     | yes     | yes     | yes     |
| timeout and cancellation    | yes     | yes        | yes     | yes     | yes     | yes     |
| error mapping propagation   | yes     | yes        | yes     | yes     | yes     | yes     |
| announce handshake          | yes     | yes        | yes     | yes     | yes     | yes     |
| core runtime integration    | yes     | yes        | yes     | yes     | yes     | yes     |
