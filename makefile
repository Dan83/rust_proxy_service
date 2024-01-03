
INSTALL_PATH := /etc/systemd/system/

.PHONY: all
all: build

.PHONY: build
build:
	cargo build --release 

.PHONY: run
run:
	cargo run

.PHONY: install
install:
	cp ./target/release/traffic /usr/bin/traffic
	cp traffic /etc/systemd/system/traffic.service
	chmod 755 /usr/bin/traffic
	systemctl enable traffic
	systemctl start traffic

	
.PHONY: uninstall
uninstall:
	systemctl stop traffic
	systemctl disable traffic
	rm /etc/systemd/system/traffic.service
	rm /usr/bin/traffic
	

.PHONY: clean
clean:
	@rm -rf ./target