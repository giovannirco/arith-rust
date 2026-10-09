# arith

arith does integer arithmetic over HTTP. Four endpoints, a page that calls them, Prometheus on `/metrics`. One Rust process on port 8000. Traces and logs go over OTLP if you set the usual `OTEL_*` variables; otherwise they stay on stdout.

Helm and Kustomize both install a Deployment and a ClusterIP Service. The default install works on a cluster that has nothing else: no ingress controller, no special CNI, no operator.

I run one at <https://arith.giovanni.dev.br>. The values for that cluster are in `deploy/examples/arith.giovanni.dev.br.yaml`.

## API

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
| `GET /metrics` | `200`, Prometheus text format |
| `GET /` | `200`, the page |

### Decisions

- Terms and results are signed 64-bit integers. `1.5` is rejected, not rounded. A term that looks like an integer but does not fit gets its own message: `term_one does not fit in a 64-bit integer, got "..."`.
- Division truncates toward zero: `7/2 = 3`, `-7/2 = -3`. Dividing by zero is a `400`.
- A result outside the 64-bit range is a `400`, not a wrapped number.
- Every error is JSON, `{"error":"..."}`, and the text says what was wrong with which parameter. An unknown path is `404 {"error":"not found"}`. A method other than GET is `405 {"error":"method not allowed"}`.
- In a query string `+` is a space, so `term_two=+2` is rejected. Send `%2B2`, or just `2`.
- `/healthz` is the only health URL. Liveness and readiness both use it.

## The page

`GET /` is one screen from the same process: two fields, four operations, the result, and the error text when the API refuses a call. No frontend build, no second container. The page asks the API for every result. It does not compute anything itself.

Locally that is <http://localhost:8000>. In a cluster the Service is ClusterIP, so a person opens it with `kubectl port-forward`. Other workloads call the Service directly.

## Run it here

