# arith-rust

Read [README.md](README.md) first. That file is the contract. If a command in it does not match the tree, fix the tree or the README in the same change. A documented command that does not run is a bug.

This repository is the open-source arithmetic service the README describes. Code, docs, commits, UI copy and issues stay on that subject.

## Shape

One Rust binary, [axum](https://docs.rs/axum) on tokio, nothing else on top. Port 8000 is fixed. The page and the API ship in the same image.

```
src/calc.rs            the four operations on i64, with their table of cases
src/api.rs             handlers, query parsing, the {"error": ...} shape
src/page.rs            web/ compiled into the binary
src/observe.rs         per-request span, metrics and access log
src/metrics.rs         Prometheus registry behind /metrics
src/telemetry.rs       stdout and OTLP exporters for logs and traces
src/config.rs          environment variables, parsed once
src/server.rs          listen, graceful shutdown
src/main.rs            environment in, signals in, exit code out
web/                   index.html, style.css, app.js; no build step
tests/                 HTTP contract, tracing behaviour
Dockerfile             rust:alpine build stage, distroless/static runtime, uid 65532
deploy/helm/arith-rust chart: Deployment, Service, optional Ingress, HTTPRoute,
                       NetworkPolicy, CiliumNetworkPolicy, ServiceMonitor, a helm test
deploy/kustomize       base (namespace, deployment, service), one component per
                       optional piece, an example overlay
.github/workflows      test, lint, coverage, multi-arch image and chart to ghcr
Makefile               test, cover, lint, run, image, push, deploy, upgrade, remove
```

Operations live in `src/calc.rs`. Adding or changing one is a function and a table row there, a route in `src/lib.rs`, a button in `web/index.html`. The README's layout section points at these files. Keep that true.

## API

Match the table in the README, including the error strings. Signed 64-bit integers only. Division truncates toward zero. Division by zero, a missing or non-integer term, and a result that does not fit are all `400` with `{"error":"..."}`. Unknown path `404`, wrong method `405`, both JSON. Tests pin every string; change the test and the string together.

`/healthz` is the only health URL. Liveness and readiness both use it.

## Observability

Metrics are pulled from `/metrics` and always on. Traces and logs leave over OTLP only when the standard `OTEL_*` variables ask for it. Defaults are stdout logs and no traces. Do not invent `ARITH_*` names for things OpenTelemetry already names. Labels on metrics stay bounded: route template, not path; outcome enum, not error text.

## Page

One screen. Two inputs, four operations, the result, the error text the API returned, and the request line that produced them. It calls the same endpoints the tests do and computes nothing itself.

Keep it quiet: system fonts, one accent, generous space, a result readable from across a desk, light and dark. No canvas, no chart, no dashboard, no traffic generator, no framework, no build step. It has to work at phone width and look finished at laptop width.

## Cluster

Both Helm and Kustomize must produce the same default objects: a Deployment with both probes on `/healthz`, non-root, read-only root filesystem, small requests, one replica, `maxUnavailable: 0`; a ClusterIP Service on 8000. The default install must succeed on a vanilla cluster with no ingress controller, no particular CNI, no operator.

Everything else is a toggle that is off by default: Ingress, HTTPRoute, NetworkPolicy, CiliumNetworkPolicy, ServiceMonitor, OTLP export. A toggle in `values.yaml` has a matching component under `deploy/kustomize/components`. When you add a knob to one, add it to the other.

The README has four pasteable sections, and they are the acceptance test: deploy the public image; request the worked example from a pod in the namespace; change `sum`, build tag `2`, roll it out, request again; delete the namespace. Run them on a clean kind cluster before calling a change done.

Image: `ghcr.io/giovannirco/arith-rust`. Tags are plain integers (`1`, `2`), so a rollout is `--set image.tag=2` or `kubectl set image` with one variable. CI publishes a multi-arch image and the chart from a git tag. The Makefile builds the operator's local tag.

## Tests

`make test` is the suite, `make cover` the coverage report. Cover the worked example, truncation in both signs, division by zero, a missing term, a non-integer, a term outside 64 bits, overflow in every operation, the error bodies, the page and its assets, the metrics text, graceful shutdown and the exporter wiring. Keep `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean. Do not add a coverage threshold whose only job is to print a number.

## Versions

Pin what you depend on and look the version up before pinning it: base images by tag and digest, the curl image for tests, crate versions in `Cargo.lock`. Never write a version from memory.

## Out of scope

A second service. A database. Authentication. A service mesh. An Ingress or a LoadBalancer in the default install. A UI framework or a frontend build step. Pushing metrics anywhere.

## Done

- `make lint`, `make test` and `make cover` pass.
- `make run`, then every row of the README's API table returns the documented body, and `/` renders.
- `make image` produces an image that serves on 8000 as uid 65532 with a read-only root.
- `helm lint`, `helm template` with every toggle on, and `kubectl kustomize deploy/kustomize/overlays/example` all render and dry-run apply.
- On a clean kind cluster, following only the README: deploy, request from a pod, change `sum`, redeploy, request again, delete; namespace gone.
