.PHONY: dev dev-tunnel services-up services-down services-down-clean migrate build clean format help lint check-file-length

# Loads DATABASE_URL (and anything else) from the repo-root .env for targets
# that shell out to tools which don't read it themselves (diesel-cli).
ifneq (,$(wildcard .env))
include .env
export
endif

help:
	@echo "Targets:"
	@echo "  make dev              Start postgres+rustfs (if needed), run pending migrations, then start the app (frontend+backend)"
	@echo "  make dev-tunnel       Same as 'make dev', plus a cloudflared quick tunnel exposing the frontend at a public https://*.trycloudflare.com URL"
	@echo "  make services-up      Start postgres+rustfs only (docker compose), detached"
	@echo "  make services-down    Stop postgres+rustfs, keep their data volumes"
	@echo "  make services-down-clean  Stop postgres+rustfs and DELETE their data volumes"
	@echo "  make migrate          Run pending Diesel migrations against DATABASE_URL"
	@echo "  make build            Production build (engine WASM + backend + frontend)"
	@echo "  make clean            Remove build output (dist/)"
	@echo "  make format           Run prettier + cargo fmt"
	@echo "  make lint             Run cargo clippy (-D warnings) across the workspace"
	@echo "  make check-file-length  Fail if any tracked .rs file exceeds 1000 lines (see scripts/check-file-length.sh)"

dev: services-up migrate
	pnpm dev

dev-tunnel: services-up migrate
	@command -v cloudflared >/dev/null 2>&1 || { \
		echo "cloudflared not found. Install it: https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/"; \
		exit 1; \
	}
	pnpm dev --tunnel

services-up:
	@if [ ! -f .env ]; then \
		echo "No .env found at repo root — copy .env.example to .env first (see .env.example)."; \
		exit 1; \
	fi
	docker compose up -d
	@echo "Waiting for postgres to accept connections..."
	@until docker compose exec -T postgres pg_isready -U postgres >/dev/null 2>&1; do sleep 1; done
	@echo "postgres is ready."

services-down:
	docker compose stop

services-down-clean:
	docker compose down -v

migrate:
	@command -v diesel >/dev/null 2>&1 || { \
		echo "diesel-cli not found. Install it with: cargo install diesel_cli --no-default-features --features postgres"; \
		exit 1; \
	}
	cd src/server && diesel migration run

build:
	pnpm build

clean:
	pnpm clean

format:
	pnpm format

lint:
	cargo clippy --workspace --all-targets -- -D warnings

check-file-length:
	@./scripts/check-file-length.sh
