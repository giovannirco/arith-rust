# arith-rust

Integer arithmetic over HTTP: signed 64-bit terms, four endpoints, a page that calls them, Prometheus on `/metrics`. One process on port 8000.

The default install is a Deployment and a ClusterIP Service and nothing else, so it works on a cluster with no ingress controller, no particular CNI and no operator.

## Install

```sh
helm install arith oci://ghcr.io/giovannirco/charts/arith-rust \
  --namespace arith --create-namespace --wait
```

Add `--version <chart version>` to pin one. `helm test arith -n arith` asks the Service for `4 - 1` and checks the answer.

From inside the cluster:

```sh
kubectl -n arith run client --rm -i --restart=Never --image=curlimages/curl:8.22.0 \
  --command -- sh -c "sleep 2; curl -s 'http://arith:8000/api/sub?term_one=4&term_two=1'"
```

prints `{"result":3}`. For the page, `kubectl -n arith port-forward svc/arith 8000:8000` and open <http://localhost:8000>.

## Endpoints

| Request | Response |
|---|---|
| `GET /api/sum?term_one=4&term_two=1` | `200 {"result":5}` |
| `GET /api/sub?term_one=4&term_two=1` | `200 {"result":3}` |
| `GET /api/mul?term_one=4&term_two=1` | `200 {"result":4}` |
| `GET /api/div?term_one=7&term_two=2` | `200 {"result":3}` |

Division truncates toward zero. Division by zero, a missing or non-integer term, and a result outside 64 bits are `400 {"error":"..."}`. `/healthz` serves both probes and `/metrics` is Prometheus text.

## Versions

The image tag is a plain integer: `--set image.tag=2` rolls out build `2`. The chart version is its own semver and changes when a template or a default does. The chart's `appVersion` is the image tag it installs by default.

## Values

Ingress, HTTPRoute, NetworkPolicy, CiliumNetworkPolicy, ServiceMonitor and OTLP export are all off by default. `values.yaml` documents each one.

## Change it and redeploy

The [repository README](https://github.com/giovannirco/arith-rust#3-change-the-api-and-redeploy) walks through editing an operation, building tag `2`, rolling it out with `helm upgrade --set image.tag=2`, and asking again.

## License

MIT
