.PHONY: dev dev-tunnel seed services-up services-down services-down-clean migrate build clean format help lint check-file-length test-torture-session test-torture-session-5 test-torture-session-10 test-torture-session-25 test-torture-session-50 test-torture-session-100 test-torture-clean

# Loads DATABASE_URL (and anything else) from the repo-root .env for targets
# that shell out to tools which don't read it themselves (diesel-cli).
ifneq (,$(wildcard .env))
include .env
export
endif

help:
	@echo "Targets:"
	@echo "  make dev              Start postgres+rustfs (if needed), run pending migrations, then start the app (frontend+backend)"
	@echo "  make dev-tunnel       Same as 'make dev', plus a cloudflared tunnel exposing the frontend publicly."
	@echo "                          Set TUNNEL_TOKEN + TUNNEL_HOSTNAME in .env for a stable hostname;"
	@echo "                          without them you get an ephemeral https://*.trycloudflare.com URL"
	@echo "  make services-up      Start postgres+rustfs only (docker compose), detached"
	@echo "  make services-down    Stop postgres+rustfs, keep their data volumes"
	@echo "  make services-down-clean  Stop postgres+rustfs and DELETE their data volumes"
	@echo "  make migrate          Run pending Diesel migrations against DATABASE_URL"
	@echo "  make seed             Seed local demo logins (admin/admin, user1/user1, user2/user2) + a ready-to-play world"
	@echo "  make build            Production build (engine WASM + backend + frontend)"
	@echo "  make clean            Remove build output (dist/)"
	@echo "  make format           Run prettier + cargo fmt"
	@echo "  make lint             Run cargo clippy (-D warnings) across the workspace"
	@echo "  make check-file-length  Fail if any tracked .rs file exceeds 1000 lines (see scripts/check-file-length.sh)"
	@echo ""
	@echo "Load/torture tiers (standalone — no other target ever runs these):"
	@echo "  make test-torture-session  Run the cheap 5-session tier (alias for test-torture-session-5)"
	@echo "  make test-torture-session-N  Run the N-session tier, N one of 5, 10, 25, 50, 100 (scripts/torture.mjs)"
	@echo "                          25/50/100 ask for confirmation: y/N prompt on a TTY, otherwise CONFIRM=1 is required"
	@echo "  make test-torture-clean  Remove orphaned tf-torture-* compose projects and apps/web/torture-results/"

dev: services-up migrate seed
	pnpm dev

dev-tunnel: services-up migrate seed
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
	@# rustfs holds every uploaded map and token image. The backend starts
	@# without it and then fails per-request, which reads as "the asset is
	@# broken" rather than "the object store is not up yet" — so it is waited
	@# for here, beside postgres, rather than discovered later. Any HTTP
	@# answer means it is listening; a 403 from an unauthenticated probe is a
	@# perfectly good sign of life.
	@echo "Waiting for rustfs to accept connections..."
	@i=0; until curl -s -o /dev/null --max-time 2 http://127.0.0.1:9000 || [ $$i -ge 60 ]; do 		i=$$((i+1)); sleep 1; 	done; 	if [ $$i -ge 60 ]; then 		echo "rustfs did not answer on :9000 after 60s — uploads and map imports will fail."; 		exit 1; 	fi
	@echo "rustfs is ready."

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

# Local demo logins and a world that is ready to play, so a fresh clone can
# be tested by a person instead of only by the e2e suite.
#
# The passwords are deliberately weaker than the application's own 12
# character minimum, which is exactly why this is guarded. The seed writes
# real Argon2id hashes straight into `users`, bypassing the registration
# validation that would refuse "admin" outright — so it must never reach a
# database anyone else can log into.
#
# Idempotent: every insert is keyed on a fixed UUID with ON CONFLICT DO
# NOTHING, so `make dev` re-running it costs one no-op query per row.
seed:
	@command -v psql >/dev/null 2>&1 || { 		echo "psql not found — skipping the demo seed. Install postgresql-client to get local logins."; 		exit 0; 	}
	@case "$(DATABASE_URL)" in 		*@localhost*|*@127.0.0.1*|*@postgres*|*@db*) ;; 		*) echo "Refusing to seed demo accounts into a non-local DATABASE_URL."; 		   echo "These passwords are below the app's own minimum and exist for local testing only."; 		   exit 1 ;; 	esac
	@# A DATABASE_URL with no port does not mean the same thing to every tool.
	@# diesel and the server fall back to 5432; a psql from a multi-cluster
	@# install can default to 5433 and then report "connection refused" for a
	@# database that is running perfectly well. Naming the port when the URL
	@# omits one makes this agree with the application rather than with
	@# whatever the local libpq was compiled to prefer.
	@url="$(DATABASE_URL)"; 	case "$$url" in 		*@*:[0-9]*/*) ;; 		*) url=$$(printf '%s' "$$url" | sed -E 's#@([^:/@]+)/#@\1:5432/#') ;; 	esac; 	psql "$$url" -q -v ON_ERROR_STOP=1 -f src/server/seeds/demo_accounts.sql
	@echo "Seeded: admin/admin (site admin), user1/user1 (GM), user2/user2 (player), with a world ready to play."

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

# Load/torture tiers. Deliberately standalone: nothing depends on these and
# they are wired into no aggregate target, because each one stands up its own
# throwaway stack (compose.torture.yml) and takes minutes. They run only
# because somebody typed the name.
#
# 5 and 10 are cheap enough to run on request. 25, 50 and 100 additionally
# require confirmation — an interactive y/N prompt when stdin is a TTY, and
# CONFIRM=1 when it is not, so CI fails fast instead of hanging on a read.
torture-confirm = \
	if [ "$(CONFIRM)" = "1" ]; then \
		:; \
	elif [ -t 0 ]; then \
		printf "This starts a %s-session torture storm (throwaway containers, several minutes). Continue? [y/N] " "$(1)"; \
		read reply; \
		case "$$reply" in \
			[yY]|[yY][eE][sS]) ;; \
			*) echo "Aborted."; exit 1;; \
		esac; \
	else \
		echo "Refusing to start the $(1)-session torture tier without confirmation."; \
		echo "Run it from a terminal, or set CONFIRM=1:  make test-torture-session-$(1) CONFIRM=1"; \
		exit 1; \
	fi

test-torture-session: test-torture-session-5

test-torture-session-5:
	pnpm torture:5

test-torture-session-10:
	pnpm torture:10

test-torture-session-25:
	@$(call torture-confirm,25)
	pnpm torture:25

test-torture-session-50:
	@$(call torture-confirm,50)
	pnpm torture:50

test-torture-session-100:
	@$(call torture-confirm,100)
	pnpm torture:100

# For the Ctrl-C case only — the runner tears its own stack down on both
# success and failure.
test-torture-clean:
	@projects=$$(docker compose ls --all -q 2>/dev/null | grep '^tf-torture-' || true); \
	if [ -n "$$projects" ]; then \
		for project in $$projects; do \
			echo "Removing torture project $$project"; \
			docker compose -f compose.torture.yml -p "$$project" down -v --remove-orphans; \
		done; \
	else \
		echo "No leftover tf-torture-* compose projects."; \
	fi
	rm -rf apps/web/torture-results
