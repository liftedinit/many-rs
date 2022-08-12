# Mock Server

This is a mock server that extends the `many` binary with simple
responses for simple methods.

## How to use it

Write a toml file where each key is a method you'll request, and each
value is a string containing a CBOR diagnosis object. See
[testmockfile.toml](./tests/testmockfile.toml) for reference.

Then, start many as normal, but pass your toml file as the
`--mockfile` parameter. For example:

```sh
  cargo run -- server --pem <you.pem> --mockfile <yourtomlfile.toml>
```

After that, you'll be able to request the methods you added to the
toml file, and will receive the expected responses.
