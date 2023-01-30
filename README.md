# system-image-loader

Provides the binary loader for [SystemImageLoader.jl](https://github.com/MichaelHatherly/SystemImageLoader.jl).

## Release Process

Releasing a new version of this software uses [`cargo-workspaces`](https://github.com/pksunkara/cargo-workspaces). Install it globally using

```sh
cargo install cargo-workspaces
```

Then in the the repo root directory run

```sh
cargo ws version
```

and select the type of version bump you would like. This will update, commit, and push changes to the upstream `main` branch that will trigger a CI job that builds new binaries and uploads them as new release artifacts.