You need Rust 1.85 or newer ([rustup](https://rustup.rs)), or just Docker.

```sh
make test     # unit tests and the HTTP contract
make cover    # the same, with a line coverage report (cargo-llvm-cov)
make run      # serve on :8000 with readable logs
make image    # docker build, tag 1
```

`make cover` prints a per-file table and writes `coverage/index.html`. The suite covers the worked example, truncation in both signs, division by zero, missing and non-integer terms, overflow in every operation, the error bodies, the page and its assets, the metrics, graceful shutdown and the exporter wiring. `main.rs` (signals and process exit) is the part it does not reach.

## Configuration

Everything is an environment variable. `ARITH_*` belong to this program. `OTEL_*` are the [OpenTelemetry names](https://opentelemetry.io/docs/specs/otel/configuration/sdk-environment-variables/), so a collector's documentation applies as written.

| Variable | Default | Meaning |
|---|---|---|
| `ARITH_ADDR` | `0.0.0.0:8000` | Listen address |
| `ARITH_LOG_LEVEL` | `info` | Log level, or a `tracing` filter such as `info,arith::access=debug` |
| `ARITH_LOG_FORMAT` | `json` | `json` or `text` |
| `ARITH_SHUTDOWN_TIMEOUT` | `10` | Seconds to let requests finish after SIGTERM |
| `OTEL_TRACES_EXPORTER` | `none` | `otlp` sends one span per request |
| `OTEL_LOGS_EXPORTER` | `console` | `console`, `otlp`, or `console,otlp` |
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` | or `grpc` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4318` (`:4317` for gRPC) | Where OTLP goes. `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` and `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` override it per signal |
| `OTEL_SERVICE_NAME` | `arith` | `service.name` on every span and log record |
| `OTEL_TRACES_SAMPLER`, `OTEL_TRACES_SAMPLER_ARG` | `parentbased_always_on` | e.g. `parentbased_traceidratio` and `0.1` |
| `OTEL_EXPORTER_OTLP_HEADERS` | | `key=value,...`, for tenancy or auth headers |

Metrics are pulled, not pushed: scrape `/metrics`. You get `http_requests_total` and `http_request_duration_seconds` by method, route template and status, `arith_operations_total` by operation and outcome (`ok`, `bad_input`, `division_by_zero`, `overflow`), and `arith_build_info`.

Two shapes that work:

```sh
# Through a collector (Alloy, the OpenTelemetry Collector) that fans out to Tempo and Loki.
OTEL_TRACES_EXPORTER=otlp OTEL_LOGS_EXPORTER=console,otlp \
OTEL_EXPORTER_OTLP_ENDPOINT=http://alloy.monitoring.svc:4318 ./arith

# Straight to the stores. Tempo takes OTLP on 4318. Loki takes OTLP logs on /otlp.
OTEL_TRACES_EXPORTER=otlp OTEL_LOGS_EXPORTER=otlp \
OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=http://tempo:4318/v1/traces \
OTEL_EXPORTER_OTLP_LOGS_ENDPOINT=http://loki:3100/otlp/v1/logs ./arith
```

A request that arrives with a W3C `traceparent` header joins that trace, so a span from a gateway in front continues into the service. Each log record carries the trace id, as a field on stdout and in the record itself over OTLP.

If an agent already tails pod stdout into Loki, pick one of `console` and `otlp` for logs, or every line is stored twice.

## Deploy

You need `kubectl` pointed at a cluster, and `helm` 3.8 or newer for the Helm path. The default install is a Deployment and a ClusterIP Service in namespace `arith`. Helm and Kustomize produce the same objects.

### 1. Deploy the public image

```sh
git clone https://github.com/giovannirco/arith && cd arith
helm install arith deploy/helm/arith --namespace arith --create-namespace --wait
```

The same with Kustomize, no Helm needed:

```sh
kubectl apply -k deploy/kustomize/base
kubectl -n arith rollout status deployment/arith
```

### 2. Request it from inside the cluster

```sh
kubectl -n arith run client --rm -i --restart=Never --image=curlimages/curl:8.22.0 \
  --command -- sh -c "sleep 2; curl -s 'http://arith:8000/api/sub?term_one=4&term_two=1'"
```

Prints `{"result":3}`. The pause lets kubectl attach before curl exits; without it a pod this quick often prints nothing. `helm test arith -n arith` runs the same check as a Helm test. For the page:

```sh
kubectl -n arith port-forward svc/arith 8000:8000
```

and open <http://localhost:8000>.

### 3. Change the API and redeploy

Any edit works. The one used for the dry run makes `sum` saturate at the 64-bit limits instead of refusing. In `src/calc.rs`:

```diff
 pub fn sum(a: i64, b: i64) -> Result<i64, CalcError> {
-    a.checked_add(b).ok_or(CalcError::Overflow)
+    Ok(a.saturating_add(b))
 }
```

`make test` now fails on the overflow expectations for `sum`. That is the suite doing its job. Three places disagree with the new behaviour:

- two rows in the table in `src/calc.rs`: change `(Sum, i64::MAX, 1, Err(Overflow))` to `Ok(i64::MAX)` and `(Sum, i64::MIN, -1, Err(Overflow))` to `Ok(i64::MIN)`;
- one case in `tests/api.rs`, `/api/sum?term_one=9223372036854775807&term_two=1` in `errors_are_400_with_a_reason`: move it into `the_worked_example` and expect `{"result":9223372036854775807}`.

Cargo stops at the first failing test binary, so the first run only shows the `src/calc.rs` rows. Run `make test` again until it passes, then build and roll out:

```sh
make test
make image TAG=2
```

Put the image where the cluster can pull it. For kind:

```sh
kind load docker-image ghcr.io/giovannirco/arith:2 --name kind
```

For minikube, `minikube image load ghcr.io/giovannirco/arith:2`. For a real cluster, push to a registry it trusts and add `--set image.repository=registry.example.com/arith` to the next command.

```sh
helm upgrade arith deploy/helm/arith --namespace arith --set image.tag=2 --wait
```

With Kustomize: `kubectl -n arith set image deployment/arith arith=ghcr.io/giovannirco/arith:2 && kubectl -n arith rollout status deployment/arith`, and write the new tag into `deploy/kustomize/base/kustomization.yaml` so the next apply keeps it.

Then ask again:

```sh
kubectl -n arith run client --rm -i --restart=Never --image=curlimages/curl:8.22.0 \
  --command -- sh -c "sleep 2; curl -s 'http://arith:8000/api/sum?term_one=9223372036854775807&term_two=1'"
```

Prints `{"result":9223372036854775807}` where tag 1 answered `400`.

### 4. Remove

```sh
helm uninstall arith --namespace arith
kubectl delete namespace arith
```

With Kustomize, `kubectl delete -k deploy/kustomize/base` removes the namespace too.

## Versions

Two numbers, and they move separately.

- The **image tag** is a plain integer: `ghcr.io/giovannirco/arith:1`, `:2`. `--set image.tag=2` or `kubectl set image` rolls one out. `make image TAG=2` builds one locally.
- The **chart version** is semver, in `deploy/helm/arith/Chart.yaml`. It goes up whenever a template or a default changes, and the chart's `appVersion` is the image tag it installs by default.

A release is a commit that bumps the chart version, then a numeric git tag: `git tag 2 && git push origin 2`. CI publishes the image as `:2` and `:latest`, and the chart at its new version with `appVersion` set to `2`. If that chart version is already on GHCR the chart job fails instead of overwriting it. `oras repo tags ghcr.io/giovannirco/charts/arith` lists what is published, and

```sh
helm install arith oci://ghcr.io/giovannirco/charts/arith --version <chart version> --namespace arith --create-namespace
```

installs a particular one. The chart in this repository keeps `appVersion: "1"`, so section 3 above always shows a rollout from 1 to 2. Follow the walkthrough with that chart, not a published one: a published chart defaults to the newest image, and `--set image.tag=2` may then change nothing.

## Optional pieces

Each one is a Helm toggle and a Kustomize component, off by default, because each needs something a cluster may not have.

| Piece | Helm value | Kustomize component | Needs |
|---|---|---|---|
| Ingress | `ingress.enabled` | `components/ingress` | an ingress controller |
| HTTPRoute | `httpRoute.enabled` | `components/httproute` | Gateway API and a Gateway to attach to |
| NetworkPolicy | `networkPolicy.enabled` | `components/networkpolicy` | a CNI that enforces policy |
| CiliumNetworkPolicy | `ciliumNetworkPolicy.enabled` | `components/cilium-networkpolicy` | Cilium |
| ServiceMonitor | `serviceMonitor.enabled` | `components/servicemonitor` | the Prometheus Operator CRDs |
| OTLP export | `env.OTEL_*` | `components/otlp` | a collector, Tempo or Loki to send to |

The two network policies default-deny and then allow: ingress on 8000 from pods in the cluster (or from the namespaces you list, such as your gateway's), probes from the nodes, egress to DNS and to whatever you name as a destination (your OTLP collector). `deploy/helm/arith/values.yaml` documents every value. `deploy/kustomize/overlays/example` composes every component with placeholder names. `deploy/examples/arith.giovanni.dev.br.yaml` is a full set that actually runs.

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
deploy/helm/arith      the chart
deploy/kustomize       base, components, an example overlay
Makefile               test, cover, lint, run, image, push, deploy, upgrade, remove
```

To add or change an operation: edit the function in `src/calc.rs`, add its row to the table below it, register the route in `src/lib.rs`, and give the page a button in `web/index.html`. The error strings live in `src/calc.rs` and `src/api.rs`, next to the tests that pin them.

## License

[MIT](LICENSE)
