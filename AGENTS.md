# arith

Public arithmetic service. Read [README.md](README.md) first. It is the contract. When a command in the README does not match the tree, fix the tree or fix the README in the same change. A documented command that does not run is a bug.

This repository is the open-source project described in the README. Docs, commits, UI copy, and issues stay on that subject.

## Shape

One Go process, standard library `net/http`, no web framework. Port 8000 is fixed. The page and the API ship in the same image.

```
cmd/arith/main.go       listen, routes, static page
internal/calc/          Sum, Sub, Mul, Div
internal/calc/calc_test.go
web/index.html          the page, plus its CSS
web/app.js              fetches the API and renders the result
Dockerfile              multi-stage, non-root, port 8000
deploy/                 namespace, deployment, service, kustomization
Makefile                test, cover, run, image, deploy, upgrade, remove
```

Operations live in `internal/calc`. Adding or changing one is a function and a test row, then the route list. The README's change section points at that file.

Image: `ghcr.io/giovannirco/arith`. Tags are plain (`1`, `2`), set from the Makefile, so a rollout is `--set` or `kubectl set image` with one variable.

## API

Match the table in the README, including the error strings for division by zero and a non-integer term. Integers only, int64, division truncates toward zero, overflow is a 400.

`/healthz` is the only health URL. Liveness and readiness both use it.

## Page

One screen. Two inputs, four operations, the result, and the error the API returned. It calls the same endpoints the tests call.

Keep it quiet: a clear type hierarchy, generous space, one accent, a result that is easy to read from across a desk. No canvas, no dashboard, no traffic generator, no chart. A system font stack is enough. The page has to work at a narrow width and look finished at a laptop width.

The page is not a second product. Cluster access stays a ClusterIP Service. People use port-forward. Do not add an Ingress to the documented install.

## Cluster

Plain YAML and `kubectl apply -k deploy/`. No Helm. No assumption about the CNI, the ingress controller, or a metrics stack. A NetworkPolicy is optional and off in the default install. The deploy must succeed on a vanilla cluster without it.

Probes, a non-root user, and a read-only root filesystem belong in the Deployment. Resource requests stay small. One replica is enough.

The README, once the manifests exist, has four pasteable sections:

1. Deploy the public image.
2. Request the worked example from a pod inside the namespace.
3. Change `Sum`, build tag `2`, roll it out, request the endpoint again.
4. Delete the namespace.

## Tests

`go test -cover ./...` is the coverage report. Cover the worked example, truncation both signs, division by zero, a missing parameter, a non-integer, and an overflow. Skip a coverage target that exists to print a number.

## Out of scope

A second service. A database. Authentication. Prometheus, Grafana, or tracing. A service mesh. An Ingress on the default path. A UI framework or a frontend build step.

## Done

- `make test` and `make cover` pass.
- `make run`, then the README's example requests return the documented bodies, and `/` renders.
- `docker build` produces an image that serves on 8000.
- On a clean kind cluster, following only the README: deploy, request from a pod, change `Sum`, redeploy, request again, delete, namespace gone.
