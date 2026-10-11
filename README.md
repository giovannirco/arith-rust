# arith-rust

Integer arithmetic over HTTP: `GET /api/{sum,sub,mul,div}?term_one=<int>&term_two=<int>` answers `{"result": <int>}`. One Rust binary (axum on tokio) listening on port 8000, with a health check at `/healthz`, Prometheus metrics at `/metrics` and a small page at `/`.

The default install is a Deployment and a **ClusterIP** Service: reachable only from inside the cluster, and it needs nothing else from the cluster (no ingress controller, no particular CNI, no operator). Exposing it publicly is an optional extra (see [Optional pieces](#optional-pieces)).

The same contract is implemented three times; this is the Rust one. The others are <https://github.com/giovannirco/arith-ts> (TypeScript, the one running at <https://arith-ts.giovanni.dev.br>) and <https://github.com/giovannirco/arith-ruby> (Ruby).

## Prerequisites

- A Kubernetes cluster and `kubectl` pointed at it.
- `helm` 3.8 or newer.
- For step 3 only: `git`, `make`, Docker, and Rust (rustup; `rust-toolchain.toml` pins the toolchain) to run the tests. To get the image into the cluster you also need kind or minikube, or a registry the cluster can pull from.

## The API

| Request | Response |
|---|---|
| `GET /api/sum?term_one=4&term_two=1` | `200 {"result":5}` |
| `GET /api/sub?term_one=4&term_two=1` | `200 {"result":3}` |
| `GET /api/mul?term_one=4&term_two=1` | `200 {"result":4}` |
| `GET /api/div?term_one=7&term_two=2` | `200 {"result":3}` |
| `GET /api/div?term_one=1&term_two=0` | `400 {"error":"division by zero"}` |
| `GET /api/sum?term_one=abc&term_two=1` | `400 {"error":"term_one must be an integer, got \"abc\""}` |
| `GET /api/sum?term_one=1` | `400 {"error":"term_two is required"}` |
| `GET /api/sum?term_one=9223372036854775807&term_two=1` | `400 {"error":"result does not fit in a 64-bit integer"}` |
| `GET /healthz` | `200 {"status":"ok"}` |

Terms and results are signed 64-bit integers. Division truncates toward zero (`-7/2 = -3`). Bad input, division by zero and a result that does not fit are `400` with a JSON reason; an unknown path is `404` and another method `405`, both JSON. In a query string `+` means a space, so send `%2B2` (or just `2`) for a positive sign.

## 1. Deploy

```sh
git clone https://github.com/giovannirco/arith-rust && cd arith-rust
helm install arith deploy/helm/arith-rust --namespace arith --create-namespace --wait
```

This installs image `ghcr.io/giovannirco/arith-rust:1.2.0`, the release the chart belongs to. Without Helm: `kubectl apply -k deploy/kustomize/base && kubectl -n arith rollout status deployment/arith`.

## 2. Verify

From a pod inside the cluster:

```sh
kubectl -n arith run client --rm -i --restart=Never --image=curlimages/curl:8.22.0 \
  --command -- sh -c "sleep 2; curl -s 'http://arith:8000/api/sub?term_one=4&term_two=1'"
```

Prints `{"result":3}`. (The pause lets kubectl attach before curl exits.) `helm test arith -n arith` runs the same check. To see the page: `kubectl -n arith port-forward svc/arith 8000:8000` and open <http://localhost:8000>.

## 3. Change the API and redeploy

The example makes `sum` saturate at the 64-bit limits instead of refusing. In `src/calc.rs`:

```diff
 pub fn sum(a: i64, b: i64) -> Result<i64, CalcError> {
-    a.checked_add(b).ok_or(CalcError::Overflow)
+    Ok(a.saturating_add(b))
 }
```

The tests pin the old behaviour, so update them too:

- two rows in the table in `src/calc.rs`: change `(Sum, i64::MAX, 1, Err(Overflow))` to `Ok(i64::MAX)` and `(Sum, i64::MIN, -1, Err(Overflow))` to `Ok(i64::MIN)`;
- one case in `tests/api.rs`, `/api/sum?term_one=9223372036854775807&term_two=1` in `errors_are_400_with_a_reason`: move it into `the_worked_example` and expect `{"result":9223372036854775807}`.

Then test, build under the local tag `dev` (no release uses it, so the cluster cannot pull a different image by that name), load it into the cluster and roll it out:

```sh
make test
make image TAG=dev
kind load docker-image ghcr.io/giovannirco/arith-rust:dev            # kind (add --name <cluster> if not "kind")
# minikube image load ghcr.io/giovannirco/arith-rust:dev             # or minikube
helm upgrade arith deploy/helm/arith-rust --namespace arith --set image.tag=dev --wait
```

On a cluster that pulls from a registry, build and push to yours instead: `make image push IMAGE=registry.example.com/arith-rust TAG=dev` (add `PLATFORM=linux/amd64` when your machine and the cluster's nodes differ), then add `--set image.repository=registry.example.com/arith-rust` to the `helm upgrade`.

Ask again:

```sh
kubectl -n arith run client --rm -i --restart=Never --image=curlimages/curl:8.22.0 \
  --command -- sh -c "sleep 2; curl -s 'http://arith:8000/api/sum?term_one=9223372036854775807&term_two=1'"
```

Prints `{"result":9223372036854775807}` where the release answered `400`.

## 4. Remove

```sh
helm uninstall arith --namespace arith
kubectl delete namespace arith
```

With Kustomize: `kubectl delete -k deploy/kustomize/base`.

## Tests and coverage

```sh
make test     # cargo test: the table of cases, the HTTP contract, tracing, graceful shutdown
make cover    # the same with cargo-llvm-cov line coverage, report in coverage/index.html
make lint     # cargo fmt --check and cargo clippy -D warnings
make run      # serve on :8000 with readable logs
```

CI runs lint, tests with coverage, the chart and Kustomize renders, and installs this commit's image with the chart on a kind cluster and runs `helm test`.

## Configuration

Environment variables; the chart sets them through `env` in `values.yaml`.

| Variable | Default | Meaning |
|---|---|---|
| `ARITH_ADDR` | `0.0.0.0:8000` | Listen address |
| `ARITH_LOG_LEVEL` | `info` | A level, or a `tracing` filter such as `info,arith::access=debug` |
| `ARITH_LOG_FORMAT` | `json` | `json` or `text` |
| `ARITH_SHUTDOWN_TIMEOUT` | `10` | Seconds to finish in-flight requests after SIGTERM |
| `OTEL_TRACES_EXPORTER` | `none` | `otlp` sends one span per request |
| `OTEL_LOGS_EXPORTER` | `console` | `console`, `otlp`, `console,otlp` or `none` |
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` | or `grpc` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4318` | The collector; the other standard `OTEL_*` variables apply too |

A value the program cannot use stops it at start with a message naming the variable.

## Optional pieces

Off by default; each is a Helm value and a matching Kustomize component, because each needs something a cluster may not have.

| Piece | Helm value | Needs |
|---|---|---|
| Ingress | `ingress.enabled` | an ingress controller |
| HTTPRoute (public access) | `httpRoute.enabled` | Gateway API and a Gateway |
| NetworkPolicy | `networkPolicy.enabled` | a CNI that enforces policy |
| CiliumNetworkPolicy | `ciliumNetworkPolicy.enabled` | Cilium |
| ServiceMonitor | `serviceMonitor.enabled` | the Prometheus Operator CRDs |
| OTLP export | `env.OTEL_*` | a collector |

`deploy/kustomize/overlays/example` composes every component with placeholder names. None of it is needed for steps 1 to 4. The TypeScript implementation's `deploy/examples` holds a full set that runs on a real cluster.

## Versions and releases

One version per release, semver, written in `Cargo.toml` and repeated in `Chart.yaml` (`version` and `appVersion`), the Kustomize base and this README; CI fails if any of them disagree. The current release is `v1.2.0`: image `ghcr.io/giovannirco/arith-rust:1.2.0` and chart `oci://ghcr.io/giovannirco/charts/arith-rust` version `1.2.0`, which installs that image by default. Commits on master (and pull requests from this repository) also get an image `:sha-<commit>`. CI never publishes `latest` or a bare-integer tag, and never republishes a version.

To release: bump the version in all of those places in one commit, merge it, then `git tag v$(make version) && git push origin v$(make version)`.

```sh
helm install arith oci://ghcr.io/giovannirco/charts/arith-rust --version 1.2.0 --namespace arith --create-namespace
```

## Layout

```
src/calc.rs            sum, sub, mul, div on i64, and their table of cases
src/api.rs             the handlers and the {"error": ...} shape
src/page.rs            serves web/ from inside the binary
src/observe.rs         one span, the request metrics and one log line per request
src/metrics.rs         the Prometheus registry
src/telemetry.rs       where logs and traces go
src/config.rs          the environment variables
src/server.rs          listen, drain on SIGTERM
src/main.rs            reads the environment and the signals
web/                   index.html, style.css, app.js
tests/                 the HTTP contract and the tracing behaviour
Dockerfile             rust:alpine build, distroless/static runtime, uid 65532
deploy/helm/arith-rust the chart
deploy/kustomize       base, components, an example overlay
Makefile               version, test, cover, lint, run, image, push, deploy, upgrade, remove
```

To add or change an operation: edit the function in `src/calc.rs`, add its row to the table below it, register the route in `src/lib.rs`, and give the page a button in `web/index.html`. The error strings live in `src/calc.rs` and `src/api.rs`, next to the tests that pin them.

## License

[MIT](LICENSE)
