---
title: "Canonicalization example: café 🦀"
source_reference: "urn:example:canonicalization"
---
# Operations

Do **not** restart version `9.0.22` before waiting 20 ms. Read the [guide][guide].

### Checks

1. Keep the original Markdown.
   - Preserve Unicode: café 🦀.
   - [x] Keep source spans.

```sh
if [ "$ready" != "yes" ]; then
    sleep 20
fi
```

| Version | Wait |
|:--------|-----:|
| 9.0.22  | 20 ms |

### Checks

> Warnings are not permissions.[^note]

![Local diagram](assets/flow.svg)

[guide]: https://example.test/guide "Synthetic guide"

[^note]: The source's access policy is unknown.
