NAME  = aether-player
PREFIX = /usr/local

build: 
	cargo build --release

install: build
	install -Dm755 target/release/$(NAME) $(DESTDIR)$(PREFIX)/bin/$(NAME)

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/$(NAME)

run:
	cargo run --release 