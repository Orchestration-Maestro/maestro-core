# Local GGUF tokenizer

`NativeTokenizer` counts tokens with the upstream llama.cpp `llama-tokenize`
tool against a pinned GGUF vocabulary, in a subprocess, without a model forward
pass. The committed profile pins every artifact by size and SHA-256; a local
binding says where this machine keeps them.

## Qualified identity

| Item | Qualified value |
| --- | --- |
| Model | BGE-M3 Q8_0 GGUF, 634,553,760 bytes, SHA-256 `aa473d51f451a22f0fcf39ba3330c14bed38a385712b1113440f69df4047a173` |
| llama.cpp | Commit `77f132cb1df1de6357617aeaf0ca04c02cf15fb1` (tag `b10681`) and its shared libraries |
| Counter | Unmodified upstream `tools/tokenize/tokenize.cpp`, 40,720 bytes, SHA-256 `7a74998544dfae5ce7f81da0c798feae9b44f05c56170b1577b7236b2909694e` |
| Profile | [tokenizer-contract.json](tokenizer-contract.json), schema `local-tokenizer-contract/2`, identifier `sha256:3546447555757daa4996a2e2e708bc67bce4389e8cee3f8386ed503eeaa6d01c` |

The identifier is the SHA-256 of the profile's JSON with sorted keys, no extra
whitespace and no escaping of non-ASCII text, `contract_id` excluded. Chunk and
prepared-input identities include it; where the artifacts sit does not.

## Bind the artifacts on a machine

Write a binding file outside the repository and name it in
`MAESTRO_NATIVE_BINDING`:

```json
{
  "schema": "maestro-native-binding/1",
  "model": "models/bge-m3-q8_0.gguf",
  "counter": "bin/llama-tokenize",
  "library_directory": "/opt/llama.cpp/build/bin",
  "source_root": "/opt/llama.cpp"
}
```

- A relative path resolves against the binding file's directory.
- The file holds at most 1 MiB and exactly these keys.
- Every artifact must match the profile's size and SHA-256; symbolic links
  to artifacts are refused.
- The library directory holds exactly the profile's libraries and their
  version aliases.
  Each profile file name follows one platform's naming, which gives its
  aliases the same way on every host (the committed profile pins a Linux
  build; another platform's build has other bytes and needs its own profile):

  | Platform | Pinned file | Its aliases, links to it |
  | --- | --- | --- |
  | Linux | `libx.so.1.2` | `libx.so`, `libx.so.1` |
  | macOS | `libx.1.2.dylib` | `libx.dylib`, `libx.1.dylib` |
  | Windows | `x.dll` | none |

  An unversioned `libx.so` or `libx.dylib` has no alias. Any other file
  there named with `.so`, or with a `.dylib` or `.dll` extension in any
  letter case, is refused. Every alias must resolve to its pinned file in
  this directory, and the pinned file must not be a link itself. Both sides
  are compared after resolving the directory, so reaching it through a link
  (macOS `/var`) changes nothing.
- The counter loads exactly those libraries: its platform's loader searches
  the library directory before the directory the counter was built for.
  The rest of the environment is the profile's, on every platform. A
  library directory holding the loader's list separator (`:`, or `;` on
  Windows) is refused.

  | Platform | The counter runs with | What the binding must provide |
  | --- | --- | --- |
  | Linux | `LD_LIBRARY_PATH` set to the directory | Nothing more |
  | macOS | `DYLD_LIBRARY_PATH` set to the directory | The Mach-O counter itself, see below |
  | Windows | `PATH`: the directory, then the profile's | The counter inside the directory |

  On macOS, System Integrity Protection strips `DYLD_*` variables when it
  starts a protected executable (anything under `/usr/bin`, `/bin`, `/sbin`
  or `/System`, such as the `/usr/bin/python3` shim), and the hardened
  runtime ignores them without the
  `com.apple.security.cs.allow-dyld-environment-variables` entitlement. The
  counter must be the built `llama-tokenize` itself, outside those
  directories and not signed with the hardened runtime; a wrapper script or
  interpreter in between would lose the variable.

  Windows searches the executable's own directory, then the system
  directories and the current directory, before `PATH`, and no variable
  precedes them. So there the counter is refused unless it sits in the
  library directory, as llama.cpp's Windows build places it.

Then run the native tests: `just native`. Without a binding they fail with an
error naming `MAESTRO_NATIVE_BINDING`; they are never skipped silently.

## Exact input and counting contract

