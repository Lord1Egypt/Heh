# Security policy

## Supported versions

Only the latest Heh release receives security fixes. The language surface is
frozen, but the implementation and toolchain continue to be hardened.

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting for this repository. Do
not open a public issue for a suspected vulnerability. Include a minimal Heh
program or repository, the affected version and platform, the observed impact,
and any suggested mitigation.

Security reports are acknowledged within seven days. Confirmed issues receive
a coordinated fix and advisory before technical details are made public.

## Security boundaries

Heh programs receive ambient effects only through `Sys`. Capability denial is
a language boundary; the `heh` host process, vendored native tools such as
`curl` and `git`, and the operating system remain trusted computing base.
Vendored Heh sources are verified against `heh.lock` before execution.
