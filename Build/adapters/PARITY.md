# Parity Measurement Table

## Legend

| Value   | Meaning                                         |
|---------|-------------------------------------------------|
| yes     | implemented and documented in adapter API       |
| partial | available indirectly or with reduced ergonomics |
| no      | currently missing                               |

## Cool table

| Capability                  | Rust    | TypeScript | Python | C#      | C       | C++     |
|-----------------------------|---------|------------|--------|---------|---------|---------|
| call                        | yes     | yes        | yes    | yes     | yes     | yes     |
| cast                        | yes     | yes        | yes    | yes     | yes     | yes     |
| batch                       | yes     | yes        | yes    | yes     | yes     | yes     |
| stream                      | yes     | yes        | yes    | yes     | yes     | yes     |
| channel                     | yes     | yes        | yes    | yes     | yes     | yes     |
| resource invocation helpers | yes     | partial    | yes    | partial | yes     | yes     |
| log forwarding helper       | yes     | yes        | yes    | yes     | yes     | yes     |
| provider registration       | yes     | yes        | yes    | yes     | yes     | yes     |
| schema extractor CLI        | partial | yes        | yes    | partial | no      | no      |
| typed codegen output        | no      | yes        | yes    | yes     | partial | partial |

## Cool table but tests

| Test                        | Rust    | TypeScript | Python  | C#      | C       | C++     |
|-----------------------------|---------|------------|---------|---------|---------|---------|
| call                        | yes     | yes        | yes     | yes     | no      | no      |
| cast                        | yes     | yes        | yes     | yes     | no      | no      |
| batch                       | yes     | yes        | yes     | yes     | partial | no      |
| stream                      | partial | yes        | yes     | yes     | partial | no      |
| channel                     | partial | yes        | yes     | yes     | partial | no      |
| resource invocation helpers | partial | yes        | yes     | yes     | partial | no      |
| log forwarding helper       | partial | yes        | yes     | yes     | partial | no      |
| provider registration       | yes     | yes        | yes     | yes     | partial | no      |
| schema extractor CLI        | no      | partial    | no      | partial | no      | no      |
| typed codegen output        | no      | no         | no      | no      | yes     | yes     |
| envelope roundtrip          | partial | yes        | yes     | yes     | no      | no      |
| transport behavior          | yes     | yes        | yes     | yes     | no      | no      |
| timeout and cancellation    | partial | yes        | yes     | yes     | no      | no      |
| error mapping propagation   | yes     | yes        | yes     | yes     | partial | no      |
| announce handshake          | partial | yes        | partial | partial | no      | no      |
| core runtime integration    | yes     | yes        | yes     | yes     | partial | partial |
