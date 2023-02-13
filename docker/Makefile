SHELL = bash

.PHONY: lockfile
lockfile:
	cd ..; cargo fetch

../Cargo.lock: lockfile

.PHONY: docker
docker: many/many-abci many/many-kvstore many/many-ledger

.PHONY: many/many-abci
many/many-abci:
	make -f Makefile.common many/many-abci

.PHONY: many/many-ledger
many/many-ledger:
	make -f Makefile.ledger many/many-ledger

.PHONY: many/many-kvstore
many/many-kvstore:
	make -f Makefile.kvstore many/many-kvstore

.PHONY: start-ledger-cluster
start-ledger-cluster:
	make -f Makefile.ledger start-nodes

.PHONY: stop-ledger-cluster
stop-ledger-cluster:
	make -f Makefile.ledger stop-nodes

.PHONY: start-ledger-cluster-dettached
start-ledger-cluster-dettached:
	make -f Makefile.ledger start-nodes-dettached

.PHONY: start-kvstore-cluster
start-kvstore-cluster:
	make -f Makefile.kvstore start-nodes

.PHONY: stop-kvstore-cluster
stop-kvstore-cluster:
	make -f Makefile.kvstore stop-nodes

.PHONY: start-kvstore-cluster-dettached
start-kvstore-cluster-dettached:
	make -f Makefile.kvstore start-nodes-dettached

.PHONY: clean
clean:
	make -f Makefile.ledger clean
	make -f Makefile.kvstore clean
	rm -rf genfiles-common
