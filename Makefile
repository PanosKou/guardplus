APP_NAME=guardplus

all: build

build:
	cargo build --release

run:
	cargo run

test:
	cargo test

fmt:
	cargo fmt

lint:
	cargo clippy -- -D warnings

docker-build:
	docker build -t $(APP_NAME):latest .

docker-run:
	docker run -p 8080:8080 \
		-v $(PWD)/config.yaml:/app/config.yaml \
		-v $(PWD)/cert.pem:/app/cert.pem \
		-v $(PWD)/key.pem:/app/key.pem \
		$(APP_NAME):latest

helm-install:
	helm install $(APP_NAME) ./chart --create-namespace --namespace $(APP_NAME)

helm-upgrade:
	helm upgrade --install $(APP_NAME) ./chart --namespace $(APP_NAME)

clean:
	rm -rf target