# genesis-from-db

This is a tool to extract the genesis data from a RocksDB store.
This tool requires the persistent storage to have the following migrations activated:
- Data migration
- Memo migration
- Token migration

This tool will extract
- The IDStore seed
- The IDStore keys
- The symbols
- The token identity
- The account identity
- The balances
- The accounts
and create a genesis file, i.e., `ledger_state.json`.

It also has the ability to extract
- Events
- Multisig transactions

and output the result as JSON.

This tool will NOT extract
- The data attributes (recalculated at block 1)
- The data info (recalculated at block 1)
- The token extended info (not used in the genesis)
- The next subresource id (recalculated)
- Kvstore-related data

The new genesis file can be used to start a new ledger with the same state as the original ledger. The following
migrations will need to be activated from block 0
- Data migration
- Memo migration
- Token migration

## Usage

```sh
# Create a new genesis file from the database
$ genesis-from-db storage.db genesis > ledger_state.json

# Extract events from the database
$ genesis-from-db storage.db events > events.json

# Extract multisig transactions from the database
$ genesis-from-db storage.db multisig > multisig.json
```

## Known issues
- Only Memo containing a single `String` are supported
- ExtendedInfo are mocked 
- Only Ledger is supported, i.e., no KvStore, Compute, ...
- Only the Multisig `Feature` attributes are extracted
- Unsupported Events/Multisig will trigger a panic
- Only the MFX token is supported