1. **Prepare one string:** `ordered-input-parts/v1` concatenates the ordered
   `input_parts[].text` verbatim. Titles, headings, repeated table headers,
   list context and separators are explicit parts. No hidden prefix or suffix,
   query instruction, trimming, escaping, line-ending conversion or caller
   Unicode normalization is applied.
2. **Tokenize the complete string:** its UTF-8 bytes go to stdin with
   `--offline --stdin --no-escape --ids`. The GGUF's own normalization applies
   inside llama.cpp; it never rewrites the retained input.
3. **Both special-token controls on:** `add_special=true`, `parse_special=true`;
   the model adds BOS `0` and EOS `2`. Special-looking literals follow this
   GGUF's vocabulary.
4. **Count without padding or truncation:** target 500, hard maximum 700,
   context and separators included. Above 700, chunking splits; nothing is
   clipped.
5. **Local execution boundary:** the environment is replaced by the profile's,
   GPU visibility cleared, library fingerprints checked, a 30-second timeout
   enforced, and only a JSON array of valid IDs accepted.

The GGUF declares a `t5` tokenizer, which this llama.cpp maps to its unigram
tokenizer: 250,002 entries and an embedded normalization map, all pinned by
the model's hash.

## Why this matches the embedding path

In the pinned llama.cpp source, `tools/server/server-context.cpp`
(`handle_embeddings_impl`) tokenizes string inputs with `add_special=true`,
`parse_special=true`; `tools/server/server-common.cpp` delegates to
`common_tokenize` without an instruction or chat template; and
`tools/tokenize/tokenize.cpp` reads stdin verbatim with `--no-escape` and calls
the same tokenizer. No live router call was made; S1 checks the serving path
against this profile before indexing (ADR-0008).

## Qualification results

41 synthetic complete inputs, each tokenized twice, compared by ordered IDs:

| Check | Result |
| --- | --- |
| Multilingual, Unicode, code, headings, tables, whitespace, NUL, literal specials, inert instructions | Deterministic; input hashes and IDs retained |
| Boundaries | Exact 499/500/501 and 699/700/701 counts; 8,192/8,193 counted in full |
| Specials | `Hello world` gives `[0,35378,8999,2]`; without automatic specials `[35378,8999]` |
| Cached ONNX tokenizer as a substitute | 19/41 matched, 22/41 differed: rejected |

| Input | GGUF count | Cached tokenizer | Difference |
| --- | ---: | ---: | --- |
| `"  a   b  "` | 4 | 5 | An extra whitespace ID `6` before EOS |
| Whitespace only | 2 | 3 | An extra whitespace ID |
| `"left   <mask>   right"` | 8 | 5 | Mask ID `250001` where this GGUF tokenizes the literal |
| 700-token cases | 700 | 701 | Trailing whitespace |

The comparison tokenizer both overcounts and undercounts, so it is no
conservative bound. The Python qualification script that produced these
results is kept with the S0 archive; S1 replaces it with Rust parity tests.

## Build the counter

With `LLAMA_CPP` naming a llama.cpp checkout at the pinned commit and
`LLAMA_LIB` its built library directory:

```bash
mkdir -p target/tokenizer-qualification
c++ -std=c++17 -O3 -DNDEBUG \
  -DGGML_BACKEND_SHARED -DGGML_SHARED -DGGML_USE_CPU -DGGML_USE_CUDA -DLLAMA_SHARED \
  -I"$LLAMA_CPP/common" -I"$LLAMA_CPP/vendor" -I"$LLAMA_CPP/include" \
  -I"$LLAMA_CPP/ggml/include" "$LLAMA_CPP/tools/tokenize/tokenize.cpp" \
  -L"$LLAMA_LIB" -Wl,-rpath,"$LLAMA_LIB" \
  "$LLAMA_LIB/libllama-common.so.0.3.0" "$LLAMA_LIB/libllama.so.0.3.0" \
  "$(readlink -f "$LLAMA_LIB/libggml.so")" -o target/tokenizer-qualification/llama-tokenize
```

A rebuilt counter has new bytes: it needs a new profile and review, not a
silent swap. The tool supports `--no-parse-special` but not `--parse-special`
(true is its default); `--no-bos` also drops EOS and is a negative control only.

## Limitations

The subprocess reloads the vocabulary for each input; a persistent adapter
waits for a measured need. CUDA-linked libraries stay dependencies with GPU
visibility cleared. This is not a universal tokenizer equivalence, an
authenticity certification of the model's origin, or a claim that embedding
inference accepts 8,193 tokens.
