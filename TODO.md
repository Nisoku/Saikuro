# TODO

## Adapters

### I can implement

- [ ] JavaScript (maybe TS wrapper idk but useful, or we can have `tsup` compile it to JS)
- [ ] Bash (lol, I know some Bash but this is the scariest adapter haha we don't want to delete anything from the system, it needs to be EXTREMELY SANDBOXED/safe)
- [ ] Zsh (ditto)
- [ ] AssemblyScript (lol this technically isn't needed since TS adapter but we could have a wasm-specific adapter using it (I don't know AS but it's really similar to TS from what i've heard))
- [ ] Swift Adapter (Ladybird would benefit from it (bit too late for that but either way))
- [ ] Lua (super useful, especially for games and game engines and so much more (Picotility might benefit hehe))
- [ ] AppleScript (yes really, it's powerful but painful to write in)
- [ ] Kotlin (so good)
- [ ] Java Adapter

### Need help

- [ ] Nim (Compiles to C. Easy adapter. I just don’t know Nim at all.)
- [ ] F# (easy, since we already have a C# one but idk F#)
- [ ] Zig (genuinely super useful but again, idk Zig)
- [ ] Go (Go brrrr. I don’t know Go. Someone else can go do Go. Go do Go.)
- [ ] Ruby (old but common, super useful)
- [ ] PHP (HAHAHAHA. This would be so funny. Also horrifying. Also kind of useful.)
- [ ] Perl (For the memes, but also rather useful for legacy)
- [ ] Haskell (For the memes, but also rather useful for legacy)
- [ ] OCaml (For the memes, but also rather useful for legacy)
- [ ] R (For the memes, but also rather useful for legacy)
- [ ] Julia (Fast. Scientific. Dynamic. I googled it once. That’s the extent of my expertise.)
- [ ] Fortran (Shockingly useful for HPC. Scientists would cry tears of joy.)
- [ ] Elixir/Erlang (The BEAM VM is a whole thingy. I need a distributed‑systems wizard.)
- [ ] Dart (I don’t know Dart but it would be huge for mobile devs)
- [ ] Scala
- [ ] Clojure
- [ ] Crystal
- [ ] V  (New, simple, clean. I know nothing about it. I like it though, it looks clean.)
- [ ] Ada (old(?) but useful)
- [ ] Lisp/CLisp (very useful)
- [ ] PowerShell (same scary as Bash)
- [ ] Fish (same scary as Bash and PowerShell)
- [ ] Awk (Yes, really. Text processing god.)
- [ ] GDScript (So useful for Godot)
- [ ] Gleam (Another new lang, really cute mascot hehe)
- [ ] Jakt (from SerenityOS: <https://github.com/SerenityOS/jakt>)
- [ ] Luau (yes that one)
- [ ] Assembly (this would be peak but silly but still peak.)
- [ ] D (Dlang)
- [ ] MATLAB
- [ ] [Odin](https://github.com/odin-lang/odin)
- [ ] [Carbon](https://github.com/carbon-language/carbon-lang)
- [ ] [PureScript](https://github.com/purescript/purescript)
- [ ] [Vale](https://github.com/ValeLang/Vale)
- [ ] [Forth](https://www.forthlang.org/)
- [ ] VB.NET
- [ ] Nushell
- [ ] [Tungsten](https://github.com/RickIsGone/tungsten) (beta)

## Transports

- [ ] Add HTTP transport (credit: u/emetah850 on Reddit)
- [ ] Add support for custom transports (credit: u/rogerara on Reddit)
- [ ] Named Pipes (Windows)
- [ ] WebRTC Transport

## Other

- [ ] Every language adapter independently re-implements the same wire protocol logic, so every protocol change requires updating all of them
- [ ] Language Adapter Template Generator (using above)

## Current

- [ ] lessen Saikuro memory usage for embedded

- [ ] Asyncify fallback for JSPI (before the spinning one)
- [ ] Update `saikuro-c`/`saikuro-cpp`/`saikuro-csharp` to just use a new `Build/codegen/cxx/` thing that automatically generates bindings for all three (1-1 api match, not the yucky serializing to JSON or whatever) (not safe tho (cffi) :( hmmm)
- [ ] Possibly use something similar for JS/TS
- [ ] python-ctypes too for Python via the same `cxx/` (not safe tho (cffi) :( hmmm)

- [ ] Add more tests

- [ ] add fancy stuff like `miri`, `cargo-geiger`, **`cargo-checkmate`**, `cargo-spellcheck`/`typos-cli` lol i need it, `cargo-outdated`, `cargo-geiger`, **`siderophile`**, etc
- [ ] use nextest if we can, or a custom test runner cli that works across everything
- [ ] revamp CI, deny.toml, devcontainer and Just setup for the new system and everything
- [ ] Update docs/demo/examples for the new everything
- [ ] update all the various md files and stuff, CONTRIBUTING, CODE_OF_CONDUCT, CHANGELOG, HANDROLLED, PARITY, README, SECURITY, etc. and add more

- [ ] maybe a root Cargo.toml (nested workspaces possible? idk)

- [ ] remake demo to be something better, and maybe cooler? + combine with Examples so that we have one thing for all that
