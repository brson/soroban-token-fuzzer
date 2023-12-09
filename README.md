# soroban-token-fuzzer

This is a reusable fuzzer for Soroban contracts
that implement the standard
[`TokenInterface`](https://docs.rs/soroban-sdk/latest/soroban_sdk/token/trait.TokenInterface.html).

Soroban contract authors implementing tokens can use it
to gain confidence in their code.


## How to use

**Note: at present this fuzzer has only been tested on
the Stellar native token, and the Soroban example token.
It needs work to be adapted to requirements of e.g. tokens
that implement logic that deviates from the example token.
If you find the fuzzer triggers false positives on your
token please file issues.**

Running the fuzzer against two in-tree tokens:

```
cargo +nightly fuzz run fuzz_native_token
```

```
cargo +nightly fuzz run fuzz_example_token
```

The main part of this project is the
`soroban-token-fuzzer` crate, in the root directory of this repo.
It is a library that implements reusable token fuzzing logic.
Customized token fuzzers are programs that link to `soroban-token-fuzzer`
and run it with their own configuration.

In this repo, the `soroban-token-fuzzer-driver` crate,
in the [`fuzz`](./fuzz) directory, is such a crate. It includes
the `fuzz_native_token` and `fuzz_example_token` fuzzers.

The easiest way to use this fuzzer is to clone this repo,
and simply add another fuzzer to the `soroban-token-fuzzer-driver`
crate.

### Adding a fuzzer to `soroban-token-fuzzer-driver`

1) Copy `fuzz/fuzz_targets/fuzz_example_token.rs` to
   e.g. `fuzz_my_token.rs`
2) Edit `fuzz/Cargo.toml` to add your contract as a dependency, e.g.

   ```toml
   my-token.path = "../tokens/my-token"
   my-token.features = ["testutils"]
   ```

   Make sure your crate has a "testutils" feature and it is activated.
3) In `fuzz/Cargo.toml`, declare `fuzz_my_token.rs` as a binary:

   ```toml
   [[bin]]
   name = "fuzz_my_token"
   path = "fuzz_targets/fuzz_my_token.rs"
   test = false
   doc = false
   ```

4) Adapt `fuzz_my_token.rs` to use your token.

Now you can fuzz your token with

```
cargo +nightly fuzz run fuzz_my_token
```


## What is tested?


## What is yet to be tested?

- Auths
- Non-contract address types


## License

MIT/Apache-2.0
