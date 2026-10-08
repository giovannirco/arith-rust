# arith

arith is a small HTTP service for integer arithmetic, with a page for the same four operations and a Kubernetes layout an operator can deploy, change, and delete from this file.

One process. Port 8000.

**Status:** this commit is the contract. The service, the page, the image, and the manifests are the work that follows. The commands below are the interface they have to grow into. They are not runnable yet.

## API

| Request | Response |
|---|---|
| `GET /api/sum?term_one=4&term_two=1` | `200 {"result":5}` |
| `GET /api/sub?term_one=4&term_two=1` | `200 {"result":3}` |
| `GET /api/mul?term_one=4&term_two=1` | `200 {"result":4}` |
| `GET /api/div?term_one=7&term_two=2` | `200 {"result":3}` |
| `GET /api/div?term_one=1&term_two=0` | `400 {"error":"division by zero"}` |
| `GET /api/sum?term_one=abc&term_two=1` | `400 {"error":"term_one must be an integer, got \"abc\""}` |

`GET /healthz` returns `200` when the process can serve.

### Decisions

- Terms and results are integers. `1.5` is rejected, not rounded.
- Division truncates toward zero: `7/2 = 3`, `-7/2 = -3`.
- A result that does not fit in an int64 is a `400`, not a wrapped number.
- Bad input and division by zero are `400` with a JSON body `{"error":"..."}` that names what was wrong.
- Responses are JSON. Strings use double quotes.

## UI

`GET /` is one page, served by the same process as the API. Two fields, the four operations, the result, and the error text when the API rejects a call. No second container and no frontend build.

Locally the page is <http://localhost:8000>. In a cluster the Service is ClusterIP, so a person opens the page with `kubectl port-forward`. Other applications in the cluster call the Service directly.

## Run

Once the service exists:

```sh
make test
make cover
make run
```

`make run` listens on `:8000`.

## Deploy

One namespace, `arith`. A Deployment and a ClusterIP Service. The documented path has no Ingress and no LoadBalancer. The container listens on 8000, and both probes use `/healthz`.

The first install pulls a public image, `ghcr.io/giovannirco/arith`. Changing the program is a loop an operator can follow without asking anyone: edit an operation, run the tests, build a new tag, push it or load it into the cluster, roll the Deployment, request the endpoint again, and see the new result. Removal deletes the namespace.

Those commands belong in this README, next to `deploy/`, when they exist. Write them so each one can be pasted. Then try them on a clean kind cluster in this order: deploy, request `/api/sub?term_one=4&term_two=1` from a pod, change `Sum`, redeploy, request it again, delete the namespace.

## License

[MIT](LICENSE)
