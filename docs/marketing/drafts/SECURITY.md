## Security

If you discover a security vulnerability in Alpha File Manager, please report it
privately — either by emailing **security@aiec.fun** (or your own contact of
choice) or by opening a [GitHub security
advisory](https://github.com/kizemo/alpha-file-manager/security/advisories/new).
Please **do not** file a public issue. Coordinated disclosure is appreciated.

We aim to acknowledge reports within 3 working days and ship a fix for
confirmed issues within 14 days. Severity is judged by what an attacker could
do with the bug, not by how the bug looks on paper.

## Built-in MCP server (3.2.1-beta and later)

The MCP server shipped in 3.2.1-beta is **read-only by design**. None of the
six tools (`list_directory`, `get_drives`, `get_mountable_devices`,
`search_files`, `get_directory_size`, `get_favorites`) move, rename, copy or
delete files. They answer questions; they do not change state.

| Safeguard | What it does |
| --- | --- |
| Localhost only | Binds `127.0.0.1`. No external interface listens. |
| Access key | Random token generated on first activation. Requests without it are refused. |
| Origin allow-list | Requests from non-`file://` origins refused at the protocol layer. |
| Permission levels | Re-checked on every call. Turning a permission down takes effect on the next request. |
| Call log | Every call is logged next to settings — reviewable in plain text. |

If you find a way to bypass any of these, please report it privately using the
channels above.

## Supported versions

Security fixes are backported to the latest released version only. Earlier
versions are not patched — please upgrade.

| Version | Supported |
| --- | --- |
| 3.x (latest stable) | yes |
| 2.x | no — please upgrade |
| 3.2.1-beta series | best-effort, beta cadence |

## What to include in a report

To help us triage, please send:
- The version and OS (Windows / Linux distro + version)
- A short description of the bug and the impact you think it has
- Reproduction steps — what an assistant or external tool asked for, and what
  it received instead
- A screenshot or copy of the call log entry, if relevant

Thanks for helping make Alpha File Manager safer.
