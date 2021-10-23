ifneq (,$(wildcard ./.env))
	include .env
	export
endif

.PHONY: serve

serve:
	cargo run -- serve
