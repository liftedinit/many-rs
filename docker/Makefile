SHELL = bash

.PHONY: lockfile
lockfile:
	cd ..; cargo fetch

../Cargo.lock: lockfile

.PHONY: start-ledger-cluster
start-ledger-cluster:
	make -f Makefile.ledger start-nodes

.PHONY: stop-ledger-cluster
stop-ledger-cluster:
	make -f Makefile.ledger stop-nodes

.PHONY: start-ledger-cluster-detached
start-ledger-cluster-detached:
	make -f Makefile.ledger start-nodes-detached

.PHONY: start-kvstore-cluster
start-kvstore-cluster:
	make -f Makefile.kvstore start-nodes

.PHONY: stop-kvstore-cluster
stop-kvstore-cluster:
	make -f Makefile.kvstore stop-nodes

.PHONY: start-kvstore-cluster-detached
start-kvstore-cluster-detached:
	make -f Makefile.kvstore start-nodes-detached

.PHONY: clean
clean:
	make -f Makefile.ledger clean
	make -f Makefile.kvstore clean
