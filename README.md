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


## How does it work?

The fuzzer generates several addresses,
one of which will be an admin.
These addresses may be contract addresses or native account addresses.

It uses token-specific code to initialize the contract.

It then executes some number of commands against the contract,
either a method on the `TokenInterface` interface,
a token-specific `mint` method, or a command to advance time
and begin a new transaction.
For each call it generates auths for a random subset of addresses.

After every step the fuzzer makes general assertions about invariants,
and specific assertions related to the executed command.

It maintains independent state about what it expects from the token's
internal state, including information about mints, burns, allowances and balances.


## What is tested / asserted?

After every step various invariants are asserted:

- The sum of all balances is equal to the sum of mints minus the sum of burns.
- All pairs of addresses have allowance equal to the fuzzer's own accounting of allowances.
- All current balances are greater than 0.
- All current balances are equal to the fuzzer's own accounting of balances.
- Contract calls do not panic (unless it's with `panic_with_error!`).
  An error of type [`WasmVm`](https://docs.rs/soroban-sdk/latest/soroban_sdk/xdr/enum.ScErrorType.html#variant.WasmVm)
  and code [`InvalidAction`](https://docs.rs/soroban-sdk/latest/soroban_sdk/xdr/enum.ScErrorCode.html#variant.InvalidAction)
  is considered a panic,
  as that is what the runtime generates on panic.
- Math does not overflow (detected as a panic).
- For `approve`, `transfer`, `transfer_from`, `burn_from`, `burn`,
  if the input amount is negative, the call returns an error.
- If the correct auths have not been provided the call fails.
- The results of the `name`, `symbol` and `decimals`
  methods have not changed.


## What is yet to be tested?

- Admin methods other than `mint`. There is no standard
  admin interface for Soroban tokens.
- Accessor methods don't mutate internal state.
- More assertions about negative numbers in various situations.
- More assertions about expected results of individual calls.
- Intentionally expiring allowances, the contract etc.
- Assertions about expected events.
- Comparison to reference implementation
  - We can test that many tokens all have the same / similar behavior as a reference implementation


## Tips for writing fuzzable Soroban contracts

The most important thing to know about fuzzing soroban contracts:
never call `panic!` and related functions to handle errors that may
occur during normal operation: the fuzzer views panics as bugs.
Instead, use the Soroban-specific
[`panic_with_error!`](https://docs.rs/soroban-sdk/latest/soroban_sdk/macro.panic_with_error.html)
macro, which the fuzzer can distinguish from a bare `panic!`.

For additional tips see the end of
[this video presentation](https://www.youtube.com/watch?v=EzhMdIaPETo&pp=ygUec3RlbGxhciBmdXp6aW5nIGJyaWFuIGFuZGVyc29u).


## License

MIT/Apache-2.0
