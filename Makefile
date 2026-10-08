# Variables you may want to override on the command line:
#   make image TAG=2
#   make deploy TAG=2 NAMESPACE=arith
IMAGE     ?= ghcr.io/giovannirco/arith
TAG       ?= 1
NAMESPACE ?= arith
RELEASE   ?= arith
CHART     ?= deploy/helm/arith

.PHONY: help test cover lint run image push deploy upgrade remove kustomize-deploy kustomize-remove

help: ## This list
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ {printf "  %-18s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

test: ## Unit and HTTP tests
	cargo test

cover: ## Tests with a line coverage report (needs cargo-llvm-cov)
	@command -v cargo-llvm-cov >/dev/null || { \
	  echo "cargo-llvm-cov is not installed. Install it with:"; \
	  echo "  rustup component add llvm-tools-preview && cargo install cargo-llvm-cov"; \
	  echo "or: brew install cargo-llvm-cov"; exit 1; }
	cargo llvm-cov --all-targets --summary-only
	cargo llvm-cov report --html --output-dir coverage
	@echo "HTML report: coverage/index.html"

lint: ## rustfmt and clippy, as CI runs them
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings

run: ## Serve on :8000 with readable logs
	ARITH_LOG_FORMAT=text cargo run

image: ## Build $(IMAGE):$(TAG) for this machine's architecture
	docker build -t $(IMAGE):$(TAG) .

push: ## Push $(IMAGE):$(TAG)
	docker push $(IMAGE):$(TAG)

deploy: ## Install or upgrade the Helm release with image tag $(TAG)
	helm upgrade --install $(RELEASE) $(CHART) \
	  --namespace $(NAMESPACE) --create-namespace \
	  --set image.tag=$(TAG) --wait

upgrade: deploy ## Same as deploy; reads better after a change

remove: ## Uninstall the release and delete the namespace
	helm uninstall $(RELEASE) --namespace $(NAMESPACE) --ignore-not-found
	kubectl delete namespace $(NAMESPACE) --ignore-not-found

kustomize-deploy: ## The same deployment through kubectl apply -k
	kubectl apply -k deploy/kustomize/base
	kubectl -n $(NAMESPACE) rollout status deployment/arith

kustomize-remove: ## Delete what kustomize-deploy created, namespace included
	kubectl delete -k deploy/kustomize/base --ignore-not-found
