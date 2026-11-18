# Patch layout

- `runtime/` contains the patches the installer applies. Only patches that
  go into an install belong here.

Scripts must only consume patches below `runtime/`. Each runtime patch set
uses a `series` file as the single source for patch order and build identity.
