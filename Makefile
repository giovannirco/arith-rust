# Variables you may want to override on the command line:
#   make image TAG=dev
#   make deploy TAG=dev NAMESPACE=arith
#
# VERSION is the release, read from Cargo.toml; Chart.yaml and the
# Kustomize base carry the same one. TAG defaults to it, so `make deploy`
# installs the release. A local build gets a tag no release uses, such as dev.
RELEASE_IMAGE := ghcr.io/giovannirco/arith-rust
VERSION   := $(shell sed -n 's/^version = \"\(.*\)\"$$/\1/p' Cargo.toml)
IMAGE     ?= $(RELEASE_IMAGE)
TAG       ?= $(VERSION)
# Empty builds for this machine; linux/amd64 builds for a cluster of that
# architecture from, say, an arm64 laptop.
PLATFORM  ?=
NAMESPACE ?= arith
RELEASE   ?= arith
CHART     ?= deploy/helm/arith-rust

.PHONY: help version test cover lint run image push deploy upgrade remove kustomize-deploy kustomize-remove

help: ## This list
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ {printf "  %-18s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

version: ## The release version, from Cargo.toml
	@echo $(VERSION)

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

image: ## Build $(IMAGE):$(TAG), for this machine's architecture unless PLATFORM is set
	docker build $(if $(PLATFORM),--platform $(PLATFORM)) -t $(IMAGE):$(TAG) .

push: ## Push $(IMAGE):$(TAG) to your own registry (set IMAGE)
	@if [ "$(IMAGE)" = "$(RELEASE_IMAGE)" ]; then \
	  echo "$(RELEASE_IMAGE) is published by CI from a v<version> tag; set IMAGE to your own registry."; exit 1; fi
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
