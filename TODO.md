# TODO

## Adapters

### I can implement

- [ ] JavaScript (maybe TS wrapper idk but useful, or we can have `tsup` compile it to JS)
- [ ] Bash (lol, I know some Bash but this is the scariest adapter haha we don't want to delete anything from the system, it needs to be EXTREMELY SANDBOXED/safe)
- [ ] AssemblyScript (lol this technically isn't needed since TS adapter but we could have a wasm-specific adapter using it (I don't know AS but it's really similar to TS from what i've heard))
- [ ] Swift Adapter (Ladybird would benefit from it (bit too late for that but either way))
- [ ] Lua (super useful, especially for games and game engines and so much more (Picotility might benefit hehe))
- [ ] AppleScript (yes really, it's powerful but painful to write in)

### Need help

- [ ] Nim (Compiles to C. Easy adapter. I just don’t know Nim at all.)
- [ ] F# (easy, since we already have a C# one but idk F#)
- [ ] Java Adapter (Not good at Java in the slightest but I know a few people who are :D)
- [ ] Kotlin (genuinely super useful but again, idk Kotlin)
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
- [ ] Jakt (from SerenityOS: https://github.com/SerenityOS/jakt)
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
- [ ] Zsh
- [ ] Nushell
- [ ] [Tungsten](https://github.com/RickIsGone/tungsten) (beta)

## Transports

- [ ] Add HTTP transport (credit: u/emetah850 on Reddit)
- [ ] Add support for custom transports (credit: u/rogerara on Reddit)
- [ ] Named Pipes (Windows)
- [ ] WebRTC Transport

## Features

- [ ] Storage backend (will be working on soon), allows all FS access to be agnostic and stuff
- [O] Buildscripts (Python ofc) and more dev conveniences
- [ ] Make WASM compilation of runtime work like how i imagined it
- [ ] WasmHostTransport for WASM
    - A transport that uses:
        - postMessage (basic other stuff, logs for ex?)
        - MessageChannel (transport and channels (hah))
        - BroadcastChannel (discovery) 
    - (can choose which of the three or use all three in diff places)
- [ ] Language Adapter Template Generator
