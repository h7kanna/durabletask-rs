# DurableTask Rust

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

WIP implementation of DurableTask Rust SDK for [Dapr SDK](https://github.com/dapr/rust-sdk)

## Examples

```shell
dapr init
dapr run --app-id myapp --dapr-http-port 3500 --dapr-grpc-port 3501
```

## Postgres

```shell
docker run -p 5432:5432 -e POSTGRES_PASSWORD=example postgres
dapr run --app-id denote --dapr-http-port 3500 --dapr-grpc-port 3501 --resources-path components
```

```shell
cargo run --example activity_sequence --features tracing-subscriber
```

```shell
cargo run --example external_events --features tracing-subscriber
```

```shell
cargo run --example parallel --features tracing-subscriber
```

```shell
cargo run --example terminate_test --features tracing-subscriber
```

## Test

```shell
cargo test test_context -- --nocapture
```